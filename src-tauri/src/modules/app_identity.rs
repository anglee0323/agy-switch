//! Read the running App's cached identity, rather than a shared keyring entry
//! that an older agy process can refresh independently. Never writes credentials.

#[cfg(any(target_os = "macos", test))]
use std::io::Read;

#[cfg(any(target_os = "macos", test))]
const STATUS_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUserStatus";
#[cfg(any(target_os = "macos", test))]
const MAX_STATUS: u64 = 128 * 1024;

#[cfg(any(target_os = "macos", test))]
fn csrf_from_args(args: &str) -> Result<&str, String> {
    let mut words = args.split_whitespace();
    let mut found = None;
    while let Some(word) = words.next() {
        let value = if word == "--csrf_token" {
            Some(words.next().ok_or("running_app_identity_unavailable")?)
        } else {
            word.strip_prefix("--csrf_token=")
        };
        if let Some(value) = value {
            if found.is_some() || uuid::Uuid::parse_str(value).is_err() {
                return Err("running_app_identity_unavailable".into());
            }
            found = Some(value);
        }
    }
    found.ok_or_else(|| "running_app_identity_unavailable".into())
}

#[cfg(any(target_os = "macos", test))]
fn status_email(bytes: &[u8]) -> Result<String, String> {
    let status: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "running_app_identity_unavailable".to_string())?;
    let email = status["userStatus"]["email"]
        .as_str()
        .filter(|email| {
            email.len() <= 254
                && email.is_ascii()
                && !email
                    .bytes()
                    .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
                && email.split_once('@').is_some_and(|(name, domain)| {
                    !name.is_empty() && !domain.is_empty() && !domain.contains('@')
                })
        })
        .ok_or("running_app_identity_unavailable")?;
    Ok(email.to_string())
}

#[cfg(any(target_os = "macos", test))]
fn status_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|_| "running_app_identity_unavailable".to_string())
}

