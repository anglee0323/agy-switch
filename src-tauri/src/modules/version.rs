use crate::modules::process;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs;
use std::path::PathBuf;

/// Antigravity 版本信息
#[derive(Debug, Clone)]
pub struct AntigravityVersion {
    pub short_version: String,
    pub bundle_version: String,
}

/// 从任意字符串中提取第一个语义化版本号 (X.Y.Z)
#[cfg(any(target_os = "linux", test))]
fn extract_semver(raw: &str) -> Option<String> {
    for token in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = token.trim_matches(|c: char| c == '"' || c == '\'' || c == '(' || c == ')');
        if t.is_empty() {
            continue;
        }
        let mut parts = t.split('.');
        let p1 = parts.next();
        let p2 = parts.next();
        let p3 = parts.next();
        if p1.is_some()
            && p2.is_some()
            && p3.is_some()
            && [p1.unwrap(), p2.unwrap(), p3.unwrap()]
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        {
            return Some(t.to_string());
        }
    }
    None
}

/// 检测 Antigravity 版本（跨平台）
pub fn get_antigravity_version(target_ide: Option<&str>) -> Result<AntigravityVersion, String> {
    // 1. 获取 Antigravity 可执行文件路径（复用现有功能）
    let exe_path = process::get_antigravity_executable_path(target_ide)
        .ok_or("Unable to locate Antigravity executable")?;

    // 2. 根据平台读取版本信息
    #[cfg(target_os = "macos")]
    {
        get_version_macos(&exe_path)
    }

    #[cfg(target_os = "windows")]
    {
        get_version_windows(&exe_path)
    }

    #[cfg(target_os = "linux")]
    {
        get_version_linux(&exe_path)
    }
}

/// macOS: 从 Info.plist 读取版本
#[cfg(target_os = "macos")]
fn get_version_macos(exe_path: &PathBuf) -> Result<AntigravityVersion, String> {
    use plist::Value;

    // exe_path 可能是 /Applications/Antigravity.app 或内部可执行文件
    // 需要找到 .app 目录
    let path_str = exe_path.to_string_lossy();
    let app_path = if let Some(idx) = path_str.find(".app") {
        PathBuf::from(&path_str[..idx + 4])
    } else {
        exe_path.clone()
    };

    let info_plist_path = app_path.join("Contents/Info.plist");
    if !info_plist_path.exists() {
        return Err(format!("Info.plist not found: {:?}", info_plist_path));
    }

    let content =
        fs::read(&info_plist_path).map_err(|e| format!("Failed to read Info.plist: {}", e))?;

    let plist: Value =
        plist::from_bytes(&content).map_err(|e| format!("Failed to parse Info.plist: {}", e))?;

    let dict = plist
        .as_dictionary()
        .ok_or("Info.plist is not a dictionary")?;

    let short_version = dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .ok_or("CFBundleShortVersionString not found")?;

    let bundle_version = dict
        .get("CFBundleVersion")
        .and_then(|v| v.as_string())
        .unwrap_or(short_version);

    Ok(AntigravityVersion {
        short_version: short_version.to_string(),
        bundle_version: bundle_version.to_string(),
    })
}

/// Windows: 从可执行文件元数据读取版本
#[cfg(target_os = "windows")]
fn get_version_windows(exe_path: &PathBuf) -> Result<AntigravityVersion, String> {
    use crate::utils::command::CommandExtWrapper;
    use std::process::Command;

    // Windows: 使用 PowerShell 读取文件版本信息
    let mut cmd = Command::new("powershell");
    let output = cmd
        .creation_flags_windows()
        .args([
            "-Command",
            &format!(
                "(Get-Item '{}').VersionInfo.FileVersion",
                exe_path.display()
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        return Err("Failed to read version from executable".to_string());
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if version.is_empty() {
        return Err("Version information not found in executable".to_string());
    }

    Ok(AntigravityVersion {
        short_version: version.clone(),
        bundle_version: version,
    })
}

/// Linux: 从 package.json 或 --version 参数读取
#[cfg(target_os = "linux")]
fn get_version_linux(exe_path: &PathBuf) -> Result<AntigravityVersion, String> {
    use std::process::Command;

    let exe_path = std::fs::canonicalize(exe_path).unwrap_or_else(|_| exe_path.clone());

    // 方法1 (优先): 尝试从安装目录的 package.json 读取，避免执行可执行文件意外拉起 GUI
    if let Some(parent) = exe_path.parent() {
        let package_json = parent.join("resources/app/package.json");
        if package_json.exists() {
            if let Ok(content) = fs::read_to_string(&package_json) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                        return Ok(AntigravityVersion {
                            short_version: version.to_string(),
                            bundle_version: version.to_string(),
                        });
                    }
                }
            }
        }
    }

    // 方法2 (兜底): 尝试执行 --version (仅在无法从 package.json 获取时执行)
    let mut command = Command::new(&exe_path);
    command.arg("--version");
    crate::modules::process::clean_appimage_env(&mut command);
    let output =
        crate::utils::process::output_with_timeout(command, std::time::Duration::from_secs(3));

    if let Ok(result) = output {
        if result.status.success() {
            let raw_version = String::from_utf8_lossy(&result.stdout).trim().to_string();
            if !raw_version.is_empty() {
                let version = extract_semver(&raw_version).unwrap_or_else(|| {
                    raw_version
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string()
                });
                return Ok(AntigravityVersion {
                    short_version: version.clone(),
                    bundle_version: raw_version,
                });
            }
        }
    }

    Err("Unable to determine Antigravity version on Linux".to_string())
}

