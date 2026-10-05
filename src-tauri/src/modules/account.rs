use serde::Serialize;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{Account, AccountIndex, AccountSummary, QuotaData, TokenData};
use crate::modules;
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global per-account lock to prevent concurrent write collisions on the same account JSON file
static ACCOUNT_FILE_LOCKS: Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn get_account_lock(account_id: &str) -> Arc<Mutex<()>> {
    let mut locks = ACCOUNT_FILE_LOCKS.lock().unwrap();
    locks
        .entry(account_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex as StdMutex;

    // Global mutex to prevent concurrent test execution
    static TEST_MUTEX: Lazy<StdMutex<()>> = Lazy::new(|| StdMutex::new(()));

    struct TestDataDir {
        path: PathBuf,
    }

    impl TestDataDir {
        fn new() -> Self {
            let temp_path = std::env::temp_dir().join(format!(
                "antigravity_test_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ));
            fs::create_dir_all(&temp_path).expect("Failed to create temp dir");

            Self { path: temp_path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TestDataDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Helper to write corrupted content to accounts.json
    fn write_corrupted_index(path: &PathBuf, content: &[u8]) {
        let index_path = path.join("accounts.json");
        fs::write(&index_path, content).expect("Failed to write corrupted index");
    }

    /// Helper to create a valid account file in accounts/ directory
    fn create_account_file(path: &PathBuf, account_id: &str, email: &str) {
        let accounts_dir = path.join("accounts");
        fs::create_dir_all(&accounts_dir).expect("Failed to create accounts dir");

        let account = Account::new(
            account_id.to_string(),
            email.to_string(),
            TokenData::new(
                "test_access_token".to_string(),
                "test_refresh_token".to_string(),
                3600,
                Some(email.to_string()),
                None,
                None,
                true,
                None,
            ),
        );

        let content = serde_json::to_string_pretty(&account).expect("Failed to serialize account");
        let account_path = accounts_dir.join(format!("{}.json", account_id));
        fs::write(&account_path, content).expect("Failed to write account file");
    }

    #[test]
    fn test_load_account_index_with_bom_prefix() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // UTF-8 BOM followed by valid JSON
        let bom = [0xEF, 0xBB, 0xBF];
        let json = r#"{"version":"2.0","accounts":[],"current_account_id":null}"#;
        let mut content = Vec::new();
        content.extend_from_slice(&bom);
        content.extend_from_slice(json.as_bytes());

        write_corrupted_index(dir.path(), &content);

        let result = load_account_index_in_dir(dir.path());

        // New behavior: BOM is stripped and JSON parses successfully
        assert!(
            result.is_ok(),
            "BOM should be stripped and JSON should parse: {:?}",
            result
        );
        let index = result.unwrap();
        assert!(index.accounts.is_empty());
        println!("BOM case: successfully loaded index after sanitization");
    }

    #[test]
    fn test_load_account_index_with_nul_prefix() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // NUL byte prefix followed by valid JSON
        let nul = [0x00];
        let json = r#"{"version":"2.0","accounts":[],"current_account_id":null}"#;
        let mut content = Vec::new();
        content.extend_from_slice(&nul);
        content.extend_from_slice(json.as_bytes());

        write_corrupted_index(dir.path(), &content);

        let result = load_account_index_in_dir(dir.path());

        // New behavior: NUL bytes are stripped and JSON parses successfully
        assert!(
            result.is_ok(),
            "NUL prefix should be stripped and JSON should parse: {:?}",
            result
        );
        let index = result.unwrap();
        assert!(index.accounts.is_empty());
        println!("NUL prefix case: successfully loaded index after sanitization");
    }

    #[test]
    fn test_load_account_index_with_garbage_content() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Non-JSON garbage content - should trigger recovery
        write_corrupted_index(dir.path(), b"\0\0not json");

        let result = load_account_index_in_dir(dir.path());

        // New behavior: garbage content triggers recovery, returns empty index
        assert!(
            result.is_ok(),
            "Garbage content should trigger recovery and return Ok: {:?}",
            result
        );
        let index = result.unwrap();
        assert!(
            index.accounts.is_empty(),
            "Recovered index should be empty when no account files exist"
        );
        println!("Garbage content case: successfully recovered to empty index");
    }

    #[test]
    fn test_load_account_index_with_empty_file() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Empty file
        write_corrupted_index(dir.path(), b"");

        let result = load_account_index_in_dir(dir.path());

        // Current behavior: empty file returns new empty index
        assert!(result.is_ok());
        let index = result.unwrap();
        assert!(index.accounts.is_empty());
    }

    #[test]
    fn test_load_account_index_with_whitespace_only() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Whitespace-only file
        write_corrupted_index(dir.path(), b"   \n\t  ");

        let result = load_account_index_in_dir(dir.path());

        // Current behavior: whitespace-only file returns new empty index
        assert!(result.is_ok());
        let index = result.unwrap();
        assert!(index.accounts.is_empty());
    }

    #[test]
    fn test_missing_index_with_existing_accounts() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Create accounts directory with account files but NO accounts.json index
        create_account_file(dir.path(), "test-id-1", "user1@example.com");
        create_account_file(dir.path(), "test-id-2", "user2@example.com");

        // accounts.json does not exist
        let index_path = dir.path().join("accounts.json");
        assert!(!index_path.exists());

        // Load account index - should recover from accounts directory
        let result = load_account_index_in_dir(dir.path());
        assert!(result.is_ok(), "Should recover from accounts directory");
        let index = result.unwrap();
        assert_eq!(
            index.accounts.len(),
            2,
            "Index should have 2 accounts recovered from accounts directory"
        );

        // Verify recovered accounts have correct data
        let emails: Vec<_> = index.accounts.iter().map(|s| s.email.clone()).collect();
        assert!(emails.contains(&"user1@example.com".to_string()));
        assert!(emails.contains(&"user2@example.com".to_string()));

        // Verify account files still exist
        let accounts_dir = dir.path().join("accounts");
        let account_files: Vec<_> = fs::read_dir(&accounts_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            .collect();
        assert_eq!(
            account_files.len(),
            2,
            "Account files should still exist on disk"
        );

        println!(
            "Missing index with existing accounts: successfully recovered {} accounts",
            index.accounts.len()
        );
    }

    #[test]
    fn test_save_account_index_roundtrip() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Build an AccountIndex with 2 accounts
        let now = chrono::Utc::now().timestamp();
        let index = AccountIndex {
            version: "2.0".to_string(),
            accounts: vec![
                AccountSummary {
                    id: "acc-1".to_string(),
                    email: "user1@example.com".to_string(),
                    name: Some("User One".to_string()),
                    disabled: false,
                    protected_models: HashSet::new(),
                    created_at: now,
                    last_used: now,
                },
                AccountSummary {
                    id: "acc-2".to_string(),
                    email: "user2@example.com".to_string(),
                    name: None,
                    disabled: true,
                    protected_models: HashSet::new(),
                    created_at: now - 100,
                    last_used: now - 50,
                },
            ],
            current_account_id: Some("acc-1".to_string()),
            current_target_ide: None,
        };

        // Save the index
        save_account_index_in_dir(dir.path(), &index).expect("Failed to save account index");

        // Load it back
        let loaded = load_account_index_in_dir(dir.path()).expect("Failed to load account index");

        // Assert it matches
        assert_eq!(loaded.accounts.len(), 2, "Should have 2 accounts");
        assert_eq!(
            loaded.current_account_id,
            Some("acc-1".to_string()),
            "current_account_id should match"
        );

        // Check first account
        let acc1 = loaded
            .accounts
            .iter()
            .find(|a| a.id == "acc-1")
            .expect("acc-1 should exist");
        assert_eq!(acc1.email, "user1@example.com");
        assert_eq!(acc1.name, Some("User One".to_string()));
        assert!(!acc1.disabled);

        // Check second account
        let acc2 = loaded
            .accounts
            .iter()
            .find(|a| a.id == "acc-2")
            .expect("acc-2 should exist");
        assert_eq!(acc2.email, "user2@example.com");
        assert_eq!(acc2.name, None);
        assert!(acc2.disabled);

        println!(
            "save_account_index roundtrip: successfully saved and loaded index with {} accounts",
            loaded.accounts.len()
        );
    }

    #[test]
    fn test_set_current_account_id_with_target() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();
        std::env::set_var("ABV_DATA_DIR", dir.path());

        // Create a dummy account index with some accounts
        let now = chrono::Utc::now().timestamp();
        let index = AccountIndex {
            version: "2.0".to_string(),
            accounts: vec![AccountSummary {
                id: "acc-1".to_string(),
                email: "user1@example.com".to_string(),
                name: Some("User One".to_string()),
                disabled: false,
                protected_models: HashSet::new(),
                created_at: now,
                last_used: now,
            }],
            current_account_id: None,
            current_target_ide: None,
        };
        save_account_index_in_dir(dir.path(), &index).unwrap();

        // 1. Call set_current_account_id_with_target with Some("agy")
        set_current_account_id_with_target("acc-1", Some("agy")).unwrap();

        // Load back and verify
        let index = load_account_index_in_dir(dir.path()).unwrap();
        assert_eq!(index.current_account_id, Some("acc-1".to_string()));
        assert_eq!(index.current_target_ide, Some("agy".to_string()));

        // 2. Call set_current_account_id (which sets target to None)
        set_current_account_id("acc-1").unwrap();

        // Load back and verify target is None
        let index = load_account_index_in_dir(dir.path()).unwrap();
        assert_eq!(index.current_account_id, Some("acc-1".to_string()));
        assert_eq!(index.current_target_ide, None);

        // Clean up environment variable
        std::env::remove_var("ABV_DATA_DIR");
    }

    #[test]
    fn test_backup_created_on_parse_failure() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        // Create a valid account file
        create_account_file(dir.path(), "recovered-acc", "recovered@example.com");

        // Create corrupt accounts.json with garbage (non-empty)
        let garbage_content = b"this is not valid json { broken";
        write_corrupted_index(dir.path(), garbage_content);

        // Verify accounts.json exists and is corrupt
        let index_path = dir.path().join("accounts.json");
        assert!(index_path.exists(), "accounts.json should exist");

        // Call load_account_index to trigger recovery and backup creation
        let recovered =
            load_account_index_in_dir(dir.path()).expect("Should recover from accounts");
        assert_eq!(recovered.accounts.len(), 1, "Should recover 1 account");
        assert_eq!(recovered.accounts[0].email, "recovered@example.com");
        assert_eq!(
            recovered.current_account_id,
            Some("recovered-acc".to_string())
        );

        // Assert a backup file exists with prefix "accounts.json.corrupt-"
        let data_dir = dir.path();
        let backup_files: Vec<_> = fs::read_dir(data_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |name| name.starts_with("accounts.json.corrupt-"))
            })
            .collect();

        assert_eq!(backup_files.len(), 1, "Should have exactly one backup file");

        // Verify backup contains the original garbage content
        let backup_content =
            fs::read(&backup_files[0].path()).expect("Should be able to read backup file");
        assert_eq!(
            backup_content, garbage_content,
            "Backup should contain original corrupt content"
        );

        println!("Backup creation on parse failure: successfully created backup");
    }

    #[test]
    fn test_load_account_with_trailing_characters() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let dir = TestDataDir::new();

        create_account_file(dir.path(), "corrupt-tail-acc", "tail@example.com");
        let account_path = dir.path().join("accounts").join("corrupt-tail-acc.json");

        // Append trailing '}' to simulate Issue #3345
        let mut raw = fs::read_to_string(&account_path).unwrap();
        raw.push('}');
        fs::write(&account_path, &raw).unwrap();

        // Load account should successfully self-heal and return valid Account
        let loaded =
            load_account_at_path(&account_path).expect("Should self-heal trailing characters");
        assert_eq!(loaded.id, "corrupt-tail-acc");
        assert_eq!(loaded.email, "tail@example.com");

        // Verify the file was cleaned and re-written as valid JSON
        let healed_raw = fs::read_to_string(&account_path).unwrap();
        let regular_parse: Result<Account, _> = serde_json::from_str(&healed_raw);
        assert!(
            regular_parse.is_ok(),
            "Healed file should be standard valid JSON"
        );
    }
}

