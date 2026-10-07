//! Read-only installation, process and listener metadata for App identity.
//! No debugging connection, script injection, launch or credential writes.
#[cfg(any(target_os = "macos", test))]
use serde_json::Value;
#[cfg(any(target_os = "macos", test))]
use std::collections::BTreeSet;
#[cfg(any(target_os = "macos", test))]
use std::fs::File;
#[cfg(any(target_os = "macos", test))]
use std::io::{Read, Seek, SeekFrom};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct Installation {
    pub bundle: PathBuf,
    pub executable: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListenerObservation {
    pub address: SocketAddr,
    pub owner_pid: u32,
}

pub(crate) fn parse_listener_fields(text: &str) -> Result<Vec<ListenerObservation>, &'static str> {
    if text.len() > 256 * 1024 {
        return Err("metadata_unavailable");
    }
    let mut owner = None;
    let mut records = Vec::new();
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix('p') {
            owner = Some(raw.parse::<u32>().map_err(|_| "metadata_unavailable")?);
        } else if let Some(raw) = line.strip_prefix('n') {
            let pid = owner.filter(|p| *p > 0).ok_or("metadata_unavailable")?;
            let (host, port) = raw.rsplit_once(':').ok_or("metadata_unavailable")?;
            let port = port.parse::<u16>().map_err(|_| "metadata_unavailable")?;
            let ip: IpAddr = match host {
                "*" => "0.0.0.0".parse().unwrap(),
                other => {
                    let host = if other.starts_with('[') {
                        other
                            .strip_prefix('[')
                            .and_then(|v| v.strip_suffix(']'))
                            .ok_or("metadata_unavailable")?
                    } else {
                        other
                    };
                    host.parse().map_err(|_| "metadata_unavailable")?
                }
            };
            if port == 0 || records.len() >= 64 {
                return Err("metadata_unavailable");
            }
            records.push(ListenerObservation {
                address: SocketAddr::new(ip, port),
                owner_pid: pid,
            });
        }
    }
    if records.is_empty() {
        return Err("metadata_unavailable");
    }
    Ok(records)
}

#[cfg(target_os = "macos")]
pub(crate) fn command(program: &str, args: &[String]) -> Result<String, &'static str> {
    let mut command = std::process::Command::new(program);
    command.args(args).stdin(std::process::Stdio::null());
    let output =
        crate::utils::process::output_with_timeout(command, std::time::Duration::from_secs(3))
            .map_err(|_| "metadata_unavailable")?;
    if !output.status.success() || output.stdout.len() > 256 * 1024 || !output.stderr.is_empty() {
        return Err("metadata_unavailable");
    }
    String::from_utf8(output.stdout)
        .map(|v| v.trim().to_string())
        .map_err(|_| "metadata_unavailable")
}

#[cfg(target_os = "macos")]
pub(crate) fn pids(args: &[String]) -> Result<Vec<u32>, &'static str> {
    let mut command = std::process::Command::new("/usr/bin/pgrep");
    command.args(args).stdin(std::process::Stdio::null());
    let output =
        crate::utils::process::output_with_timeout(command, std::time::Duration::from_secs(3))
            .map_err(|_| "metadata_unavailable")?;
    if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(vec![]);
    }
    if !output.status.success() || output.stdout.len() > 1024 || !output.stderr.is_empty() {
        return Err("metadata_unavailable");
    }
    parse_pid_metadata(&output.stdout)
}

