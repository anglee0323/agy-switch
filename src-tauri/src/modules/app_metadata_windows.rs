//! Windows App identity and complete TCP listener ownership. Never collect
//! process command lines, environment variables, credentials or page contents.
use super::app_transport::{AppOrigin, ListenerObservation, VerifiedListener};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) struct Installation {
    pub executable: PathBuf,
    pub server: PathBuf,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProcessObservation {
    pub pid: u32,
    pub parent: Option<u32>,
    pub executable: PathBuf,
    pub started: u64,
    pub same_user: bool,
}

pub(crate) fn processes() -> Result<Vec<ProcessObservation>, &'static str> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        ProcessRefreshKind::new()
            .with_exe(UpdateKind::Always)
            .with_user(UpdateKind::Always),
    );
    let user = system
        .process(Pid::from_u32(std::process::id()))
        .and_then(|p| p.user_id())
        .ok_or("metadata_unavailable")?;
    let mut result = Vec::new();
    for (pid, process) in system.processes() {
        if !["antigravity.exe", "language_server.exe"].contains(
            &process
                .name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
        ) {
            continue;
        }
        let executable = process
            .exe()
            .and_then(|p| p.canonicalize().ok())
            .ok_or("metadata_unavailable")?;
        let owner = process.user_id().ok_or("metadata_unavailable")?;
        result.push(ProcessObservation {
            pid: pid.as_u32(),
            parent: process.parent().map(|p| p.as_u32()),
            executable,
            started: process.start_time(),
            same_user: owner == user,
        });
        if result.len() > 64 {
            return Err("multiple_instances");
        }
    }
    Ok(result)
}

fn installation(executable: &Path) -> Option<Installation> {
    let executable = executable.canonicalize().ok()?;
    if !executable
        .file_name()?
        .to_str()?
        .eq_ignore_ascii_case("Antigravity.exe")
    {
        return None;
    }
    let resources = executable.parent()?.join("resources");
    let version = super::app_metadata_macos::read_asar_version(&resources.join("app.asar"))?;
    let server = resources
        .join("bin/language_server.exe")
        .canonicalize()
        .ok()?;
    Some(Installation {
        executable,
        server,
        version,
    })
}

pub(crate) fn installed(
    configured: Option<&str>,
    processes: &[ProcessObservation],
) -> Result<Installation, &'static str> {
    if let Some(path) = configured {
        return installation(Path::new(path)).ok_or("unknown_installation");
    }
    let mut candidates: BTreeSet<PathBuf> = processes
        .iter()
        .filter(|p| {
            p.same_user
                && p.executable
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("Antigravity.exe"))
        })
        .map(|p| p.executable.clone())
        .collect();
    for (variable, suffix) in [
        ("LOCALAPPDATA", "Programs/Antigravity/Antigravity.exe"),
        ("ProgramFiles", "Antigravity/Antigravity.exe"),
        ("ProgramFiles(x86)", "Antigravity/Antigravity.exe"),
    ] {
        if let Some(root) = std::env::var_os(variable) {
            candidates.insert(PathBuf::from(root).join(suffix));
        }
    }
    let mut matches = candidates.iter().filter_map(|p| installation(p));
    let found = matches.next().ok_or("app_not_installed")?;
    if matches.any(|p| p.executable != found.executable) {
        return Err("multiple_instances");
    }
    Ok(found)
}

#[cfg(target_os = "windows")]
pub(crate) fn active_port_file() -> Result<String, &'static str> {
    use std::io::Read;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    let path = dirs::data_dir()
        .ok_or("no_debug_port")?
        .join("Antigravity/DevToolsActivePort");
    // Open the reparse point itself, then reject it; never follow a redirected
    // port file. Socket/process ownership is checked independently afterward.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(0x00200000)
        .open(path)
        .map_err(|_| "no_debug_port")?;
    let metadata = file.metadata().map_err(|_| "no_debug_port")?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err("no_debug_port");
    }
    let mut contents = String::new();
    file.take(257)
        .read_to_string(&mut contents)
        .map_err(|_| "no_debug_port")?;
    Ok(contents)
}

