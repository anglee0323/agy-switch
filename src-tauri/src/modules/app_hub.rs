//! The App-data Hub is an agy server, not an interactive CLI task. Observe it
//! before shutdown and keep its launch arguments in memory for the same switch.
use std::ffi::OsString;
#[cfg(any(target_os = "macos", test))]
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct Hub {
    pid: u32,
    executable: PathBuf,
    arguments: Vec<OsString>,
}

#[cfg(test)]
pub(crate) fn fixture() -> Hub {
    Hub { pid: 42, executable: "/fixture/agy".into(), arguments: vec![] }
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Arguments may contain CSRF metadata and must never be logged.
        f.debug_struct("Hub").field("pid", &self.pid).finish()
    }
}

#[cfg(any(target_os = "macos", test))]
fn app_hub_command(executable: &Path, expected: &Path, arguments: &[OsString]) -> bool {
    let words: Vec<_> = arguments.iter().map(|s| s.to_string_lossy()).collect();
    let mut data = Vec::new();
    for (i, word) in words.iter().enumerate() {
        if word.as_ref() == "--app_data_dir" {
            data.push(words.get(i + 1).map(|s| s.as_ref()));
        } else if let Some(value) = word.strip_prefix("--app_data_dir=") {
            data.push(Some(value));
        }
    }
    executable == expected
        && words.iter().filter(|s| s.as_ref() == "--hub").count() == 1
        && data == vec![Some("antigravity")]
}

#[cfg(target_os = "macos")]
pub(crate) fn snapshot() -> Result<Vec<Hub>, &'static str> {
    use super::app_metadata_macos as metadata;
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All);
    let pids: Vec<_> = system.processes().values()
        .filter(|p| p.name().to_str() == Some("agy")).map(|p| p.pid()).collect();
    if pids.is_empty() { return Ok(vec![]); }
    // refresh_processes does not populate argv. Request it only for agy, so
    // an empty default snapshot cannot misclassify a persistent idle Hub.
    system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&pids),
        sysinfo::ProcessRefreshKind::new().with_exe(sysinfo::UpdateKind::Always)
            .with_cmd(sysinfo::UpdateKind::Always));
    let expected = dirs::home_dir().ok_or("task_state_unknown")?
        .join(".gemini/bin/agy").canonicalize().map_err(|_| "task_state_unknown")?;
    let mut hubs = vec![];
    for pid in pids {
        let process = system.process(pid).ok_or("task_state_unknown")?;
        let executable = process.exe().and_then(|p| p.canonicalize().ok())
            .ok_or("task_state_unknown")?;
        if !app_hub_command(&executable, &expected, process.cmd()) || hubs.len() >= 4 {
            return Err("task_state_unknown");
        }
        let pid = process.pid().as_u32();
        metadata::verify_executable(pid, &expected).map_err(|_| "task_state_unknown")?;
        let owner = metadata::command("/bin/ps", &["-p".into(), pid.to_string(), "-o".into(), "uid=".into()])
            .map_err(|_| "task_state_unknown")?;
        if owner.parse::<u32>().ok() != Some(unsafe { libc::geteuid() }) {
            return Err("task_state_unknown");
        }
        hubs.push(Hub { pid, executable, arguments: process.cmd().iter().skip(1).cloned().collect() });
    }
    Ok(hubs)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn snapshot() -> Result<Vec<Hub>, &'static str> { Ok(vec![]) }

#[cfg(target_os = "macos")]
pub(crate) fn observe(hub: &Hub) -> Result<super::agent_activity::Activity, String> {
    super::app_identity::with_server_connection(hub.pid, &hub.executable, super::agent_activity::observe)
}

pub(crate) fn close(hubs: &[Hub], wait: bool) -> Result<(), &'static str> {
    #[cfg(target_os = "macos")]
    for hub in hubs {
        use super::{agent_activity::Activity, app_metadata_macos as metadata};
        if !metadata::pids(&["-u".into(), unsafe { libc::geteuid() }.to_string(), "-x".into(), "agy".into()])
            .map_err(|_| "process_unknown")?.contains(&hub.pid) { continue; }
        // Recheck after closing the App: remote work may have started meanwhile.
        if wait {
            match observe(hub).unwrap_or(Activity::Unknown) {
                Activity::Idle => {},
                Activity::Busy => return Err("waiting_task_finish"),
                Activity::Unknown => return Err("task_state_unknown"),
            }
        }
        metadata::verify_executable(hub.pid, &hub.executable).map_err(|_| "process_unknown")?;
        let output = std::process::Command::new("/bin/kill").args(["-TERM", &hub.pid.to_string()])
            .output().map_err(|_| "client_close_failed")?;
        if !output.status.success() { return Err("client_close_failed"); }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while metadata::pids(&["-u".into(), unsafe { libc::geteuid() }.to_string(), "-x".into(), "agy".into()])
            .map_err(|_| "process_unknown")?.contains(&hub.pid) {
            if std::time::Instant::now() >= deadline { return Err("client_close_failed"); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (hubs, wait);
    Ok(())
}

pub(crate) fn restart(hubs: &[Hub]) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if !hubs.is_empty() && !snapshot().map_err(|_| "restart_failed")?.is_empty() {
        return Err("restart_failed".into());
    }
    #[cfg(target_os = "macos")]
    for hub in hubs {
        let expected = dirs::home_dir().ok_or("restart_failed")?.join(".gemini/bin/agy")
            .canonicalize().map_err(|_| "restart_failed")?;
        if hub.executable != expected {
            return Err("restart_failed".into());
        }
        let mut child = std::process::Command::new(&hub.executable).args(&hub.arguments)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null()).spawn().map_err(|_| "restart_failed")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if child.try_wait().map_err(|_| "restart_failed")?.is_some() {
                return Err("restart_failed".into());
            }
            let pid = child.id();
            let ready = super::app_metadata_macos::command("/usr/sbin/lsof", &[
                "-nP".into(), "-a".into(), "-p".into(), pid.to_string(),
                "-iTCP".into(), "-sTCP:LISTEN".into(), "-Fpn".into(),
            ]).ok().and_then(|text| super::app_metadata_macos::parse_listener_fields(&text).ok())
                .is_some_and(|rows| rows.iter().any(|row| {
                    super::app_metadata_macos::listener(row.address.port(), pid).is_ok()
                }));
            if ready { break; }
            if std::time::Instant::now() >= deadline { return Err("restart_failed".into()); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = hubs;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_verified_app_data_hubs_are_supported() {
        let path = Path::new("/fixture/agy");
        let args = |s: &str| s.split_whitespace().map(OsString::from).collect::<Vec<_>>();
        assert!(app_hub_command(path, path, &args("agy --hub --app_data_dir antigravity")));
        assert!(app_hub_command(path, path, &args("agy --app_data_dir=antigravity --hub")));
        for s in ["agy", "agy --remote-control", "agy --hub", "agy --hub --app_data_dir antigravity-cli", "agy --hub --app_data_dir antigravity --app_data_dir other", "agy --hub --hub --app_data_dir antigravity"] {
            assert!(!app_hub_command(path, path, &args(s)));
        }
        assert!(!app_hub_command(Path::new("/unrelated/agy"), path, &args("agy --hub --app_data_dir antigravity")));
        let hub = Hub { pid: 42, executable: path.into(), arguments: args("--csrf_token fixture-secret") };
        assert!(!format!("{hub:?}").contains("fixture-secret"));
    }
}