#[cfg(any(target_os = "macos", test))]
fn parse_pid_metadata(output: &[u8]) -> Result<Vec<u32>, &'static str> {
    if output.len() > 1024 {
        return Err("metadata_unavailable");
    }
    let parsed: Result<BTreeSet<u32>, _> = std::str::from_utf8(output)
        .map_err(|_| "metadata_unavailable")?
        .lines()
        .map(str::parse)
        .collect();
    let parsed = parsed.map_err(|_| "metadata_unavailable")?;
    if parsed.len() > 8 || parsed.contains(&0) {
        return Err("multiple_instances");
    }
    Ok(parsed.into_iter().collect())
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_executable(pid: u32, expected: &Path) -> Result<(), &'static str> {
    let actual = command(
        "/bin/ps",
        &[
            "-ww".into(),
            "-p".into(),
            pid.to_string(),
            "-o".into(),
            "comm=".into(),
        ],
    )?;
    let actual = std::fs::canonicalize(actual).map_err(|_| "metadata_unavailable")?;
    let expected = std::fs::canonicalize(expected).map_err(|_| "metadata_unavailable")?;
    if actual != expected {
        return Err("identity_mismatch");
    }
    Ok(())
}

/// Select only an exact executable match. Metadata collection failures must
/// never be mistaken for an unrelated process that can safely be ignored.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn verified_app_pid(
    candidates: &[u32],
    mut verify: impl FnMut(u32) -> Result<(), &'static str>,
) -> Result<u32, &'static str> {
    if candidates.is_empty() {
        return Err("not_running");
    }
    if candidates.len() > 8 || candidates.contains(&0) {
        return Err("multiple_instances");
    }
    let mut matches = Vec::new();
    for &pid in candidates {
        match verify(pid) {
            Ok(()) => matches.push(pid),
            Err("identity_mismatch") => {}
            Err(error) => return Err(error),
        }
    }
    match matches.as_slice() {
        [pid] => Ok(*pid),
        [] => Err("identity_mismatch"),
        _ => Err("multiple_instances"),
    }
}

fn verify_listener(
    port: u16,
    pid: u32,
    observations: &[ListenerObservation],
) -> Result<(), &'static str> {
    if port == 0
        || pid == 0
        || observations.is_empty()
        || observations.len() > 8
        || observations.iter().any(|row| {
            row.address.port() != port || !row.address.ip().is_loopback() || row.owner_pid != pid
        })
        || !observations
            .iter()
            .any(|row| row.address == SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
    {
        return Err("non_loopback_or_wrong_owner");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn listener(port: u16, pid: u32) -> Result<(), &'static str> {
    let text = command(
        "/usr/sbin/lsof",
        &[
            "-nP".into(),
            format!("-iTCP:{port}"),
            "-sTCP:LISTEN".into(),
            "-Fpn".into(),
        ],
    )?;
    verify_listener(port, pid, &parse_listener_fields(&text)?)
}

#[cfg(target_os = "macos")]
pub(crate) fn installed(configured: Option<&str>) -> Result<Option<Installation>, &'static str> {
    let candidates = if let Some(path) = configured {
        let mut path = PathBuf::from(path);
        while path.extension().is_none_or(|e| e != "app") {
            path = path.parent().ok_or("unknown_installation")?.to_path_buf();
        }
        vec![path]
    } else {
        let mut paths = vec![PathBuf::from("/Applications/Antigravity.app")];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Applications/Antigravity.app"));
        }
        paths
    };
    for bundle in candidates {
        match std::fs::symlink_metadata(&bundle) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("metadata_unavailable"),
        }
        let bundle = bundle.canonicalize().map_err(|_| "unknown_installation")?;
        let plist_path = bundle.join("Contents/Info.plist");
        if std::fs::metadata(&plist_path)
            .map_err(|_| "unknown_installation")?
            .len()
            > 1024 * 1024
        {
            return Err("unknown_installation");
        }
        let value = plist::Value::from_file(plist_path).map_err(|_| "unknown_installation")?;
        let info = value.as_dictionary().ok_or("unknown_installation")?;
        if info
            .get("CFBundleExecutable")
            .and_then(plist::Value::as_string)
            != Some("Antigravity")
        {
            return Err("unknown_installation");
        }
        let version = info
            .get("CFBundleShortVersionString")
            .and_then(plist::Value::as_string)
            .ok_or("unknown_installation")?
            .to_string();
        let asar_version = read_asar_version(&bundle.join("Contents/Resources/app.asar"))
            .ok_or("unknown_installation")?;
        if version != asar_version {
            return Err("unknown_installation");
        }
        return Ok(Some(Installation {
            executable: bundle.join("Contents/MacOS/Antigravity"),
            bundle,
        }));
    }
    Ok(None)
}