/// Global account write lock to prevent corruption during concurrent operations
static ACCOUNT_INDEX_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub(crate) fn lock_account_file_updates() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))
}

// ... existing constants ...
const DATA_DIR: &str = ".antigravity_tools";
const ACCOUNTS_INDEX: &str = "accounts.json";
const ACCOUNTS_DIR: &str = "accounts";

/// Get data directory path
pub fn get_data_dir() -> Result<PathBuf, String> {
    // [NEW] Support custom data directory via environment variable
    if let Ok(env_path) = std::env::var("ABV_DATA_DIR") {
        if !env_path.trim().is_empty() {
            let data_dir = PathBuf::from(env_path);
            if !data_dir.exists() {
                fs::create_dir_all(&data_dir)
                    .map_err(|e| format!("failed_to_create_custom_data_dir: {}", e))?;
            }
            return Ok(data_dir);
        }
    }

    let home = dirs::home_dir().ok_or("failed_to_get_home_dir")?;
    let data_dir = home.join(DATA_DIR);

    // Ensure directory exists
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).map_err(|e| format!("failed_to_create_data_dir: {}", e))?;
    }

    Ok(data_dir)
}

/// Get accounts directory path
pub fn get_accounts_dir() -> Result<PathBuf, String> {
    let data_dir = get_data_dir()?;
    let accounts_dir = data_dir.join(ACCOUNTS_DIR);

    if !accounts_dir.exists() {
        fs::create_dir_all(&accounts_dir)
            .map_err(|e| format!("failed_to_create_accounts_dir: {}", e))?;
    }

    Ok(accounts_dir)
}