#[cfg(any(target_os = "macos", test))]
fn request_email(
    client: &reqwest::blocking::Client,
    port: u16,
    csrf: &str,
) -> Result<String, String> {
    // The native App's language server also exposes its Connect RPC on an
    // independently owned loopback HTTP listener. No TLS bypass is needed.
    let response = client
        .post(format!("http://127.0.0.1:{port}{STATUS_PATH}"))
        .header("Content-Type", "application/json")
        .header("x-codeium-csrf-token", csrf)
        .body("{}")
        .send()
        .map_err(|_| "running_app_identity_unavailable".to_string())?;
    if !response.status().is_success() {
        return Err("running_app_identity_unavailable".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_STATUS + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "running_app_identity_unavailable".to_string())?;
    if bytes.len() as u64 > MAX_STATUS {
        return Err("running_app_identity_unavailable".into());
    }
    status_email(&bytes)
}

/// None means the App is absent/closed. A running but unverifiable App is an
/// error: falling back to the keyring then would reproduce the false identity.
#[cfg(target_os = "macos")]
pub(crate) fn running_email(configured: Option<&str>) -> Result<Option<String>, String> {
    use super::app_metadata_macos as metadata;
    let Some(installation) = metadata::installed(configured)? else {
        return Ok(None);
    };
    let candidates = metadata::pids(&["-x".into(), "Antigravity".into()])?;
    let app_pid = match metadata::verified_app_pid(&candidates, |pid| {
        metadata::verify_executable(pid, &installation.executable)
    }) {
        Ok(pid) => pid,
        Err("not_running") => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    // Package identity, process ownership and the status response establish
    // compatibility. A client update alone must not prevent this read-only RPC.
    let servers = metadata::pids(&[
        "-P".into(),
        app_pid.to_string(),
        "-x".into(),
        "language_server".into(),
    ])?;
    let [server] = servers.as_slice() else {
        return Err("running_app_identity_unavailable".into());
    };
    let executable = installation
        .bundle
        .join("Contents/Resources/bin/language_server");
    metadata::verify_executable(*server, &executable)?;
    let parent = metadata::command(
        "/bin/ps",
        &["-p".into(), server.to_string(), "-o".into(), "ppid=".into()],
    )?;
    if parent.parse::<u32>().ok() != Some(app_pid) {
        return Err("running_app_identity_unavailable".into());
    }
    let args = metadata::command(
        "/bin/ps",
        &[
            "-ww".into(),
            "-p".into(),
            server.to_string(),
            "-o".into(),
            "args=".into(),
        ],
    )?;
    let csrf = csrf_from_args(&args)?;
    let listeners = metadata::command(
        "/usr/sbin/lsof",
        &[
            "-nP".into(),
            "-a".into(),
            "-p".into(),
            server.to_string(),
            "-iTCP".into(),
            "-sTCP:LISTEN".into(),
            "-Fpn".into(),
        ],
    )?;
    let ports: std::collections::BTreeSet<_> = metadata::parse_listener_fields(&listeners)?
        .into_iter()
        .map(|row| row.address.port())
        .collect();
    if ports.is_empty() || ports.len() > 8 {
        return Err("running_app_identity_unavailable".into());
    }
    let client = status_client()?;
    for port in ports {
        // Revalidate ownership before sending the in-memory CSRF value.
        metadata::verify_executable(*server, &executable)?;
        metadata::listener(port, *server)?;
        if let Ok(email) = request_email(&client, port, csrf) {
            metadata::verify_executable(app_pid, &installation.executable)?;
            return Ok(Some(email));
        }
    }
    Err("running_app_identity_unavailable".into())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn running_email(_: Option<&str>) -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_metadata_is_unique_and_never_echoed_on_failure() {
        let value = "00000000-0000-4000-8000-000000000001";
        assert_eq!(
            csrf_from_args(&format!("server --csrf_token {value}")),
            Ok(value)
        );
        assert_eq!(
            csrf_from_args(&format!("server --csrf_token={value}")),
            Ok(value)
        );
        for args in [
            "server",
            "server --csrf_token",
            "server --csrf_token invalid",
            &format!("server --csrf_token {value} --csrf_token={value}"),
        ] {
            assert_eq!(
                csrf_from_args(args),
                Err("running_app_identity_unavailable".into())
            );
        }
    }

    #[test]
    fn identity_requires_the_live_status_email_not_unrelated_fields() {
        assert_eq!(
            status_email(
                br#"{"userStatus":{"email":"second@example.test"},"email":"first@example.test"}"#
            ),
            Ok("second@example.test".into())
        );
        for bytes in [
            br#"{"email":"first@example.test"}"#.as_slice(),
            br#"{"userStatus":{}}"#,
            br#"{"userStatus":{"email":"bad\n@example.test"}}"#,
            br#"{"userStatus":{"email":"@example.test"}}"#,
        ] {
            assert!(status_email(bytes).is_err());
        }
    }

    #[test]
    fn synthetic_status_rpc_returns_the_running_identity() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 4096);
            }
            let mut body = [0; 2];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(&body, b"{}");
            let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
            assert!(request.starts_with(&format!(
                "post {} http/1.1",
                STATUS_PATH.to_ascii_lowercase()
            )));
            assert!(request.contains("x-codeium-csrf-token: fixture-csrf"));
            let body = r#"{"userStatus":{"email":"second@example.test"}}"#;
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let client = status_client().unwrap();
        assert_eq!(
            request_email(&client, port, "fixture-csrf"),
            Ok("second@example.test".into())
        );
        server.join().unwrap();
    }

    #[test]
    fn synthetic_rpc_refuses_redirects_and_oversized_responses() {
        use std::io::Write;
        for redirect in [true, false] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 4096);
                }
                let mut body = [0; 2];
                socket.read_exact(&mut body).unwrap();
                assert_eq!(&body, b"{}");
                if redirect {
                    write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                } else {
                    let body = vec![b' '; MAX_STATUS as usize + 2];
                    write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    let _ = socket.write_all(&body);
                }
            });
            assert_eq!(
                request_email(&status_client().unwrap(), port, "fixture-csrf"),
                Err("running_app_identity_unavailable".into())
            );
            server.join().unwrap();
        }
    }
}