#[cfg(any(target_os = "macos", test))]
const MAX_ASAR_HEADER: usize = 4 * 1024 * 1024;
#[cfg(any(target_os = "macos", test))]
const MAX_PACKAGE: usize = 16 * 1024;

#[cfg(any(target_os = "macos", test))]
fn package_version(package: &[u8]) -> Option<String> {
    let package: Value = serde_json::from_slice(package).ok()?;
    // Antigravity IDE also uses the Antigravity name. Require the standalone
    // App's package identity; never run --version or execute a selected binary.
    if package.get("name")?.as_str()? != "antigravity"
        || package.get("productName")?.as_str()? != "Antigravity"
        || package.get("description")?.as_str()? != "Antigravity - Agentic Desktop Application"
    {
        return None;
    }
    let version = package.get("version")?.as_str()?;
    if version.len() > 32
        || version.split('.').count() != 3
        || !version.split('.').all(|part| {
            !part.is_empty() && part.len() <= 8 && part.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return None;
    }
    Some(version.to_string())
}

/// Read only the bounded package.json entry from an ASAR; no extraction or code
/// execution. Reject links, unpacked entries, oversized values and bad offsets.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn read_asar_version(path: &std::path::Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    let mut prefix = [0_u8; 16];
    file.read_exact(&mut prefix).ok()?;
    let number =
        |start: usize| u32::from_le_bytes(prefix[start..start + 4].try_into().unwrap()) as usize;
    let header_size = number(4);
    let json_size = number(12);
    if number(0) != 4
        || json_size == 0
        || header_size > MAX_ASAR_HEADER
        || json_size > header_size.checked_sub(8)?
        || number(8) > header_size
    {
        return None;
    }
    let mut header = vec![0; json_size];
    file.read_exact(&mut header).ok()?;
    let header: Value = serde_json::from_slice(&header).ok()?;
    let entry = header.get("files")?.get("package.json")?;
    if entry.get("link").is_some()
        || entry
            .get("unpacked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return None;
    }
    let offset = entry.get("offset")?.as_str()?.parse::<u64>().ok()?;
    let size = entry.get("size")?.as_u64()?;
    if size == 0 || size > MAX_PACKAGE as u64 {
        return None;
    }
    let position = (8_u64)
        .checked_add(header_size as u64)?
        .checked_add(offset)?;
    if position.checked_add(size)? > file_size {
        return None;
    }
    file.seek(SeekFrom::Start(position)).ok()?;
    let mut package = vec![0; size as usize];
    file.read_exact(&mut package).ok()?;
    package_version(&package)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_bounded_numeric_listener_metadata() {
        let rows =
            parse_listener_fields("p42\nf16\nn127.0.0.1:54321\np42\nn[::1]:54321\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .all(|r| r.owner_pid == 42 && r.address.ip().is_loopback()));
        for raw in [
            "",
            "n127.0.0.1:42",
            "pno\nn127.0.0.1:42",
            "p42\nnlocalhost:42",
            "p42\nn127.0.0.1:0",
            "p42\nn[[::1]]:42",
            "p42\nn[::1:42",
            "p0\nn127.0.0.1:42",
        ] {
            assert!(parse_listener_fields(raw).is_err(), "{raw}");
        }
        let external = parse_listener_fields("p42\nn*:54321").unwrap();
        assert!(!external[0].address.ip().is_loopback());
    }

    #[test]
    fn process_metadata_requires_bounded_positive_numeric_pids() {
        assert_eq!(parse_pid_metadata(b"42\n43\n42\n"), Ok(vec![42, 43]));
        for raw in [b"0\n".as_slice(), b"no\n", b"-42\n", b"42 x\n", b"\xff\n"] {
            assert!(parse_pid_metadata(raw).is_err());
        }
        assert_eq!(
            parse_pid_metadata(b"1\n2\n3\n4\n5\n6\n7\n8\n9\n"),
            Err("multiple_instances")
        );
        assert_eq!(
            parse_pid_metadata(&vec![b'1'; 1025]),
            Err("metadata_unavailable")
        );
    }

    #[test]
    fn executable_verification_never_swallows_metadata_errors() {
        assert_eq!(verified_app_pid(&[], |_| Ok(())), Err("not_running"));
        assert_eq!(verified_app_pid(&[42], |_| Ok(())), Ok(42));
        assert_eq!(
            verified_app_pid(&[42, 43], |_| Ok(())),
            Err("multiple_instances")
        );
        assert_eq!(
            verified_app_pid(&[42], |_| Err("identity_mismatch")),
            Err("identity_mismatch")
        );
        assert_eq!(
            verified_app_pid(&[42, 43], |pid| if pid == 42 {
                Ok(())
            } else {
                Err("identity_mismatch")
            }),
            Ok(42)
        );
        // A permissions/timeout failure cannot be downgraded to an excluded PID.
        assert_eq!(
            verified_app_pid(&[42, 43], |pid| if pid == 42 {
                Ok(())
            } else {
                Err("metadata_unavailable")
            }),
            Err("metadata_unavailable")
        );
    }

    fn official_package(version: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "name":"antigravity", "productName":"Antigravity",
            "description":"Antigravity - Agentic Desktop Application", "version":version
        }))
        .unwrap()
    }

    #[test]
    fn reads_only_standalone_app_identity() {
        for version in ["2.19.1", "2.21.0", "3.0.0"] {
            assert_eq!(package_version(&official_package(version)), Some(version.into()));
        }
        for version in ["", "2.19", "2.19.1-extra", "2.19.1.1", "a.b.c"] {
            assert_eq!(package_version(&official_package(version)), None);
        }
        assert_eq!(
            package_version(br#"{"name":"antigravity","version":"2.19.1"}"#),
            None
        );
        assert_eq!(package_version(b"not JSON"), None);
    }

    #[test]
    fn listener_requires_complete_loopback_ownership() {
        let local = parse_listener_fields("p42\nn127.0.0.1:54321\n").unwrap();
        assert_eq!(verify_listener(54321, 42, &local), Ok(()));
        for (port, pid, rows) in [
            (0, 42, local.clone()),
            (54321, 0, local.clone()),
            (54322, 42, local.clone()),
            (54321, 43, local.clone()),
            (54321, 42, vec![]),
            (54321, 42, vec![local[0].clone(); 9]),
            (
                54321,
                42,
                parse_listener_fields("p42\nn[::1]:54321").unwrap(),
            ),
            (
                54321,
                42,
                parse_listener_fields("p42\nn127.0.0.1:54321\np43\nn[::1]:54321").unwrap(),
            ),
            (
                54321,
                42,
                parse_listener_fields("p42\nn127.0.0.1:54321\nn*:54321").unwrap(),
            ),
        ] {
            assert_eq!(
                verify_listener(port, pid, &rows),
                Err("non_loopback_or_wrong_owner")
            );
        }
    }

    #[test]
    fn malformed_asar_is_rejected_without_executing_anything() {
        let root = std::env::temp_dir().join(format!("atl-asar-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("app.asar");
        for bytes in [vec![], vec![0; 16], vec![255; 16]] {
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(read_asar_version(&path), None);
        }
        let package = official_package("2.19.1");
        let header = serde_json::to_vec(&serde_json::json!({"files":{"package.json":{
            "size":package.len(), "offset":"0"
        }}}))
        .unwrap();
        let mut bytes = Vec::new();
        for n in [
            4,
            header.len() as u32 + 8,
            header.len() as u32 + 4,
            header.len() as u32,
        ] {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&package);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read_asar_version(&path), Some("2.19.1".to_string()));
        bytes.truncate(bytes.len() - 1);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_asar_version(&path), None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
