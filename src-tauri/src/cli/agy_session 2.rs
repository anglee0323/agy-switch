//! Which saved account the native agy session file belongs to.
//!
//! A read-only, credential-free answer for scripts: the session is matched to a
//! saved account by its refresh token, which is compared and never printed.
use super::{output::Snapshot, CliError, Result};
use std::{fs, path::Path};

pub(super) fn account_id(root: &Path, snapshot: &Snapshot) -> Result<String> {
    let session = crate::modules::integration::read_cli_credentials()
        .map_err(|_| CliError::data("Cannot read the agy session. Start agy once to initialize it."))?;
    for account in &snapshot.accounts {
        let Ok(text) = fs::read_to_string(root.join("accounts").join(format!("{}.json", account.id))) else { continue };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let saved = value.get("token").and_then(|token| token.get("refresh_token")).and_then(serde_json::Value::as_str);
        if saved.is_some_and(|token| !token.is_empty() && token == session.refresh_token) {
            return Ok(account.id.clone());
        }
    }
    Err(CliError::missing())
}