/// Antigravity IDE 2.6.0 runs its agent through the built-in Antigravity extension,
/// which starts the shared App-data Hub (`agy --hub --app_data_dir=antigravity`) and
/// signs in with the App/agy credential. Its databases no longer select the account.
const IDE_SHARED_SIGN_IN_VERSION: &str = "2.6.0";

/// Whether the discovered Antigravity IDE uses the shared App/agy sign-in.
/// An undetected IDE or unreadable version keeps the legacy independent behavior.
pub fn ide_shares_app_sign_in() -> bool {
    process::get_antigravity_executable_path(Some("ide"))
        .and_then(|executable| ide_product_version(&executable))
        .is_some_and(|version| ide_version_shares_app_sign_in(&version))
}

fn ide_version_shares_app_sign_in(version: &str) -> bool {
    compare_version(version, IDE_SHARED_SIGN_IN_VERSION) != std::cmp::Ordering::Less
}

/// The IDE's Electron version differs from its product version, so read
/// `ideVersion` from product.json instead of the executable or bundle metadata.
fn ide_product_version(executable: &std::path::Path) -> Option<String> {
    let executable = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let text = executable.to_string_lossy();
    let mut candidates = Vec::new();
    if let Some(end) = text.find(".app/").or_else(|| text.ends_with(".app").then(|| text.len() - 4)) {
        candidates.push(PathBuf::from(&text[..end + 4]).join("Contents/Resources/app/product.json"));
    }
    // Windows/Linux executables sit beside resources/; Linux launchers may be in bin/.
    for root in executable.ancestors().skip(1).take(2) {
        candidates.push(root.join("resources/app/product.json"));
    }
    candidates.into_iter().find_map(|path| {
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        json.get("ideVersion")?.as_str().map(str::to_owned)
    })
}

/// 比较版本号
pub fn compare_version(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parts1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let parts2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    for i in 0..parts1.len().max(parts2.len()) {
        let p1 = parts1.get(i).unwrap_or(&0);
        let p2 = parts2.get(i).unwrap_or(&0);
        match p1.cmp(p2) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert_eq!(
            compare_version("1.16.5", "1.16.4"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_version("1.16.5", "1.16.5"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_version("1.16.4", "1.16.5"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_version("1.17.0", "1.16.5"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_version("2.0.0", "1.16.5"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn ide_shared_sign_in_starts_at_2_6_from_product_metadata() {
        assert!(!ide_version_shares_app_sign_in("2.5.5"));
        assert!(ide_version_shares_app_sign_in("2.6.0"));
        assert!(ide_version_shares_app_sign_in("2.10.1"));

        let root = tempfile::tempdir().unwrap();
        let product = |path: &std::path::Path, body: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        // macOS bundle: Electron reports 1.107.0, the IDE product is 2.6.0.
        let bundle = root.path().join("Antigravity IDE.app");
        product(&bundle.join("Contents/Resources/app/product.json"), r#"{"version":"1.107.0","ideVersion":"2.6.0"}"#);
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        assert_eq!(ide_product_version(&bundle).as_deref(), Some("2.6.0"));
        assert_eq!(ide_product_version(&bundle.join("Contents/MacOS")).as_deref(), Some("2.6.0"));

        // Windows/Linux installation and a Linux bin/ launcher.
        let install = root.path().join("Antigravity IDE");
        product(&install.join("resources/app/product.json"), r#"{"ideVersion":"2.5.5"}"#);
        assert_eq!(ide_product_version(&install.join("antigravity-ide")).as_deref(), Some("2.5.5"));
        assert_eq!(ide_product_version(&install.join("bin/antigravity-ide")).as_deref(), Some("2.5.5"));

        // Missing or legacy metadata keeps independent IDE switching.
        let legacy = root.path().join("legacy");
        product(&legacy.join("resources/app/product.json"), r#"{"version":"1.104.0"}"#);
        assert_eq!(ide_product_version(&legacy.join("antigravity-ide")), None);
        assert_eq!(ide_product_version(&root.path().join("missing/antigravity-ide")), None);
    }

    #[test]
    fn test_extract_semver_from_messy_output() {
        let raw = "1.107.0\n1504c8cc4b34dbfbb4a97ebe954b3da2b5634516\nx64";
        assert_eq!(extract_semver(raw), Some("1.107.0".to_string()));
    }
}