/// Load account index from a specific directory (internal helper)
fn load_account_index_in_dir(data_dir: &PathBuf) -> Result<AccountIndex, String> {
    let index_path = data_dir.join(ACCOUNTS_INDEX);

    if !index_path.exists() {
        crate::modules::logger::log_warn(
            "Account index file not found, attempting recovery from accounts directory",
        );
        let recovered = rebuild_index_from_accounts_in_dir(data_dir)?;
        try_save_recovered_index(data_dir, &index_path, &recovered, None)?;
        return Ok(recovered);
    }

    let raw_content =
        fs::read(&index_path).map_err(|e| format!("failed_to_read_account_index: {}", e))?;

    // If file is empty, attempt recovery
    if raw_content.is_empty() {
        crate::modules::logger::log_warn(
            "Account index is empty, attempting recovery from accounts directory",
        );
        let recovered = rebuild_index_from_accounts_in_dir(data_dir)?;
        try_save_recovered_index(data_dir, &index_path, &recovered, None)?;
        return Ok(recovered);
    }

    // Sanitize content: strip BOM and leading NUL bytes
    let sanitized = sanitize_index_content(&raw_content);

    // If sanitized content is empty/whitespace, attempt recovery
    if sanitized.trim().is_empty() {
        crate::modules::logger::log_warn(
            "Account index is empty after sanitization, attempting recovery from accounts directory",
        );
        let recovered = rebuild_index_from_accounts_in_dir(data_dir)?;
        try_save_recovered_index(data_dir, &index_path, &recovered, None)?;
        return Ok(recovered);
    }

    // Try to parse sanitized content
    match serde_json::from_str::<AccountIndex>(&sanitized) {
        Ok(index) => {
            crate::modules::logger::log_info(&format!(
                "Successfully loaded index with {} accounts",
                index.accounts.len()
            ));
            Ok(index)
        }
        Err(parse_err) => {
            crate::modules::logger::log_error(&format!(
                "Failed to parse account index: {}. Attempting recovery from accounts directory",
                parse_err
            ));
            let recovered = rebuild_index_from_accounts_in_dir(data_dir)?;
            try_save_recovered_index(data_dir, &index_path, &recovered, Some(&raw_content))?;
            Ok(recovered)
        }
    }
}

/// Save account index to a specific directory (internal helper)
fn save_account_index_in_dir(data_dir: &PathBuf, index: &AccountIndex) -> Result<(), String> {
    let index_path = data_dir.join(ACCOUNTS_INDEX);

    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("failed_to_serialize_account_index: {}", e))?;

    crate::utils::fs::write_atomic(&index_path, content.as_bytes())
        .map_err(|e| format!("failed_to_save_account_index: {}", e))
}

