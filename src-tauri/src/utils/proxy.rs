//! Fixed system proxies are reread when the shared client cache expires.
//! Environment settings take precedence; local App RPC clients remain explicitly direct.
use crate::models::config::{NetworkPreferences, ProxyMode};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(crate) struct Routes {
    pub http: Option<String>,
    pub https: Option<String>,
    pub all: Option<String>,
    pub bypass: String,
}
#[derive(Clone)]
pub(crate) enum Routing {
    NativeSystem,
    Direct,
    Fixed(Routes),
}

pub(crate) fn resolve() -> Result<Routing, String> {
    let settings = crate::modules::config::load_app_config()?.network;
    resolve_with(&settings, has_proxy_environment(), system_routes)
}

fn resolve_with(
    settings: &NetworkPreferences,
    environment: bool,
    system: impl FnOnce() -> Result<Option<Routes>, String>,
) -> Result<Routing, String> {
    settings.validate()?;
    match settings.proxy_mode {
        ProxyMode::Direct => Ok(Routing::Direct),
        ProxyMode::Manual => Ok(Routing::Fixed(Routes {
            all: Some(settings.proxy_url.clone()),
            bypass: "localhost,127.0.0.1,::1".into(),
            ..Routes::default()
        })),
        ProxyMode::System if environment => Ok(Routing::NativeSystem),
        ProxyMode::System => Ok(system()?
            .map(Routing::Fixed)
            .unwrap_or(Routing::NativeSystem)),
    }
}

fn has_proxy_environment() -> bool {
    [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ]
    .iter()
    .any(|key| std::env::var(key).is_ok_and(|value| !value.trim().is_empty()))
}

#[cfg(target_os = "macos")]
fn system_routes() -> Result<Option<Routes>, String> {
    let output = std::process::Command::new("/usr/sbin/scutil")
        .arg("--proxy")
        .output()
        .map_err(|_| "Cannot read system proxy settings.")?;
    if !output.status.success() {
        return Err("Cannot read system proxy settings.".into());
    }
    parse_macos(&String::from_utf8_lossy(&output.stdout)).map(Some)
}

#[cfg(target_os = "windows")]
fn system_routes() -> Result<Option<Routes>, String> {
    use crate::utils::command::CommandExtWrapper;
    let output = std::process::Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        ])
        .creation_flags_windows()
        .output()
        .map_err(|_| "Cannot read system proxy settings.")?;
    if !output.status.success() {
        return Err("Cannot read system proxy settings.".into());
    }
    parse_windows(&String::from_utf8_lossy(&output.stdout)).map(Some)
}

#[cfg(any(target_os = "windows", test))]
fn parse_windows(text: &str) -> Result<Routes, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let mut fields = line.trim().splitn(2, char::is_whitespace);
        if let (Some(key), Some(rest)) = (fields.next(), fields.next()) {
            let rest = rest.trim_start();
            if let Some((_, value)) = rest.split_once(char::is_whitespace) {
                map.insert(key, value.trim());
            }
        }
    }
    let get = |key| map.get(key).copied().unwrap_or("");
    if get("ProxyEnable") != "0x1" {
        if !get("AutoConfigURL").is_empty() {
            return Err("PAC proxy scripts are unsupported. Set the proxy tool's HTTP or SOCKS address in Settings.".into());
        }
        return Ok(Routes::default());
    }
    let mut routes = Routes {
        bypass: get("ProxyOverride").replace(';', ",").replace("*.", "."),
        ..Routes::default()
    };
    for entry in get("ProxyServer")
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (kind, address) = entry.split_once('=').unwrap_or(("all", entry));
        let address = if address.contains("://") {
            address.to_owned()
        } else {
            format!(
                "{}://{address}",
                if kind == "socks" { "socks5h" } else { "http" }
            )
        };
        url::Url::parse(&address).map_err(|_| "Invalid system proxy address.")?;
        match kind {
            "http" => routes.http = Some(address),
            "https" => routes.https = Some(address),
            "all" | "socks" => routes.all = Some(address),
            _ => {}
        }
    }
    Ok(routes)
}