#[cfg(target_os = "windows")]
pub(crate) fn listeners() -> Result<Vec<ListenerObservation>, &'static str> {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
    let mut observations = Vec::new();
    for family in [AF_INET, AF_INET6] {
        let mut size = 0_u32;
        let result = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                family as u32,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result != 122 {
            return Err("metadata_unavailable");
        }
        let mut complete = false;
        for _ in 0..3 {
            if !(4..=1024 * 1024).contains(&size) {
                return Err("metadata_unavailable");
            }
            let mut bytes = vec![0_u8; size as usize];
            let result = unsafe {
                GetExtendedTcpTable(
                    bytes.as_mut_ptr().cast(),
                    &mut size,
                    0,
                    family as u32,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                )
            };
            if result == 122 {
                continue;
            }
            if result != 0 || size < 4 || size as usize > bytes.len() {
                return Err("metadata_unavailable");
            }
            let count = u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as usize;
            let stride = if family == AF_INET {
                std::mem::size_of::<MIB_TCPROW_OWNER_PID>()
            } else {
                std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>()
            };
            if count > 16384 || 4 + count * stride > size as usize {
                return Err("metadata_unavailable");
            }
            for index in 0..count {
                let pointer = unsafe { bytes.as_ptr().add(4 + index * stride) };
                let (ip, port, pid) = if family == AF_INET {
                    let row =
                        unsafe { std::ptr::read_unaligned(pointer.cast::<MIB_TCPROW_OWNER_PID>()) };
                    (
                        Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes()).into(),
                        row.dwLocalPort,
                        row.dwOwningPid,
                    )
                } else {
                    let row = unsafe {
                        std::ptr::read_unaligned(pointer.cast::<MIB_TCP6ROW_OWNER_PID>())
                    };
                    (
                        Ipv6Addr::from(row.ucLocalAddr).into(),
                        row.dwLocalPort,
                        row.dwOwningPid,
                    )
                };
                observations.push(ListenerObservation {
                    address: SocketAddr::new(ip, u16::from_be(port as u16)),
                    owner_pid: pid,
                });
            }
            complete = true;
            break;
        }
        if !complete {
            return Err("metadata_unavailable");
        }
    }
    Ok(observations)
}

pub(crate) fn browser<'a>(
    processes: &'a [ProcessObservation],
    executable: &Path,
    port: u16,
    rows: &[ListenerObservation],
) -> Result<&'a ProcessObservation, &'static str> {
    let observed = rows
        .iter()
        .filter(|r| r.address.port() == port)
        .cloned()
        .collect::<Vec<_>>();
    let pid = observed.first().ok_or("no_debug_port")?.owner_pid;
    VerifiedListener::from_complete_observation(port, pid, &observed)
        .map_err(|_| "identity_mismatch")?;
    processes
        .iter()
        .find(|p| p.pid == pid && p.same_user && p.executable == executable)
        .ok_or("identity_mismatch")
}

pub(crate) fn server_origins(
    processes: &[ProcessObservation],
    browser: &ProcessObservation,
    executable: &Path,
    rows: &[ListenerObservation],
) -> Result<Vec<AppOrigin>, &'static str> {
    let servers = processes
        .iter()
        .filter(|p| p.parent == Some(browser.pid) && p.same_user && p.executable == executable)
        .collect::<Vec<_>>();
    if servers.is_empty() || servers.len() > 8 {
        return Err("app_page_unavailable");
    }
    let mut ports = BTreeSet::new();
    for server in servers {
        for port in rows
            .iter()
            .filter(|r| r.owner_pid == server.pid)
            .map(|r| r.address.port())
            .collect::<BTreeSet<_>>()
        {
            let observed = rows
                .iter()
                .filter(|r| r.address.port() == port)
                .cloned()
                .collect::<Vec<_>>();
            if VerifiedListener::from_complete_observation(port, server.pid, &observed).is_ok() {
                ports.insert(port);
            }
        }
    }
    ports
        .into_iter()
        .map(|port| AppOrigin::from_verified_server_port(port).map_err(|_| "identity_mismatch"))
        .collect()
}

