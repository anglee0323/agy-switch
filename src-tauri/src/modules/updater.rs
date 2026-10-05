use serde::{Deserialize, Serialize};
const RELEASE_ROOT: &str = "https://github.com/anglee0323/agy-switch/releases/tag/";
const API_URL: &str = "https://api.github.com/repos/anglee0323/agy-switch/releases/latest";

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
}
fn version(value: &str) -> Option<[u64; 3]> {
    let clean = value.strip_prefix('v').unwrap_or(value);
    let parts = clean.split('.').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) { return None; }
    Some([parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?])
}
fn evaluate(current: &str, release: Release) -> Result<UpdateInfo, String> {
    let latest = version(&release.tag_name).ok_or("invalid_release")?;
    let installed = version(current).ok_or("invalid_release")?;
    if release.draft || release.prerelease || release.html_url != format!("{RELEASE_ROOT}{}", release.tag_name) {
        return Err("invalid_release".into());
    }
    Ok(UpdateInfo { current_version: current.into(), latest_version: release.tag_name,
        has_update: latest > installed, release_url: release.html_url })
}

/// Read public release metadata only; no credentials, download or installer execution.
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("agy-switch/", env!("CARGO_PKG_VERSION")))
        .build().map_err(|_| "update_network_failed")?;
    let response = client.get(API_URL).header("Accept", "application/vnd.github+json")
        .send().await.map_err(|_| "update_network_failed")?;
    if !response.status().is_success() { return Err("update_network_failed".into()); }
    let body = response.text().await.map_err(|_| "update_network_failed")?;
    if body.len() > 1_000_000 { return Err("invalid_release".into()); }
    let release = serde_json::from_str(&body).map_err(|_| "invalid_release")?;
    evaluate(env!("CARGO_PKG_VERSION"), release)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release { Release { tag_name: tag.into(), html_url: format!("{RELEASE_ROOT}{tag}"), draft: false, prerelease: false } }
    #[test]
    fn only_newer_stable_versions_trigger_updates() {
        assert!(evaluate("4.7.8", release("v4.7.9")).unwrap().has_update);
        assert!(evaluate("4.7.8", release("v4.10.0")).unwrap().has_update);
        for tag in ["v4.7.8", "v4.7.7"] { assert!(!evaluate("4.7.8", release(tag)).unwrap().has_update); }
    }
    #[test]
    fn malformed_versions_and_untrusted_release_links_are_rejected() {
        for tag in ["v4.7", "v4.7.9-beta", "vv4.7.9", "v4.7.9/path", "4.7.99999999999999999999999"] { assert!(evaluate("4.7.8", release(tag)).is_err()); }
        for url in ["file:///tmp/installer", "https://evil.invalid/releases/tag/v4.7.9", "https://github.com/another/project/releases/tag/v4.7.9"] {
            let mut r = release("v4.7.9"); r.html_url = url.into(); assert!(evaluate("4.7.8", r).is_err());
        }
        for prerelease in [true, false] { let mut r = release("v4.7.9"); r.draft = !prerelease; r.prerelease = prerelease; assert!(evaluate("4.7.8", r).is_err()); }
    }
}

#[derive(Default)]
pub struct UpdateRuntime(pub tokio::sync::Mutex<()>);

#[derive(Clone, Serialize)]
pub struct UpdateProgress {
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
}

fn trusted_download(version: &str, url: &str) -> bool {
    if self::version(version).is_none() { return false; }
    let platform = if cfg!(target_os = "macos") { "macos-arm64.app.tar.gz" }
        else if cfg!(target_os = "windows") { "windows-x64-setup.exe" }
        else { "linux-amd64.deb" };
    url == format!("https://github.com/anglee0323/agy-switch/releases/download/v{version}/agy-switch-{version}-{platform}")
}

/// Only a user-triggered command can download and install. The caller supplies no URL or path.
pub async fn download_and_install(app: tauri::AppHandle, expected_version: String,
    progress: tauri::ipc::Channel<UpdateProgress>) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_updater::UpdaterExt;
    let runtime = app.state::<UpdateRuntime>();
    let _guard = runtime.0.try_lock().map_err(|_| "update_busy")?;
    let metadata = check_for_updates().await?;
    if !metadata.has_update || metadata.latest_version.trim_start_matches('v') != expected_version.trim_start_matches('v') {
        return Err("update_changed".into());
    }
    let updater = app.updater_builder().timeout(std::time::Duration::from_secs(180))
        .build().map_err(|_| "update_unavailable")?;
    let mut update = updater.check().await.map_err(|_| "update_unavailable")?.ok_or("update_changed")?;
    if update.version != metadata.latest_version.trim_start_matches('v') || !trusted_download(&update.version, update.download_url.as_str()) {
        return Err("invalid_release".into());
    }
    if !signed_filename_matches(&update.signature, &update.download_url) { return Err("invalid_release".into()); }
    update.timeout = Some(std::time::Duration::from_secs(300));
    let mut downloaded = 0;
    let bytes = update.download(|chunk, total| {
        downloaded += chunk as u64;
        let _ = progress.send(UpdateProgress { stage: "downloading", downloaded, total });
    }, || {}).await.map_err(|_| "update_download_failed")?;
    let _ = progress.send(UpdateProgress { stage: "installing", downloaded, total: Some(downloaded) });
    #[cfg(target_os = "macos")]
    install_macos(&bytes, &update.version)?;
    #[cfg(not(target_os = "macos"))]
    update.install(&bytes).map_err(|_| "update_install_failed")?;
    #[cfg(not(target_os = "windows"))]
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