/// Rebuild AccountIndex by scanning accounts/*.json files in specific directory
fn rebuild_index_from_accounts_in_dir(data_dir: &PathBuf) -> Result<AccountIndex, String> {
    let accounts_dir = data_dir.join(ACCOUNTS_DIR);
    let mut summaries = Vec::new();

    if accounts_dir.exists() {
        if let Ok(entries) = fs::read_dir(&accounts_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Some(account_id) = path.file_stem().and_then(|s| s.to_str()) {
                        match load_account_at_path(&path) {
                            Ok(account) => {
                                summaries.push(AccountSummary {
                                    id: account.id,
                                    email: account.email,
                                    name: account.name,
                                    disabled: account.disabled,
                                    protected_models: account.protected_models,
                                    created_at: account.created_at,
                                    last_used: account.last_used,
                                });
                            }
                            Err(e) => {
                                crate::modules::logger::log_warn(&format!(
                                    "Failed to load account {} during recovery: {}",
                                    account_id, e
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by last_used desc, then by email for deterministic order
    summaries.sort_by(|a, b| {
        b.last_used
            .cmp(&a.last_used)
            .then_with(|| a.email.cmp(&b.email))
    });

    let current_account_id = summaries.first().map(|s| s.id.clone());

    crate::modules::logger::log_info(&format!(
        "Rebuilt index from accounts directory: {} accounts recovered",
        summaries.len()
    ));

    Ok(AccountIndex {
        version: "2.0".to_string(),
        accounts: summaries,
        current_account_id,
        current_target_ide: None,
    })
}

/// Load account from a specific path with self-healing support for trailing characters/corrupted suffixes
fn load_account_at_path(account_path: &PathBuf) -> Result<Account, String> {
    let content = fs::read_to_string(account_path)
        .map_err(|e| format!("failed_to_read_account_data: {}", e))?;

    match serde_json::from_str::<Account>(&content) {
        Ok(account) => Ok(account),
        Err(e) => {
            let err_msg = e.to_string();
            // Self-healing attempt: handle trailing characters / extra closing brackets
            if err_msg.contains("trailing characters")
                || err_msg.contains("trailing comma")
                || err_msg.contains("trailing")
            {
                let mut de = serde_json::Deserializer::from_str(&content);
                if let Ok(account) = serde::Deserialize::deserialize(&mut de) {
                    crate::modules::logger::log_warn(&format!(
                        "Self-healing account JSON at {:?}: recovered valid account data from trailing characters, saving clean file",
                        account_path
                    ));
                    let _ = save_account_at_path(account_path, &account);
                    return Ok(account);
                }
            }
            Err(format!("failed_to_parse_account_data: {}", err_msg))
        }
    }
}

/// Load account index with recovery support
pub fn load_account_index() -> Result<AccountIndex, String> {
    let data_dir = get_data_dir()?;
    load_account_index_in_dir(&data_dir)
}

/// Sanitize index file content by stripping BOM and leading NUL bytes
fn sanitize_index_content(raw: &[u8]) -> String {
    // Skip UTF-8 BOM if present
    let without_bom = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &raw[3..]
    } else {
        raw
    };

    // Skip leading NUL bytes
    let without_nul = without_bom
        .iter()
        .skip_while(|&&b| b == 0x00)
        .copied()
        .collect::<Vec<u8>>();

    // Convert to string (lossy - invalid UTF-8 sequences become replacement chars)
    String::from_utf8_lossy(&without_nul).into_owned()
}

/// Best-effort save of recovered index without deadlocking
fn try_save_recovered_index(
    data_dir: &PathBuf,
    _index_path: &PathBuf,
    index: &AccountIndex,
    corrupt_content: Option<&[u8]>,
) -> Result<(), String> {
    // Backup corrupt file if content provided
    if let Some(content) = corrupt_content {
        let timestamp = chrono::Utc::now().timestamp();
        let backup_name = format!("accounts.json.corrupt-{}-{}", timestamp, Uuid::new_v4());
        let backup_path = data_dir.join(&backup_name);
        if let Err(e) = crate::utils::fs::write_atomic(&backup_path, content) {
            crate::modules::logger::log_warn(&format!(
                "Failed to backup corrupt index to {}: {}",
                backup_name, e
            ));
        } else {
            crate::modules::logger::log_info(&format!(
                "Backed up corrupt index to {}",
                backup_name
            ));
        }
    }

    // Try to acquire lock without blocking - if we can't get it, skip saving
    match ACCOUNT_INDEX_LOCK.try_lock() {
        Ok(_guard) => {
            if let Err(e) = save_account_index_in_dir(data_dir, index) {
                crate::modules::logger::log_warn(&format!(
                    "Failed to save recovered index: {}. Will retry on next load.",
                    e
                ));
            } else {
                crate::modules::logger::log_info("Successfully saved recovered index");
            }
        }
        Err(_) => {
            crate::modules::logger::log_warn(
                "Could not acquire lock to save recovered index. Will retry on next load.",
            );
        }
    }

    Ok(())
}

/// Save account index (atomic write)
pub fn save_account_index(index: &AccountIndex) -> Result<(), String> {
    let data_dir = get_data_dir()?;
    save_account_index_in_dir(&data_dir, index)
}

/// Load account data
pub fn load_account(account_id: &str) -> Result<Account, String> {
    let accounts_dir = get_accounts_dir()?;
    let account_path = accounts_dir.join(format!("{}.json", account_id));
    load_account_at_path(&account_path)
}

/// Save account data at specific file path (thread-safe and atomic)
fn save_account_at_path(account_path: &PathBuf, account: &Account) -> Result<(), String> {
    let _lock = get_account_lock(&account.id);
    let _guard = _lock.lock().unwrap();

    let content = serde_json::to_string_pretty(account)
        .map_err(|e| format!("failed_to_serialize_account_data: {}", e))?;

    crate::utils::fs::write_atomic(account_path, content.as_bytes())
        .map_err(|e| format!("failed_to_save_account_file: {}", e))
}

/// Save account data (thread-safe and atomic)
pub fn save_account(account: &Account) -> Result<(), String> {
    let accounts_dir = get_accounts_dir()?;
    let account_path = accounts_dir.join(format!("{}.json", account.id));
    save_account_at_path(&account_path, account)
}

/// List all accounts
pub fn list_accounts() -> Result<Vec<Account>, String> {
    crate::modules::logger::log_info("Listing accounts...");
    let index = load_account_index()?;
    let mut accounts = Vec::new();

    for summary in &index.accounts {
        match load_account(&summary.id) {
            Ok(account) => accounts.push(account),
            Err(e) => {
                crate::modules::logger::log_error(&format!(
                    "Failed to load account {}: {}",
                    summary.id, e
                ));
                // [FIX #929] Removed auto-repair logic.
                // We no longer silently delete account IDs from the index if the file is missing.
                // This prevents account loss during version upgrades or temporary FS issues.
            }
        }
    }

    Ok(accounts)
}

/// Add account
pub fn add_account(
    email: String,
    name: Option<String>,
    token: TokenData,
) -> Result<Account, String> {
    let _lock = ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
    let mut index = load_account_index()?;

    // Check if account already exists
    if index.accounts.iter().any(|s| s.email == email) {
        return Err(format!("Account already exists: {}", email));
    }

    // Create new account
    let account_id = Uuid::new_v4().to_string();
    let mut account = Account::new(account_id.clone(), email.clone(), token);
    account.name = name.clone();

    // Save account data
    save_account(&account)?;

    // Update index
    index.accounts.push(AccountSummary {
        id: account.id.clone(),
        email: account.email.clone(),
        name: account.name.clone(),
        disabled: account.disabled,
        protected_models: account.protected_models.clone(),
        created_at: account.created_at,
        last_used: account.last_used,
    });

    // If first account, set as current
    if index.current_account_id.is_none() {
        index.current_account_id = Some(account_id);
    }

    save_account_index(&index)?;

    Ok(account)
}

/// Add or update account
pub fn upsert_account(
    email: String,
    name: Option<String>,
    token: TokenData,
) -> Result<Account, String> {
    let _lock = ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
    let mut index = load_account_index()?;

    // Find account ID if exists
    let existing_account_id = index
        .accounts
        .iter()
        .find(|s| s.email == email)
        .map(|s| s.id.clone());

    if let Some(account_id) = existing_account_id {
        // Update existing account
        match load_account(&account_id) {
            Ok(mut account) => {
                let old_access_token = account.token.access_token.clone();
                let old_refresh_token = account.token.refresh_token.clone();
                account.token = token;
                account.name = name.clone();
                // If an account was previously disabled (e.g. invalid_grant), any explicit token upsert
                // should re-enable it (user manually updated credentials in the UI).
                if account.disabled
                    && (account.token.refresh_token != old_refresh_token
                        || account.token.access_token != old_access_token)
                {
                    account.disabled = false;
                    account.disabled_reason = None;
                    account.disabled_at = None;
                }
                account.update_last_used();
                save_account(&account)?;

                // Sync name in index
                if let Some(idx_summary) = index.accounts.iter_mut().find(|s| s.id == account_id) {
                    idx_summary.name = name;
                    save_account_index(&index)?;
                }

                return Ok(account);
            }
            Err(e) => {
                crate::modules::logger::log_warn(&format!(
                    "Account {} file missing ({}), recreating...",
                    account_id, e
                ));
                // Index exists but file is missing, recreating
                let mut account = Account::new(account_id.clone(), email.clone(), token);
                account.name = name.clone();
                save_account(&account)?;

                // Sync name in index
                if let Some(idx_summary) = index.accounts.iter_mut().find(|s| s.id == account_id) {
                    idx_summary.name = name;
                    save_account_index(&index)?;
                }

                return Ok(account);
            }
        }
    }

    // Add if not exists
    // Note: add_account will attempt to acquire lock, which would deadlock here.
    // Use an internal version or release lock.

    // Release lock, let add_account handle it
    drop(_lock);
    add_account(email, name, token)
}

/// Delete account
pub fn delete_account(account_id: &str) -> Result<(), String> {
    let _lock = ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
    let mut index = load_account_index()?;

    // Remove from index
    let original_len = index.accounts.len();
    index.accounts.retain(|s| s.id != account_id);

    if index.accounts.len() == original_len {
        return Err(format!("Account ID not found: {}", account_id));
    }

    // Clear current account if it's being deleted
    if index.current_account_id.as_deref() == Some(account_id) {
        index.current_account_id = index.accounts.first().map(|s| s.id.clone());
    }

    save_account_index(&index)?;

    // Delete account file
    let accounts_dir = get_accounts_dir()?;
    let account_path = accounts_dir.join(format!("{}.json", account_id));

    if account_path.exists() {
        fs::remove_file(&account_path)
            .map_err(|e| format!("failed_to_delete_account_file: {}", e))?;
    }

    Ok(())
}

/// Batch delete accounts (atomic index operation)
pub fn delete_accounts(account_ids: &[String]) -> Result<(), String> {
    let _lock = ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
    let mut index = load_account_index()?;

    let accounts_dir = get_accounts_dir()?;

    for account_id in account_ids {
        // Remove from index
        index.accounts.retain(|s| &s.id != account_id);

        // Clear current account if it's being deleted
        if index.current_account_id.as_deref() == Some(account_id) {
            index.current_account_id = None;
        }

        // Delete account file
        let account_path = accounts_dir.join(format!("{}.json", account_id));
        if account_path.exists() {
            let _ = fs::remove_file(&account_path);
        }
    }

    // If current account is empty, use first one as default
    if index.current_account_id.is_none() {
        index.current_account_id = index.accounts.first().map(|s| s.id.clone());
    }

    save_account_index(&index)
}

/// Reorder account list
/// Update account order in index file based on provided IDs
pub fn reorder_accounts(account_ids: &[String]) -> Result<(), String> {
    reorder_accounts_at(&get_data_dir()?, account_ids, None)
}

/// Preserve index metadata and account files; reject stale CLI ordering drafts.
pub(crate) fn reorder_accounts_at(root: &std::path::Path, account_ids: &[String], expected: Option<&[String]>) -> Result<(), String> {
    let _process = crate::cli::SwitchLock::acquire(root)?;
    let _lock = lock_account_file_updates()?;
    let path = root.join(ACCOUNTS_INDEX);
    let bytes = fs::read(&path).map_err(|_| "Cannot read account index.")?;
    let mut index: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid account index.")?;
    let accounts = index.get_mut("accounts").and_then(serde_json::Value::as_array_mut).ok_or("Invalid account index.")?;
    let current = accounts.iter().map(|a| a.get("id").and_then(serde_json::Value::as_str).map(str::to_owned).ok_or("Invalid account index.")).collect::<Result<Vec<_>, _>>()?;
    if expected.is_some_and(|ids| ids != current) { return Err("account_order_changed".into()); }
    let mut seen = std::collections::HashSet::new();
    if current.iter().any(|id| !seen.insert(id)) { return Err("Invalid account index.".into()); }
    seen.clear();
    if account_ids.iter().any(|id| !seen.insert(id) || !current.contains(id)) { return Err("Invalid account order.".into()); }
    let ordered = account_ids.iter().chain(current.iter().filter(|id| !account_ids.contains(id)))
        .map(|id| accounts[current.iter().position(|existing| existing == id).unwrap()].clone()).collect();
    *accounts = ordered;
    crate::utils::fs::write_atomic(path, &serde_json::to_vec_pretty(&index).map_err(|_| "Cannot encode account order.")?)
        .map_err(|_| "Cannot save account order.".into())
}

/// Switch current account (Core Logic)
pub async fn switch_account(
    account_id: &str,
    target_ide: Option<&str>,
    integration: &(impl modules::integration::SystemIntegration + ?Sized),
) -> Result<(), String> {
    use crate::modules::oauth;

    // A switch spans external processes and multiple credential locations. Serialize
    // the whole operation so concurrent clicks cannot activate different accounts.
    static SWITCH_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));
    let _switch_guard = SWITCH_LOCK.lock().await;
    let _process_guard = crate::cli::SwitchLock::acquire(&get_data_dir()?)?;

    let index = {
        let _lock = ACCOUNT_INDEX_LOCK
            .lock()
            .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
        load_account_index()?
    };

    // 1. Verify account exists
    if !index.accounts.iter().any(|s| s.id == account_id) {
        return Err(format!("Account not found: {}", account_id));
    }

    let mut account = load_account(account_id)?;
    crate::modules::logger::log_info(&format!(
        "Switching to account: {} (ID: {}) (target_ide: {:?})",
        account.email, account.id, target_ide
    ));

    let original_refresh_token = account.token.refresh_token.clone();

    // 2. Ensure token is valid before switch. Surface clearer hints for known account-state failures.
    let fresh_token = match oauth::ensure_fresh_token(&account.token, Some(&account.id)).await {
        Ok(token) => token,
        Err(e) => {
            if is_account_access_blocked_message(&e) {
                mark_validation_blocked(&mut account, &e);
            }
            return Err(format_switch_refresh_error(&e));
        }
    };

    // Resolve only switch-owned fields on the detached snapshot; never save the
    // old whole account after an await, because quota/disabled state may change.
    account.token = fresh_token;
    ensure_enterprise_project_ready(&mut account).await?;
    account = merge_switch_fields(&account, &original_refresh_token)?;

    // 3. Execute platform-specific system integration (Close proc, Inject DB, Start proc, etc.)
    integration.on_account_switch(&account, target_ide).await?;

    // Merge last-used/index changes into fresh data under the account write lock.
    // A concurrent quota refresh must not be replaced by our old Account clone.
    finish_switch_account(account_id, target_ide)?;

    crate::modules::logger::log_info(&format!(
        "Account switch core logic completed: {}",
        account.email
    ));

    Ok(())
}

fn merge_switch_fields(
    snapshot: &Account,
    expected_refresh_token: &str,
) -> Result<Account, String> {
    let _write = lock_account_file_updates()?;
    let index = load_account_index()?;
    if !index.accounts.iter().any(|a| a.id == snapshot.id) {
        return Err("Account was removed during switch preparation.".into());
    }
    let mut latest = load_account(&snapshot.id)?;
    if latest.token.refresh_token != expected_refresh_token {
        return Err("Account credentials changed during switch preparation.".into());
    }
    if snapshot.token.expiry_timestamp >= latest.token.expiry_timestamp {
        latest.token = snapshot.token.clone();
    } else if latest.token.project_id.is_none() {
        latest.token.project_id = snapshot.token.project_id.clone();
    }
    if latest.device_profile.is_none() {
        latest.device_profile = Some(modules::device::generate_profile());
    }
    save_account(&latest)?;
    Ok(latest)
}

#[cfg(test)]
pub(crate) fn switch_merge_fixture_check() {
    // Called only by the isolated auto-switch subprocess fixture. Exercise the
    // same fresh-field merge after simulating a concurrent quota/flag update.
    assert!(std::env::var_os("AGY_SAFE_SWITCH_FIXTURE").is_some());
    let stale = load_account("B").unwrap();
    let mut latest = stale.clone();
    latest.disabled = true;
    latest.custom_label = Some("newer-label".into());
    if let Some(q) = latest.quota.as_mut() {
        q.models[0].percentage = 1;
    }
    save_account(&latest).unwrap();
    let merged = merge_switch_fields(&stale, &stale.token.refresh_token).unwrap();
    assert!(merged.disabled);
    assert_eq!(merged.custom_label.as_deref(), Some("newer-label"));
    assert_eq!(merged.quota.unwrap().models[0].percentage, 1);
}

fn finish_switch_account(account_id: &str, target_ide: Option<&str>) -> Result<(), String> {
    let _write = lock_account_file_updates()?;
    let mut index = load_account_index()?;
    if !index.accounts.iter().any(|a| a.id == account_id) {
        return Err("Credentials were updated, but the target account was removed.".into());
    }
    let mut latest = load_account(account_id)?;
    latest.update_last_used();
    save_account(&latest)?;
    index.current_account_id = Some(account_id.into());
    index.current_target_ide = target_ide.map(str::to_owned);
    save_account_index(&index)
}

fn is_enterprise_client(client_key: Option<&str>) -> bool {
    client_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| key.eq_ignore_ascii_case("antigravity_enterprise"))
        .unwrap_or(false)
}

fn normalize_project_id(project_id: Option<&str>) -> Option<String> {
    project_id
        .map(str::trim)
        .filter(|pid| !pid.is_empty())
        .map(ToOwned::to_owned)
}

async fn ensure_enterprise_project_ready(account: &mut Account) -> Result<(), String> {
    if !is_enterprise_client(account.token.oauth_client_key.as_deref()) {
        return Ok(());
    }

    if normalize_project_id(account.token.project_id.as_deref()).is_some() {
        return Ok(());
    }

    crate::modules::logger::log_warn(&format!(
        "Account {} is using enterprise OAuth client but missing project_id. Trying to resolve before switch...",
        account.email
    ));

    match crate::modules::project_resolver::fetch_project_id(&account.token.access_token).await {
        Ok(project_id) => {
            crate::modules::logger::log_info(&format!(
                "Resolved enterprise project_id for {}: {}",
                account.email, project_id
            ));
            account.token.project_id = Some(project_id);
            Ok(())
        }
        Err(e) => {
            crate::modules::logger::log_warn(&format!(
                "Account {} is currently missing enterprise project_id and auto-resolve failed ({}). Allowing switch to proceed, but certain enterprise features may be limited.",
                account.email, e
            ));
            Ok(())
        }
    }
}

fn is_rate_limit_error(err: &crate::error::AppError) -> bool {
    match err {
        crate::error::AppError::Network(_, Some(status)) => *status == 429,
        crate::error::AppError::Unknown(msg)
        | crate::error::AppError::OAuth(msg)
        | crate::error::AppError::Account(msg)
        | crate::error::AppError::Config(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("429")
                || lower.contains("too many requests")
                || lower.contains("resource_exhausted")
                || lower.contains("resource has been exhausted")
        }
        _ => false,
    }
}

fn recover_cached_quota_on_rate_limit(
    account: &Account,
    err: &crate::error::AppError,
) -> Option<QuotaData> {
    if !is_rate_limit_error(err) {
        return None;
    }

    let cached = account.quota.clone()?;
    if cached.models.is_empty() {
        return None;
    }

    Some(cached)
}

fn is_validation_required_error(err: &crate::error::AppError) -> bool {
    let text = err.to_string().to_lowercase();
    text.contains("verify your account")
        || text.contains("further action is required")
        || text.contains("validation_url")
        || text.contains("appeal_url")
        || text.contains("validation required")
}

fn is_account_access_blocked_message(message: &str) -> bool {
    let text = message.to_lowercase();
    text.contains("verify your account")
        || text.contains("further action is required")
        || text.contains("validation_url")
        || text.contains("appeal_url")
        || text.contains("validation required")
        || text.contains("unauthorized_client")
        || text.contains("invalid_client")
        || text.contains("invalid_grant")
        || text.contains("resource_exhausted")
        || text.contains("resource has been exhausted")
}

fn format_switch_refresh_error(message: &str) -> String {
    let lower = message.to_lowercase();

    if lower.contains("unauthorized_client")
        || lower.contains("invalid_client")
        || lower.contains("invalid_grant")
    {
        return format!(
            "Token refresh failed: OAuth client is not authorized for this account. Please sign in again in agy-switch and complete authorization/verification. Raw error: {}",
            message
        );
    }

    if lower.contains("verify your account")
        || lower.contains("further action is required")
        || lower.contains("validation_url")
        || lower.contains("appeal_url")
        || lower.contains("validation required")
    {
        return format!(
            "Token refresh failed: account requires additional verification. Please finish verification in Antigravity, then retry account switch. Raw error: {}",
            message
        );
    }

    if lower.contains("resource_exhausted") || lower.contains("resource has been exhausted") {
        return format!(
            "Token refresh failed: account is rate-limited or temporarily restricted (RESOURCE_EXHAUSTED). Please retry later. Raw error: {}",
            message
        );
    }

    format!("Token refresh failed: {}", message)
}

fn format_rate_limit_block_reason(err: &crate::error::AppError) -> String {
    format!(
        "Account is temporarily rate-limited or risk-controlled (RESOURCE_EXHAUSTED). Please cool down and retry later. Raw error: {}",
        err
    )
}

fn mark_validation_blocked(account: &mut Account, reason: &str) {
    if account.validation_blocked && account.validation_blocked_reason.as_deref() == Some(reason) {
        return;
    }

    account.validation_blocked = true;
    account.validation_blocked_reason = Some(reason.to_string());
    if let Err(e) = save_account(account) {
        crate::modules::logger::log_warn(&format!(
            "Failed to persist validation_blocked state for {}: {}",
            account.email, e
        ));
    }
}

fn clear_validation_blocked(account: &mut Account) {
    if !account.validation_blocked {
        return;
    }

    account.validation_blocked = false;
    account.validation_blocked_until = None;
    account.validation_blocked_reason = None;
    account.validation_url = None;
    if let Err(e) = save_account(account) {
        crate::modules::logger::log_warn(&format!(
            "Failed to clear validation_blocked state for {}: {}",
            account.email, e
        ));
    }
}

/// Get current account ID
pub fn get_current_account_id() -> Result<Option<String>, String> {
    let index = load_account_index()?;
    Ok(index.current_account_id)
}

/// Get currently active account details
pub fn get_current_account() -> Result<Option<Account>, String> {
    if let Some(id) = get_current_account_id()? {
        Ok(Some(load_account(&id)?))
    } else {
        Ok(None)
    }
}

/// Set current active account ID
pub fn set_current_account_id(account_id: &str) -> Result<(), String> {
    set_current_account_id_with_target(account_id, None)
}

/// Set current active account ID and target IDE
pub fn set_current_account_id_with_target(
    account_id: &str,
    target_ide: Option<&str>,
) -> Result<(), String> {
    let _lock = ACCOUNT_INDEX_LOCK
        .lock()
        .map_err(|e| format!("failed_to_acquire_lock: {}", e))?;
    let mut index = load_account_index()?;
    index.current_account_id = Some(account_id.to_string());
    index.current_target_ide = target_ide.map(|s| s.to_string());
    save_account_index(&index)
}

/// Update account quota
fn normalize_protected_model_id(model_name: &str) -> Option<String> {
    let lower = model_name.to_lowercase();
    if lower.contains("image") {
        return Some(
            if lower.contains("flash") {
                "gemini-3.1-flash-image"
            } else {
                "gemini-3-pro-image"
            }
            .to_string(),
        );
    }
    if lower.contains("flash") {
        return Some("gemini-3-flash".to_string());
    }
    if lower.contains("pro") {
        return Some("gemini-3-pro-high".to_string());
    }
    if ["claude", "opus", "sonnet", "haiku"]
        .iter()
        .any(|family| lower.contains(family))
    {
        return Some("claude".to_string());
    }
    None
}

pub fn update_account_quota(account_id: &str, quota: QuotaData) -> Result<(), String> {
    let _account_write = lock_account_file_updates()?;
    let mut account = load_account(account_id)?;
    account.update_quota(quota);

    // --- Quota protection logic start ---
    if let Ok(config) = crate::modules::config::load_app_config() {
        if config.quota_protection.enabled {
            if let Some(ref q) = account.quota {
                let threshold = config.quota_protection.threshold_percentage as i32;

                let mut group_max_percentage: HashMap<String, i32> = HashMap::new();

                for model in &q.models {
                    if let Some(std_id) = normalize_protected_model_id(&model.name) {
                        let entry = group_max_percentage.entry(std_id).or_insert(-1);
                        if model.percentage > *entry {
                            *entry = model.percentage;
                        }
                    }
                }

                for std_id in &config.quota_protection.monitored_models {
                    let lookup_key =
                        normalize_protected_model_id(std_id).unwrap_or_else(|| std_id.clone());
                    let max_pct = group_max_percentage
                        .get(&lookup_key)
                        .cloned()
                        .unwrap_or(100);

                    if max_pct < threshold {
                        if !account.protected_models.contains(&lookup_key) {
                            crate::modules::logger::log_info(&format!(
                                "[Quota] Triggering model protection: {} (Group: {} Max: {}% < Thres: {}%)",
                                account.email, lookup_key, max_pct, threshold
                            ));
                            account.protected_models.insert(lookup_key.clone());
                        }
                    } else {
                        if account.protected_models.contains(&lookup_key) {
                            crate::modules::logger::log_info(&format!(
                                "[Quota] Model protection recovered: {} (Group: {} Max: {}% >= Thres: {}%)",
                                account.email, lookup_key, max_pct, threshold
                            ));
                            account.protected_models.remove(&lookup_key);
                        }
                    }
                }
            }
        } else {
            // [FIX] 当配额保护在全局关闭时，清空受保护模型列表，避免遗留历史锁
            if !account.protected_models.is_empty() {
                crate::modules::logger::log_info(&format!(
                    "[Quota] Quota protection disabled globally, clearing protected models for {}",
                    account.email
                ));
                account.protected_models.clear();
            }
        }
    }
    // --- Quota protection logic end ---

    // Save account first
    save_account(&account)?;

    // [FIX] 同时更新索引文件中的摘要信息，确保列表页图标即时刷新
    {
        if let Ok(mut index) = load_account_index() {
            if let Some(summary) = index.accounts.iter_mut().find(|a| a.id == account_id) {
                summary.protected_models = account.protected_models.clone();
                let _ = save_account_index(&index);
            }
        }
    }

    Ok(())
}

/// Export accounts by IDs (for backup/migration)
pub fn export_accounts_by_ids(
    account_ids: &[String],
) -> Result<crate::models::AccountExportResponse, String> {
    use crate::models::{AccountExportItem, AccountExportResponse};

    let accounts = list_accounts()?;

    let export_items: Vec<AccountExportItem> = accounts
        .into_iter()
        .filter(|acc| account_ids.contains(&acc.id))
        .map(|acc| AccountExportItem {
            email: acc.email,
            refresh_token: acc.token.refresh_token,
        })
        .collect();

    Ok(AccountExportResponse {
        accounts: export_items,
    })
}

/// Reuse a cached project ID only after a subscription tier has been recorded.
/// New accounts may already have the generic `aicode-consumers` project ID, but
/// still need one loadCodeAssist request to discover whether they are PRO/ULTRA.
fn cached_project_id_for_quota(account: &Account) -> Option<&str> {
    let has_subscription_tier = account
        .quota
        .as_ref()
        .and_then(|quota| quota.subscription_tier.as_deref())
        .is_some_and(|tier| !tier.trim().is_empty());

    has_subscription_tier
        .then(|| account.token.project_id.as_deref())
        .flatten()
}

/// Quota query with retry (moved from commands to modules for reuse)
pub async fn fetch_quota_with_retry(account: &mut Account) -> crate::error::AppResult<QuotaData> {
    use crate::error::AppError;
    use crate::modules::oauth;

    // 1. Time-based check - ensure Token is valid first
    let token = match oauth::ensure_fresh_token(&account.token, Some(&account.id)).await {
        Ok(t) => t,
        Err(e) => {
            if e.contains("invalid_grant") {
                modules::logger::log_error(&format!(
                    "Disabling account {} due to invalid_grant during token refresh (quota check)",
                    account.email
                ));
                account.disabled = true;
                account.disabled_at = Some(chrono::Utc::now().timestamp());
                account.disabled_reason = Some(format!("invalid_grant: {}", e));
                let _ = save_account(account);
            }
            return Err(AppError::OAuth(e));
        }
    };

    if token.access_token != account.token.access_token {
        modules::logger::log_info(&format!("Time-based Token refresh: {}", account.email));
        account.token = token.clone();

        // Get display name (incidental to Token refresh)
        let name = if account.name.is_none()
            || account.name.as_ref().map_or(false, |n| n.trim().is_empty())
        {
            match oauth::get_user_info(&token.access_token).await {
                Ok(user_info) => user_info.get_display_name(),
                Err(_) => None,
            }
        } else {
            account.name.clone()
        };

        account.name = name.clone();
        upsert_account(account.email.clone(), name, token.clone()).map_err(AppError::Account)?;
    }

    // 0. Supplement display name (if missing or upper step failed)
    if account.name.is_none() || account.name.as_ref().map_or(false, |n| n.trim().is_empty()) {
        modules::logger::log_info(&format!(
            "Account {} missing display name, attempting to fetch...",
            account.email
        ));
        // Use updated token
        match oauth::get_user_info(&account.token.access_token).await {
            Ok(user_info) => {
                let display_name = user_info.get_display_name();
                modules::logger::log_info(&format!(
                    "Successfully fetched display name: {:?}",
                    display_name
                ));
                account.name = display_name.clone();
                // Save immediately
                if let Err(e) =
                    upsert_account(account.email.clone(), display_name, account.token.clone())
                {
                    modules::logger::log_warn(&format!("Failed to save display name: {}", e));
                }
            }
            Err(e) => {
                modules::logger::log_warn(&format!("Failed to fetch display name: {}", e));
            }
        }
    }

    // 2. Query quota. A missing tier must trigger loadCodeAssist at least once;
    // otherwise a newly added account with a cached project_id never gets its
    // subscription tier persisted and the UI cannot show PRO/ULTRA.
    let cached_project_id = cached_project_id_for_quota(account);
    let result: crate::error::AppResult<(QuotaData, Option<String>)> =
        modules::fetch_quota_with_cache(
            &account.token.access_token,
            &account.email,
            cached_project_id,
        )
        .await;

    // Capture potentially updated project_id and save
    if let Ok((ref _q, ref project_id)) = result {
        if project_id.is_some() && *project_id != account.token.project_id {
            modules::logger::log_info(&format!(
                "Detected project_id update ({}), saving...",
                account.email
            ));
            account.token.project_id = project_id.clone();
            if let Err(e) = upsert_account(
                account.email.clone(),
                account.name.clone(),
                account.token.clone(),
            ) {
                modules::logger::log_warn(&format!("Failed to sync project_id: {}", e));
            }
        }
    }

    // 3. Handle 401 error
    if let Err(AppError::Network(_, status)) = result {
        if let Some(code) = status {
            if code == 401 {
                modules::logger::log_warn(&format!(
                    "401 Unauthorized for {}, forcing refresh...",
                    account.email
                ));

                // Force refresh
                let token_res = match oauth::refresh_access_token_with_client(
                    &account.token.refresh_token,
                    Some(&account.id),
                    account.token.oauth_client_key.as_deref(),
                )
                .await
                {
                    Ok(t) => t,
                    Err(e) => {
                        if e.contains("invalid_grant") {
                            modules::logger::log_error(&format!(
                                "Disabling account {} due to invalid_grant during forced refresh (quota check)",
                                account.email
                            ));
                            account.disabled = true;
                            account.disabled_at = Some(chrono::Utc::now().timestamp());
                            account.disabled_reason = Some(format!("invalid_grant: {}", e));
                            let _ = save_account(account);
                        }
                        return Err(AppError::OAuth(e));
                    }
                };

                let new_token = TokenData::new(
                    token_res.access_token.clone(),
                    account.token.refresh_token.clone(),
                    token_res.expires_in,
                    account.token.email.clone(),
                    account.token.project_id.clone(), // Keep original project_id
                    None,                             // Add None as session_id
                    account.token.is_gcp_tos,
                    token_res.id_token.clone(),
                )
                .with_oauth_client_key(
                    token_res
                        .oauth_client_key
                        .clone()
                        .or_else(|| account.token.oauth_client_key.clone()),
                );

                // Re-fetch display name
                let name = if account.name.is_none()
                    || account.name.as_ref().map_or(false, |n| n.trim().is_empty())
                {
                    match oauth::get_user_info(&token_res.access_token).await {
                        Ok(user_info) => user_info.get_display_name(),
                        Err(_) => None,
                    }
                } else {
                    account.name.clone()
                };

                account.token = new_token.clone();
                account.name = name.clone();
                upsert_account(account.email.clone(), name, new_token.clone())
                    .map_err(AppError::Account)?;

                // Retry query, preserving the same tier-discovery behavior.
                let retry_cached_project_id = cached_project_id_for_quota(account);
                let retry_result: crate::error::AppResult<(QuotaData, Option<String>)> =
                    modules::fetch_quota_with_cache(
                        &new_token.access_token,
                        &account.email,
                        retry_cached_project_id,
                    )
                    .await;

                // Also handle project_id saving during retry
                if let Ok((ref _q, ref project_id)) = retry_result {
                    if project_id.is_some() && *project_id != account.token.project_id {
                        modules::logger::log_info(&format!(
                            "Detected update of project_id after retry ({}), saving...",
                            account.email
                        ));
                        account.token.project_id = project_id.clone();
                        let _ = upsert_account(
                            account.email.clone(),
                            account.name.clone(),
                            account.token.clone(),
                        );
                    }
                }

                if let Err(AppError::Network(_, status)) = retry_result {
                    if let Some(code) = status {
                        if code == 403 {
                            let mut q = QuotaData::new();
                            q.is_forbidden = true;
                            return Ok(q);
                        }
                    }
                }

                match retry_result {
                    Ok((q, _)) => {
                        clear_validation_blocked(account);
                        return Ok(q);
                    }
                    Err(e) => {
                        if is_validation_required_error(&e) {
                            mark_validation_blocked(account, &e.to_string());
                        }
                        if let Some(cached) = recover_cached_quota_on_rate_limit(account, &e) {
                            mark_validation_blocked(account, &format_rate_limit_block_reason(&e));
                            modules::logger::log_warn(&format!(
                                "Quota API rate-limited for {}, using cached model list as fallback",
                                account.email
                            ));
                            return Ok(cached);
                        }
                        return Err(e);
                    }
                }
            }
        }
    }

    // fetch_quota already handles 403, with additional local fallback/validation handling.
    match result {
        Ok((q, _)) => {
            clear_validation_blocked(account);
            Ok(q)
        }
        Err(e) => {
            if is_validation_required_error(&e) {
                mark_validation_blocked(account, &e.to_string());
            }
            if let Some(cached) = recover_cached_quota_on_rate_limit(account, &e) {
                mark_validation_blocked(account, &format_rate_limit_block_reason(&e));
                modules::logger::log_warn(&format!(
                    "Quota API rate-limited for {}, using cached model list as fallback",
                    account.email
                ));
                return Ok(cached);
            }
            Err(e)
        }
    }
}

#[derive(Serialize)]
pub struct RefreshStats {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub details: Vec<String>,
}

/// Core logic to batch refresh all account quotas (decoupled from Tauri status)
pub async fn refresh_all_quotas_logic() -> Result<RefreshStats, String> {
    use futures::future::join_all;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    const MAX_CONCURRENT: usize = 5;
    let start = std::time::Instant::now();

    crate::modules::logger::log_info(&format!(
        "Starting batch refresh of all account quotas (Concurrent mode, max: {})",
        MAX_CONCURRENT
    ));
    let accounts = list_accounts()?;

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));

    let tasks: Vec<_> = accounts
        .into_iter()
        .filter(|account| {
            // Skip only forbidden accounts; transient errors can be retried by the user.
            if let Some(ref q) = account.quota {
                if q.is_forbidden {
                    crate::modules::logger::log_info(&format!(
                        "  - Skipping {} (Forbidden)",
                        account.email
                    ));
                    return false;
                }
            }
            true
        })
        .map(|mut account| {
            let email = account.email.clone();
            let account_id = account.id.clone();
            let permit = semaphore.clone();
            async move {
                let _guard = permit.acquire().await.unwrap();
                crate::modules::logger::log_info(&format!("  - Processing {}", email));
                match fetch_quota_with_retry(&mut account).await {
                    Ok(quota) => {
                        if let Err(e) = update_account_quota(&account_id, quota) {
                            let msg = format!("Account {}: Save quota failed - {}", email, e);
                            crate::modules::logger::log_error(&msg);
                            Err(msg)
                        } else {
                            crate::modules::logger::log_info(&format!("    Success {}", email));
                            Ok(())
                        }
                    }
                    Err(e) => {
                        let msg = format!("Account {}: Fetch quota failed - {}", email, e);
                        crate::modules::logger::log_error(&msg);
                        Err(msg)
                    }
                }
            }
        })
        .collect();

    let total = tasks.len();
    let results = join_all(tasks).await;

    let mut success = 0;
    let mut failed = 0;
    let mut details = Vec::new();

    for result in results {
        match result {
            Ok(()) => success += 1,
            Err(msg) => {
                failed += 1;
                details.push(msg);
            }
        }
    }

    let elapsed = start.elapsed();
    crate::modules::logger::log_info(&format!(
        "Batch refresh completed: {} success, {} failed, took: {}ms",
        success,
        failed,
        elapsed.as_millis()
    ));

    Ok(RefreshStats {
        total,
        success,
        failed,
        details,
    })
}

#[cfg(test)]
mod ordering_tests {
    use super::*;
    #[test]
    fn ordering_preserves_selection_metadata_and_rejects_stale_or_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(ACCOUNTS_INDEX);
        let index = serde_json::json!({"version":"fixture", "current_account_id":"A", "current_target_ide":"ide", "extra":"keep", "accounts":[{"id":"A","last_used":123,"extra":true},{"id":"B","last_used":456}]});
        fs::write(&path, serde_json::to_vec(&index).unwrap()).unwrap();
        let current = vec!["A".into(), "B".into()]; let ordered = vec!["B".into(), "A".into()];
        reorder_accounts_at(dir.path(), &ordered, Some(&current)).unwrap();
        let result: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(result["current_account_id"], "A"); assert_eq!(result["extra"], "keep");
        assert_eq!(result["accounts"][1], index["accounts"][0]);
        let before = fs::read(&path).unwrap();
        assert!(reorder_accounts_at(dir.path(), &current, Some(&current)).is_err());
        assert!(reorder_accounts_at(dir.path(), &["A".into(), "A".into()], None).is_err());
        assert!(reorder_accounts_at(dir.path(), &["missing".into()], None).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