pub(crate) fn recheck(expected: &ProcessObservation) -> Result<(), &'static str> {
    if processes()?.iter().any(|p| p == expected) {
        Ok(())
    } else {
        Err("identity_mismatch")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn process(pid: u32, parent: Option<u32>, name: &str) -> ProcessObservation {
        ProcessObservation {
            pid,
            parent,
            executable: PathBuf::from(name),
            started: 1,
            same_user: true,
        }
    }
    fn listener(address: &str, pid: u32) -> ListenerObservation {
        ListenerObservation {
            address: address.parse().unwrap(),
            owner_pid: pid,
        }
    }
    #[test]
    fn windows_browser_requires_exact_executable_user_and_complete_listener() {
        let app = process(42, None, "Antigravity.exe");
        let rows = vec![listener("127.0.0.1:45678", 42)];
        assert_eq!(
            browser(&[app.clone()], &app.executable, 45678, &rows),
            Ok(&app)
        );
        let mut foreign = app.clone();
        foreign.same_user = false;
        assert!(browser(&[foreign], &app.executable, 45678, &rows).is_err());
        assert!(browser(
            &[app.clone()],
            Path::new("IDE/Antigravity.exe"),
            45678,
            &rows
        )
        .is_err());
        for extra in [listener("0.0.0.0:45678", 42), listener("[::1]:45678", 43)] {
            assert!(browser(
                &[app.clone()],
                &app.executable,
                45678,
                &[rows[0].clone(), extra]
            )
            .is_err());
        }
    }
    #[test]
    fn windows_pages_require_owned_child_server_and_reject_foreign_port_sharing() {
        let app = process(42, None, "Antigravity.exe");
        let server = process(43, Some(42), "resources/bin/language_server.exe");
        let rows = vec![listener("127.0.0.1:45679", 43)];
        let origins = server_origins(&[server.clone()], &app, &server.executable, &rows).unwrap();
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].as_str(), "https://127.0.0.1:45679");
        let mut foreign = server.clone();
        foreign.parent = Some(99);
        assert!(server_origins(&[foreign], &app, &server.executable, &rows).is_err());
        let foreign_rows = vec![rows[0].clone(), listener("[::1]:45679", 99)];
        assert!(
            server_origins(&[server.clone()], &app, &server.executable, &foreign_rows)
                .unwrap()
                .is_empty()
        );
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_tcp_table_observes_bound_socket_and_current_pid() {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        let observations = listeners().unwrap();
        let rows = observations
            .iter()
            .filter(|r| r.address.port() == address.port())
            .cloned()
            .collect::<Vec<_>>();
        assert!(VerifiedListener::from_complete_observation(
            address.port(),
            std::process::id(),
            &rows
        )
        .is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_process_snapshot_checks_user_and_canonical_executable() {
        const CHILD: &str = "AGY_TRANSLATION_PROCESS_FIXTURE";
        if std::env::var_os(CHILD).is_some() {
            let observed = processes().unwrap();
            let process = observed
                .iter()
                .find(|p| p.pid == std::process::id())
                .unwrap();
            assert!(process.same_user);
            assert!(process.started > 0);
            assert_eq!(
                process.executable,
                std::env::current_exe().unwrap().canonicalize().unwrap()
            );
            return;
        }
        // Rename only a synthetic test executable so the actual Windows name,
        // token-owner and canonical-path collection run together on CI.
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("Antigravity.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let result = std::process::Command::new(executable)
            .args(["--exact", "modules::app_metadata_windows::tests::windows_native_process_snapshot_checks_user_and_canonical_executable"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            result.status.success(),
            "Synthetic Windows process verification failed: {}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