#[cfg(target_os = "linux")]
fn system_routes() -> Result<Option<Routes>, String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    if desktop.contains("kde") {
        if let Some(config) = crate::modules::linux_paths::config_home() {
            if let Ok(text) = std::fs::read_to_string(config.join("kioslaverc")) {
                return parse_kde(&text).map(Some);
            }
        }
    }
    if desktop.contains("gnome") || desktop.contains("unity") || desktop.contains("cinnamon") {
        let output = std::process::Command::new("gsettings")
            .args(["list-recursively", "org.gnome.system.proxy"])
            .output()
            .map_err(|_| "Cannot read desktop proxy settings; set a manual proxy in Settings.")?;
        if !output.status.success() {
            return Err(
                "Cannot read desktop proxy settings; set a manual proxy in Settings.".into(),
            );
        }
        return parse_gnome(&String::from_utf8_lossy(&output.stdout)).map(Some);
    }
    Ok(None)
}

fn endpoint(scheme: &str, host: &str, port: &str) -> Option<String> {
    let host = host.trim().trim_matches('\'');
    let port: u16 = port.trim().parse().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    let mut url = url::Url::parse(&format!("{scheme}://localhost")).ok()?;
    url.set_host(Some(host)).ok()?;
    url.set_port(Some(port)).ok()?;
    Some(url.to_string())
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos(text: &str) -> Result<Routes, String> {
    let mut map = HashMap::new();
    let mut exceptions = Vec::new();
    let mut in_exceptions = false;
    for line in text.lines() {
        if let Some((key, value)) = line.trim().split_once(" : ") {
            if key == "ExceptionsList" {
                in_exceptions = true;
                continue;
            }
            if in_exceptions && key.parse::<usize>().is_ok() {
                exceptions.push(value.to_owned());
                continue;
            }
            map.insert(key, value);
        } else if line.trim() == "}" {
            in_exceptions = false;
        }
    }
    let get = |key| map.get(key).copied().unwrap_or("");
    let http = (get("HTTPEnable") == "1")
        .then(|| endpoint("http", get("HTTPProxy"), get("HTTPPort")))
        .flatten();
    let https = (get("HTTPSEnable") == "1")
        .then(|| endpoint("http", get("HTTPSProxy"), get("HTTPSPort")))
        .flatten();
    let all = (get("SOCKSEnable") == "1")
        .then(|| endpoint("socks5h", get("SOCKSProxy"), get("SOCKSPort")))
        .flatten();
    if http.is_none()
        && https.is_none()
        && all.is_none()
        && (get("ProxyAutoConfigEnable") == "1" || get("ProxyAutoDiscoveryEnable") == "1")
    {
        return Err("PAC proxy scripts are unsupported. Set the proxy tool's HTTP or SOCKS address in Settings.".into());
    }
    exceptions.extend(["localhost".into(), "127.0.0.1".into(), "::1".into()]);
    Ok(Routes {
        http,
        https,
        all,
        bypass: exceptions.join(",").replace("*.", "."),
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_gnome(text: &str) -> Result<Routes, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let mut fields = line.splitn(3, ' ');
        if let (Some(schema), Some(key), Some(value)) =
            (fields.next(), fields.next(), fields.next())
        {
            map.insert(
                format!("{schema}.{key}"),
                value.trim().trim_matches('\'').to_owned(),
            );
        }
    }
    let get = |schema: &str, key: &str| {
        map.get(&format!("org.gnome.system.proxy{schema}.{key}"))
            .cloned()
            .unwrap_or_default()
    };
    match get("", "mode").as_str() {
        "auto" => return Err("PAC proxy scripts are unsupported. Set the proxy tool's HTTP or SOCKS address in Settings.".into()),
        "manual" => {},
        _ => return Ok(Routes::default()),
    }
    let mut http = endpoint("http", &get(".http", "host"), &get(".http", "port"));
    if get(".http", "use-authentication") == "true" {
        if let Some(value) = http.as_mut() {
            let mut url = url::Url::parse(value).map_err(|_| "Invalid system proxy address.")?;
            url.set_username(&get(".http", "authentication-user"))
                .map_err(|_| "Invalid system proxy address.")?;
            url.set_password(Some(&get(".http", "authentication-password")))
                .map_err(|_| "Invalid system proxy address.")?;
            *value = url.to_string();
        }
    }
    let https = if get("", "use-same-proxy") == "true" {
        http.clone()
    } else {
        endpoint("http", &get(".https", "host"), &get(".https", "port")).or_else(|| http.clone())
    };
    let all = endpoint("socks5h", &get(".socks", "host"), &get(".socks", "port"));
    let bypass = get("", "ignore-hosts")
        .trim_start_matches("@as ")
        .trim_matches(['[', ']'])
        .replace(['\'', ' '], "")
        .replace("*.", ".");
    Ok(Routes {
        http,
        https,
        all,
        bypass,
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_kde(text: &str) -> Result<Routes, String> {
    let mut in_group = false;
    let mut map = HashMap::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_group = line == "[Proxy Settings]";
            continue;
        }
        if in_group {
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim(), value.trim());
            }
        }
    }
    let get = |key| map.get(key).copied().unwrap_or("");
    match get("ProxyType") {
        "1" => {},
        "2" | "3" => return Err("PAC proxy scripts are unsupported. Set the proxy tool's HTTP or SOCKS address in Settings.".into()),
        _ => return Ok(Routes::default()),
    }
    if get("ReversedException") == "true" {
        return Err(
            "Reverse proxy exceptions are unsupported. Set a manual proxy in Settings.".into(),
        );
    }
    let proxy = |key, socks: bool| {
        let value = get(key);
        if value.is_empty() {
            return None;
        }
        let value = if let Some((host, port)) = value.rsplit_once(' ') {
            format!("{host}:{port}")
        } else {
            value.to_owned()
        };
        let value = if socks {
            format!("socks5h://{}", value.split("://").last().unwrap_or(&value))
        } else if value.contains("://") {
            value
        } else {
            format!("http://{value}")
        };
        url::Url::parse(&value).ok().map(|url| url.to_string())
    };
    Ok(Routes {
        http: proxy("httpProxy", false),
        https: proxy("httpsProxy", false),
        all: proxy("socksProxy", true),
        bypass: get("NoProxyFor").replace(';', ",").replace("*.", "."),
    })
}

pub(crate) fn configure_rquest(
    mut builder: rquest::ClientBuilder,
    route: &Routing,
) -> Result<rquest::ClientBuilder, String> {
    match route {
        Routing::NativeSystem => {}
        Routing::Direct => builder = builder.no_proxy(),
        Routing::Fixed(routes) => {
            builder = builder.no_proxy();
            for (kind, value) in [
                ("http", &routes.http),
                ("https", &routes.https),
                ("all", &routes.all),
            ] {
                if let Some(value) = value {
                    let proxy = match kind {
                        "http" => rquest::Proxy::http(value),
                        "https" => rquest::Proxy::https(value),
                        _ => rquest::Proxy::all(value),
                    }
                    .map_err(|_| "Invalid proxy settings.")?
                    .no_proxy(rquest::NoProxy::from_string(&routes.bypass));
                    builder = builder.proxy(proxy);
                }
            }
        }
    }
    Ok(builder)
}

pub(crate) fn reqwest_proxies(route: &Routing) -> Result<Option<Vec<reqwest::Proxy>>, String> {
    match route {
        Routing::NativeSystem => Ok(None),
        Routing::Direct => Ok(Some(Vec::new())),
        Routing::Fixed(routes) => {
            let mut proxies = Vec::new();
            for (kind, value) in [
                ("http", &routes.http),
                ("https", &routes.https),
                ("all", &routes.all),
            ] {
                if let Some(value) = value {
                    let proxy = match kind {
                        "http" => reqwest::Proxy::http(value),
                        "https" => reqwest::Proxy::https(value),
                        _ => reqwest::Proxy::all(value),
                    }
                    .map_err(|_| "Invalid proxy settings.")?
                    .no_proxy(reqwest::NoProxy::from_string(&routes.bypass));
                    proxies.push(proxy);
                }
            }
            Ok(Some(proxies))
        }
    }
}

pub(crate) fn configure_reqwest(
    mut builder: reqwest::ClientBuilder,
) -> Result<reqwest::ClientBuilder, String> {
    if let Some(proxies) = reqwest_proxies(&resolve()?)? {
        builder = builder.no_proxy();
        for proxy in proxies {
            builder = builder.proxy(proxy);
        }
    }
    Ok(builder)
}

pub(crate) fn configure_blocking(
    mut builder: reqwest::blocking::ClientBuilder,
) -> Result<reqwest::blocking::ClientBuilder, String> {
    if let Some(proxies) = reqwest_proxies(&resolve()?)? {
        builder = builder.no_proxy();
        for proxy in proxies {
            builder = builder.proxy(proxy);
        }
    }
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };

    #[test]
    fn default_system_environment_precedence_and_manual_validation() {
        let settings = NetworkPreferences::default();
        assert_eq!(settings.proxy_mode, ProxyMode::System);
        assert!(matches!(
            resolve_with(&settings, true, || panic!("environment wins")),
            Ok(Routing::NativeSystem)
        ));
        assert!(matches!(
            resolve_with(&settings, false, || Ok(Some(Routes {
                http: Some("http://127.0.0.1:7890".into()),
                ..Routes::default()
            }))),
            Ok(Routing::Fixed(_))
        ));
        let mut manual = NetworkPreferences {
            proxy_mode: ProxyMode::Manual,
            proxy_url: "socks5h://127.0.0.1:1080".into(),
        };
        assert!(manual.validate().is_ok());
        manual.proxy_url = "http://user:secret@localhost:7890".into();
        assert!(manual.validate().is_err());
    }

    #[test]
    fn desktop_fixed_http_socks_bypass_and_pac_are_distinguished() {
        let mac = parse_macos("HTTPEnable : 1\nHTTPProxy : 127.0.0.1\nHTTPPort : 7890\nHTTPSEnable : 1\nHTTPSProxy : 127.0.0.1\nHTTPSPort : 7890\nSOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 7891\nExceptionsList : <array> {\n0 : *.local.test\n}").unwrap();
        assert_eq!(mac.https.as_deref(), Some("http://127.0.0.1:7890/"));
        assert_eq!(mac.all.as_deref(), Some("socks5h://127.0.0.1:7891"));
        assert!(mac.bypass.contains(".local.test"));
        assert!(parse_macos("ProxyAutoConfigEnable : 1").is_err());
        let gnome = parse_gnome("org.gnome.system.proxy mode 'manual'\norg.gnome.system.proxy use-same-proxy true\norg.gnome.system.proxy.http host '127.0.0.1'\norg.gnome.system.proxy.http port 7890\norg.gnome.system.proxy ignore-hosts ['localhost', '*.local.test']").unwrap();
        assert_eq!(gnome.http, gnome.https);
        assert!(gnome.bypass.contains(".local.test"));
        let kde = parse_kde("[Other]\nProxyType=0\n[Proxy Settings]\nProxyType=1\nhttpProxy=http://127.0.0.1 7890\nhttpsProxy=http://127.0.0.1 7890\nsocksProxy=socks://127.0.0.1 7891\nNoProxyFor=localhost,*.local.test").unwrap();
        assert_eq!(kde.http, gnome.http);
        assert!(kde.all.unwrap().starts_with("socks5h://"));
        assert!(parse_kde("[Proxy Settings]\nProxyType=2").is_err());
        assert!(parse_gnome("org.gnome.system.proxy mode 'auto'").is_err());
        let windows = parse_windows("ProxyEnable    REG_DWORD    0x1\nProxyServer    REG_SZ    http=127.0.0.1:7890;https=127.0.0.1:7890;socks=127.0.0.1:7891\nProxyOverride    REG_SZ    localhost;*.local.test").unwrap();
        assert_eq!(windows.https.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(windows.all.as_deref(), Some("socks5h://127.0.0.1:7891"));
    }

    fn http_proxy() -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(
                    Instant::now() < deadline,
                    "synthetic proxy received no connection"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let len = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..len]).into_owned();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture",
                )
                .unwrap();
            request
        });
        (address, worker)
    }

    #[tokio::test]
    async fn fixed_proxy_routes_http_without_tun_or_destination_dns() {
        let (address, worker) = http_proxy();
        let route = Routing::Fixed(Routes {
            http: Some(address),
            ..Routes::default()
        });
        let client = configure_rquest(
            rquest::Client::builder().timeout(Duration::from_secs(5)),
            &route,
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(
            client
                .get("http://unreachable.invalid/fixture")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "fixture"
        );
        assert!(worker
            .join()
            .unwrap()
            .starts_with("GET http://unreachable.invalid/fixture"));
    }

    #[test]
    fn system_proxy_changes_are_reread_in_isolated_process() {
        let temp = tempfile::tempdir().unwrap();
        let (first, first_worker) = http_proxy();
        let (second, second_worker) = http_proxy();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "utils::proxy::tests::proxy_env_child",
                "--ignored",
            ])
            .env("ABV_DATA_DIR", temp.path())
            .env("AGY_PROXY_FIXTURE", &second)
            .env("HTTP_PROXY", first)
            .env("HTTPS_PROXY", &second)
            .env_remove("ALL_PROXY")
            .env_remove("http_proxy")
            .env_remove("https_proxy")
            .env_remove("all_proxy")
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(first_worker.join().unwrap().contains("unreachable.invalid"));
        assert!(second_worker
            .join()
            .unwrap()
            .contains("unreachable.invalid"));
    }

    #[tokio::test]
    async fn socks_proxy_resolves_destination_and_routes_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("socks5h://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut header = [0; 2];
            stream.read_exact(&mut header).unwrap();
            assert_eq!(header[0], 5);
            let mut methods = vec![0; header[1] as usize];
            stream.read_exact(&mut methods).unwrap();
            assert!(methods.contains(&0));
            stream.write_all(&[5, 0]).unwrap();
            let mut connect = [0; 5];
            stream.read_exact(&mut connect).unwrap();
            assert_eq!(&connect[..4], &[5, 1, 0, 3]);
            let mut host = vec![0; connect[4] as usize];
            stream.read_exact(&mut host).unwrap();
            assert_eq!(host, b"unreachable.invalid");
            let mut port = [0; 2];
            stream.read_exact(&mut port).unwrap();
            assert_eq!(u16::from_be_bytes(port), 80);
            stream
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
                .unwrap();
            let mut request = [0; 4096];
            stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture",
                )
                .unwrap();
        });
        let client = configure_rquest(
            rquest::Client::builder().timeout(Duration::from_secs(5)),
            &Routing::Fixed(Routes {
                all: Some(address),
                ..Routes::default()
            }),
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(
            client
                .get("http://unreachable.invalid/fixture")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "fixture"
        );
        worker.join().unwrap();
    }

    #[test]
    #[ignore = "isolated child only; never reads the user's configuration"]
    fn proxy_env_child() {
        let second = std::env::var("AGY_PROXY_FIXTURE").expect("isolated proxy fixture required");
        let root = std::env::var_os("ABV_DATA_DIR").expect("isolated store required");
        assert!(std::path::Path::new(&root).starts_with(std::env::temp_dir()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for iteration in 0..2 {
                if iteration == 1 {
                    std::env::set_var("HTTP_PROXY", &second);
                    tokio::time::sleep(Duration::from_millis(2100)).await;
                }
                let client = crate::utils::http::get_long_standard_client().unwrap();
                assert_eq!(
                    client
                        .get("http://unreachable.invalid/fixture")
                        .send()
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap(),
                    "fixture"
                );
            }
        });
    }
}