/// Keep the existing app in place unless the signed update also passes macOS trust checks.
/// No quarantine removal, administrator shell or security-policy changes are performed.
#[cfg(target_os = "macos")]
fn install_macos(bytes: &[u8], expected_version: &str) -> Result<(), String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|_| "update_install_failed")?;
    let app_path = exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).ok_or("update_install_failed")?;
    if app_path.extension().and_then(|e| e.to_str()) != Some("app") { return Err("update_install_failed".into()); }
    let parent = app_path.parent().ok_or("update_install_failed")?;
    // Same volume makes replacement and rollback atomic; lack of write access leaves the app untouched.
    let staging = tempfile::Builder::new().prefix(".agy-update-").tempdir_in(parent).map_err(|_| "update_install_failed")?;
    let archive = staging.path().join("update.tar.gz");
    std::fs::write(&archive, bytes).map_err(|_| "update_install_failed")?;
    let extracted = staging.path().join("payload");
    std::fs::create_dir(&extracted).map_err(|_| "update_install_failed")?;
    let status = Command::new("/usr/bin/tar").args(["-xzf"]).arg(&archive).arg("-C").arg(&extracted).status().map_err(|_| "update_install_failed")?;
    if !status.success() { return Err("update_install_failed".into()); }
    let candidate = extracted.join("agy-switch.app");
    let info: plist::Value = plist::from_file(candidate.join("Contents/Info.plist")).map_err(|_| "invalid_release")?;
    let info = info.as_dictionary().ok_or("invalid_release")?;
    if info.get("CFBundleIdentifier").and_then(plist::Value::as_string) != Some("com.lbjlaq.antigravity-tools-lite")
        || info.get("CFBundleShortVersionString").and_then(plist::Value::as_string) != Some(expected_version) {
        return Err("invalid_release".into());
    }
    for (program, args) in [("/usr/bin/codesign", vec!["--verify", "--deep", "--strict"]),
        ("/usr/sbin/spctl", vec!["--assess", "--type", "execute"])] {
        if !Command::new(program).args(args).arg(&candidate).output().map_err(|_| "update_install_failed")?.status.success() {
            return Err("update_mac_trust_required".into());
        }
    }
    let backup = staging.path().join("previous.app");
    std::fs::rename(app_path, &backup).map_err(|_| "update_install_failed")?;
    if std::fs::rename(&candidate, app_path).is_err() {
        if std::fs::rename(&backup, app_path).is_err() {
            // Preserve the recoverable original bundle instead of deleting it with the temporary directory.
            let _ = staging.keep();
            return Err("update_restore_failed".into());
        }
        return Err("update_install_failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod download_tests {
    use super::*;
    #[test]
    fn updater_payload_is_pinned_to_repository_version_and_platform() {
        let suffix = if cfg!(target_os = "macos") { "macos-arm64.app.tar.gz" } else if cfg!(target_os = "windows") { "windows-x64-setup.exe" } else { "linux-amd64.deb" };
        let valid = format!("https://github.com/anglee0323/agy-switch/releases/download/v4.9.0/agy-switch-4.9.0-{suffix}");
        assert!(trusted_download("4.9.0", &valid));
        for changed in [valid.replace("anglee0323", "attacker"), valid.replace("v4.9.0", "v4.8.1"), format!("{valid}?redirect=evil"), valid.replace("https:", "http:"), "file:///tmp/update".into()] { assert!(!trusted_download("4.9.0", &changed)); }
        assert!(!trusted_download("4.9.0/path", &valid));
    }
}

// The authenticated filename binds the signature to this version and platform even when
// standalone signer output has no separate signed-version field. Download verifies its signature.
fn signed_filename_matches(signature: &str, url: &url::Url) -> bool {
    use base64::Engine;
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(signature) else { return false; };
    let Ok(text) = std::str::from_utf8(&bytes) else { return false; };
    let Some(comment) = text.lines().nth(2).and_then(|line| line.strip_prefix("trusted comment: ")) else { return false; };
    let expected = url.path_segments().and_then(|mut segments| segments.next_back());
    let names = comment.split('\t').filter_map(|field| field.strip_prefix("file:")).collect::<Vec<_>>();
    names.len() == 1 && Some(names[0]) == expected
}

#[cfg(test)]
mod signature_metadata_tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn signed_filename_cannot_relabel_an_older_or_different_platform_package() {
        let url = url::Url::parse("https://github.com/anglee0323/agy-switch/releases/download/v4.9.0/agy-switch-4.9.0-windows-x64-setup.exe").unwrap();
        let signed_comment = |filename: &str| base64::engine::general_purpose::STANDARD.encode(format!("untrusted comment: fixture\nfixture\ntrusted comment: timestamp:1\tfile:{filename}\nfixture\n"));
        assert!(signed_filename_matches(&signed_comment("agy-switch-4.9.0-windows-x64-setup.exe"), &url));
        for filename in ["agy-switch-4.8.1-windows-x64-setup.exe", "agy-switch-4.9.0-linux-amd64.deb", "agy-switch-4.9.0-windows-x64-setup.exe\tfile:other"] {
            assert!(!signed_filename_matches(&signed_comment(filename), &url));
        }
        assert!(!signed_filename_matches("invalid-base64", &url));
    }
}
