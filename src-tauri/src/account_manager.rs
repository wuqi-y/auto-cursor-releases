use crate::machine_id::MachineIdRestorer;
use crate::{log_debug, log_error, log_info, log_warn};
use anyhow::{Result, anyhow};
#[cfg(not(target_os = "windows"))]
use dirs;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTag {
    pub text: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub email: String,
    pub token: String,
    pub refresh_token: Option<String>,
    pub workos_cursor_session_token: Option<String>,
    pub is_current: bool,
    pub created_at: String,
    #[serde(rename = "isAutoSwitch", default)]
    pub is_auto_switch: Option<bool>,
    #[serde(default)]
    pub custom_tags: Option<Vec<CustomTag>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountListResult {
    pub success: bool,
    pub accounts: Vec<AccountInfo>,
    pub current_account: Option<AccountInfo>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchAccountResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogoutResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<String>,
}

pub struct AccountManager;

impl AccountManager {
    pub fn new() -> Self {
        Self
    }

    /// Get the account.json file path (same directory as backup files)
    fn get_account_file_path() -> Result<PathBuf> {
        let (db_path, _) = Self::get_cursor_paths()?;
        let db_dir = db_path
            .parent()
            .ok_or_else(|| anyhow!("Could not get parent directory"))?;
        Ok(db_dir.join("account.json"))
    }

    /// Get Cursor paths for different platforms
    #[cfg(target_os = "windows")]
    fn get_cursor_paths() -> Result<(PathBuf, PathBuf)> {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| anyhow!("APPDATA environment variable not set"))?;

        let db_path = PathBuf::from(&appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("storage.json");

        let sqlite_path = PathBuf::from(&appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");

        Ok((db_path, sqlite_path))
    }

    #[cfg(target_os = "macos")]
    fn get_cursor_paths() -> Result<(PathBuf, PathBuf)> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

        let db_path = home
            .join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("storage.json");

        let sqlite_path = home
            .join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");

        Ok((db_path, sqlite_path))
    }

    #[cfg(target_os = "linux")]
    fn get_cursor_paths() -> Result<(PathBuf, PathBuf)> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

        let db_path = home
            .join(".config")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("storage.json");

        let sqlite_path = home
            .join(".config")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb");

        Ok((db_path, sqlite_path))
    }

    /// Load accounts from account.json file
    pub fn load_accounts() -> Result<Vec<AccountInfo>> {
        let account_file = Self::get_account_file_path()?;

        if !account_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&account_file)
            .map_err(|e| anyhow!("Failed to read account file: {}", e))?;

        let accounts: Vec<AccountInfo> = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse account file: {}", e))?;

        Ok(accounts)
    }

    /// Save accounts to account.json file
    pub fn save_accounts(accounts: &[AccountInfo]) -> Result<()> {
        let account_file = Self::get_account_file_path()?;

        // Ensure directory exists
        if let Some(parent) = account_file.parent() {
            fs::create_dir_all(parent).map_err(|e| anyhow!("Failed to create directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(accounts)
            .map_err(|e| anyhow!("Failed to serialize accounts: {}", e))?;

        fs::write(&account_file, content)
            .map_err(|e| anyhow!("Failed to write account file: {}", e))?;

        Ok(())
    }

    /// Get current account from Cursor storage
    pub fn get_current_account() -> Result<Option<AccountInfo>> {
        // Try to get current token from Cursor
        let current_token = Self::get_current_token();

        if let Some(token) = current_token {
            // Load account list and find by token
            match Self::load_accounts() {
                Ok(accounts) => {
                    // Find account by token
                    if let Some(mut account) = accounts.into_iter().find(|acc| acc.token == token) {
                        // Mark as current
                        account.is_current = true;
                        return Ok(Some(account));
                    }

                    // If not found in list, try to get email and create basic account info
                    if let Some(email) = Self::get_current_email() {
                        Ok(Some(AccountInfo {
                            email,
                            token,
                            refresh_token: None,
                            workos_cursor_session_token: None,
                            is_current: true,
                            created_at: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            is_auto_switch: None,
                            custom_tags: None,
                        }))
                    } else {
                        Ok(None)
                    }
                }
                Err(_) => {
                    // If can't load accounts, fallback to basic info
                    if let Some(email) = Self::get_current_email() {
                        Ok(Some(AccountInfo {
                            email,
                            token,
                            refresh_token: None,
                            workos_cursor_session_token: None,
                            is_current: true,
                            created_at: chrono::Local::now()
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string(),
                            is_auto_switch: None,
                            custom_tags: None,
                        }))
                    } else {
                        Ok(None)
                    }
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Get current email from Cursor storage
    fn get_current_email() -> Option<String> {
        // Try SQLite database
        if let Some(email) = Self::get_email_from_sqlite() {
            return Some(email);
        }

        // Try storage.json first
        if let Some(email) = Self::get_email_from_storage() {
            return Some(email);
        }
        None
    }

    /// Get current token from Cursor storage
    fn get_current_token() -> Option<String> {
        // Use the existing token detection logic from auth_checker
        let token_info = crate::auth_checker::AuthChecker::get_token_auto();
        token_info.token
    }

    /// Get email from storage.json
    fn get_email_from_storage() -> Option<String> {
        use fs2::FileExt;

        let (storage_path, _) = Self::get_cursor_paths().ok()?;

        if !storage_path.exists() {
            return None;
        }

        // 使用共享锁进行读取操作
        let file = fs::File::open(&storage_path).ok()?;
        file.lock_shared().ok()?;

        let content = fs::read_to_string(&storage_path).ok()?;
        let storage_data: serde_json::Value = serde_json::from_str(&content).ok()?;

        // Try cursorAuth/cachedEmail first
        if let Some(email) = storage_data
            .get("cursorAuth/cachedEmail")
            .and_then(|v| v.as_str())
        {
            if email.contains('@') {
                let _ = file.unlock();
                return Some(email.to_string());
            }
        }

        // Try other email fields
        if let Some(obj) = storage_data.as_object() {
            for (key, value) in obj {
                if key.to_lowercase().contains("email") {
                    if let Some(email_str) = value.as_str() {
                        if email_str.contains('@') {
                            let _ = file.unlock();
                            return Some(email_str.to_string());
                        }
                    }
                }
            }
        }

        let _ = file.unlock();
        None
    }

    /// Get email from SQLite database
    fn get_email_from_sqlite() -> Option<String> {
        let (_, sqlite_path) = Self::get_cursor_paths().ok()?;

        if !sqlite_path.exists() {
            return None;
        }

        let conn = Connection::open(&sqlite_path).ok()?;
        let query =
            "SELECT value FROM ItemTable WHERE key LIKE '%email%' OR key LIKE '%cursorAuth%'";

        let mut stmt = conn.prepare(query).ok()?;
        let rows = stmt
            .query_map([], |row| {
                let value: String = row.get(0)?;
                Ok(value)
            })
            .ok()?;

        for row_result in rows {
            if let Ok(value) = row_result {
                // If it's a string and contains @, it might be an email
                if value.contains('@') && value.len() > 5 && value.len() < 100 {
                    return Some(value);
                }

                // Try to parse as JSON
                if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&value) {
                    if let Some(obj) = json_data.as_object() {
                        // Check for email field
                        if let Some(email) = obj.get("email") {
                            if let Some(email_str) = email.as_str() {
                                return Some(email_str.to_string());
                            }
                        }

                        // Check for cachedEmail field
                        if let Some(cached_email) = obj.get("cachedEmail") {
                            if let Some(email_str) = cached_email.as_str() {
                                return Some(email_str.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Add a new account
    pub fn add_account(
        email: String,
        token: String,
        refresh_token: Option<String>,
        workos_cursor_session_token: Option<String>,
    ) -> Result<()> {
        // Check if email is empty or whitespace only
        if email.trim().is_empty() {
            log_warn!("⚠️ [DEBUG] Attempted to add account with empty email, ignoring");
            return Err(anyhow!("Cannot add account with empty email"));
        }

        let mut accounts = Self::load_accounts()?;

        // Check if account already exists
        if accounts.iter().any(|acc| acc.email == email) {
            return Err(anyhow!("Account with this email already exists"));
        }

        let new_account = AccountInfo {
            email,
            token,
            refresh_token,
            workos_cursor_session_token,
            is_current: false,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_auto_switch: None,
            custom_tags: None,
        };

        accounts.push(new_account);
        Self::save_accounts(&accounts)?;

        Ok(())
    }

    /// Save account list to file
    pub fn save_account_list(accounts: &[AccountInfo]) -> Result<(), String> {
        Self::save_accounts(accounts).map_err(|e| e.to_string())
    }

    /// Get all accounts with current account info
    pub fn get_account_list() -> AccountListResult {
        match Self::load_accounts() {
            Ok(mut accounts) => {
                let current_account = Self::get_current_account().unwrap_or(None);

                // Ensure current account is in the list
                if let Some(ref current) = current_account {
                    // Use token to match current account, not email
                    let current_exists = accounts.iter().any(|acc| acc.token == current.token);

                    if !current_exists {
                        // Add current account to the list
                        accounts.push(current.clone());
                        // Save the updated list
                        let _ = Self::save_accounts(&accounts);
                    }

                    // Mark current account in the list using token
                    for account in &mut accounts {
                        account.is_current = account.token == current.token;
                    }
                }

                AccountListResult {
                    success: true,
                    accounts,
                    current_account,
                    message: "Account list loaded successfully".to_string(),
                }
            }
            Err(e) => AccountListResult {
                success: false,
                accounts: Vec::new(),
                current_account: None,
                message: format!("Failed to load accounts: {}", e),
            },
        }
    }

    /// Switch to a different account using email and token directly
    pub fn switch_account_with_token(
        email: String,
        token: String,
        auth_type: Option<String>,
    ) -> SwitchAccountResult {
        let mut details = Vec::new();
        let auth_type = auth_type.unwrap_or_else(|| "Auth_0".to_string());

        details.push(format!(
            "Switching to account: {} (auth type: {})",
            email, auth_type
        ));

        // 1. Inject email to SQLite database
        match Self::inject_email_to_sqlite(&email) {
            Ok(()) => {
                details.push("Successfully injected email to SQLite database".to_string());
            }
            Err(e) => {
                details.push(format!("Warning: Failed to inject email to SQLite: {}", e));
            }
        }

        // 2. Inject token to SQLite database with auth type
        match Self::inject_token_to_sqlite_with_auth_type(&token, &auth_type) {
            Ok(()) => {
                details.push(
                    "Successfully injected token and auth type to SQLite database".to_string(),
                );
            }
            Err(e) => {
                return SwitchAccountResult {
                    success: false,
                    message: format!("Failed to inject token: {}", e),
                    details,
                };
            }
        }

        // 3. Update storage.json if possible
        match Self::update_storage_json(&email, &token) {
            Ok(()) => {
                details.push("Successfully updated storage.json".to_string());
            }
            Err(e) => {
                details.push(format!("Warning: Failed to update storage.json: {}", e));
            }
        }

        // Wait for database updates to complete (CRITICAL!)
        log_debug!("🔍 [DEBUG] Waiting for database updates to complete...");
        std::thread::sleep(std::time::Duration::from_millis(500));
        log_info!("✅ [DEBUG] Database update wait completed");
        details.push("Waited for database updates to complete".to_string());

        SwitchAccountResult {
            success: true,
            message: format!("Successfully switched to account: {}", email),
            details,
        }
    }

    /// Switch to a different account (legacy method - looks up from saved accounts)
    pub fn switch_account(email: String, auto_restart: bool) -> SwitchAccountResult {
        let mut details = Vec::new();

        // Load accounts to find the target account
        let accounts = match Self::load_accounts() {
            Ok(accounts) => accounts,
            Err(e) => {
                return SwitchAccountResult {
                    success: false,
                    message: format!("Failed to load accounts: {}", e),
                    details: vec![e.to_string()],
                };
            }
        };

        let target_account = match accounts.iter().find(|acc| acc.email == email) {
            Some(account) => account,
            None => {
                return SwitchAccountResult {
                    success: false,
                    message: "Account not found".to_string(),
                    details: vec![format!("No account found with email: {}", email)],
                };
            }
        };

        details.push(format!("Switching to account: {}", email));

        // 0. Force kill and restart Cursor processes (CRITICAL!) - only if auto_restart is true
        if auto_restart {
            log_debug!("🔍 [DEBUG] Auto-restart enabled, force killing Cursor processes...");

            // First, try to gracefully close Cursor
            if Self::is_cursor_running() {
                log_debug!("🔍 [DEBUG] Cursor is running, attempting graceful close first...");
                match Self::force_close_cursor() {
                    Ok(()) => {
                        log_info!("✅ [DEBUG] Gracefully closed Cursor");
                        details.push("Gracefully closed Cursor processes".to_string());
                    }
                    Err(e) => {
                        log_error!("❌ [DEBUG] Graceful close failed: {}", e);
                        details.push(format!("Graceful close failed: {}", e));
                    }
                }
            }

            // Wait a moment for graceful shutdown
            std::thread::sleep(std::time::Duration::from_millis(1000));

            // Force kill any remaining Cursor processes
            log_debug!("🔍 [DEBUG] Force killing any remaining Cursor processes...");
            match Self::force_kill_cursor_processes() {
                Ok(killed_count) => {
                    if killed_count > 0 {
                        log_info!("✅ [DEBUG] Force killed {} Cursor processes", killed_count);
                        details.push(format!(
                            "Force killed {} remaining Cursor processes",
                            killed_count
                        ));
                    } else {
                        log_info!("✅ [DEBUG] No remaining Cursor processes to kill");
                        details.push("No remaining Cursor processes found".to_string());
                    }
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] Failed to force kill Cursor processes: {}", e);
                    details.push(format!("Warning: Failed to force kill processes: {}", e));
                }
            }

            // Wait for processes to be fully terminated
            std::thread::sleep(std::time::Duration::from_millis(500));
        } else {
            log_info!("⏭️ [DEBUG] Auto-restart disabled, skipping Cursor restart");
            details.push("Skipped Cursor restart (auto-restart disabled)".to_string());
        }

        // 1. Inject email to SQLite database
        log_info!(
            "🔍 [DEBUG] Starting email injection for: {}",
            target_account.email
        );
        match Self::inject_email_to_sqlite(&target_account.email) {
            Ok(()) => {
                log_info!("✅ [DEBUG] Email injection successful");
                details.push("Successfully injected email to SQLite database".to_string());
            }
            Err(e) => {
                log_error!("❌ [DEBUG] Email injection failed: {}", e);
                details.push(format!("Warning: Failed to inject email to SQLite: {}", e));
            }
        }

        // 2. Inject token to SQLite database
        log_info!(
            "🔍 [DEBUG] Starting token injection, token length: {}",
            target_account.token.len()
        );
        match Self::inject_token_to_sqlite(&target_account.token) {
            Ok(()) => {
                log_info!("✅ [DEBUG] Token injection successful");
                details.push("Successfully injected token to SQLite database".to_string());
            }
            Err(e) => {
                log_error!("❌ [DEBUG] Token injection failed: {}", e);
                return SwitchAccountResult {
                    success: false,
                    message: format!("Failed to inject token: {}", e),
                    details,
                };
            }
        }

        // 3. Update storage.json if possible
        match Self::update_storage_json(&target_account.email, &target_account.token) {
            Ok(()) => {
                details.push("Successfully updated storage.json".to_string());
            }
            Err(e) => {
                details.push(format!("Warning: Failed to update storage.json: {}", e));
            }
        }

        // 4. Inject email update JavaScript to Cursor UI
        match MachineIdRestorer::new() {
            Ok(restorer) => match restorer.inject_email_update_js(&target_account.email) {
                Ok(()) => {
                    details
                        .push("Successfully injected email update script to Cursor UI".to_string());
                }
                Err(e) => {
                    details.push(format!(
                        "Warning: Failed to inject email update script: {}",
                        e
                    ));
                }
            },
            Err(e) => {
                details.push(format!(
                    "Warning: Failed to initialize email updater: {}",
                    e
                ));
            }
        }

        // Wait for database updates to complete (CRITICAL!)
        log_debug!("🔍 [DEBUG] Legacy switch - Waiting for database updates to complete...");
        std::thread::sleep(std::time::Duration::from_millis(500));
        log_info!("✅ [DEBUG] Legacy switch - Database update wait completed");
        details.push("Waited for database updates to complete".to_string());

        // 5. Restart Cursor if auto_restart is enabled
        if auto_restart {
            log_debug!("🔍 [DEBUG] Auto-restart enabled, starting Cursor...");

            // Wait a bit more to ensure all database operations are complete
            std::thread::sleep(std::time::Duration::from_millis(1000));

            // Wait for Cursor processes to be completely terminated before starting
            let mut wait_count = 0;
            while Self::is_cursor_running() && wait_count < 10 {
                log_debug!(
                    "🔍 [DEBUG] Waiting for Cursor processes to terminate... ({}/10)",
                    wait_count + 1
                );
                std::thread::sleep(std::time::Duration::from_millis(500));
                wait_count += 1;
            }

            if Self::is_cursor_running() {
                log_warn!(
                    "⚠️ [DEBUG] Cursor processes still running after waiting, attempting to start anyway"
                );
                details.push(
                    "Warning: Cursor processes still running when attempting restart".to_string(),
                );
            }

            match Self::start_cursor() {
                Ok(()) => {
                    log_info!("✅ [DEBUG] Successfully restarted Cursor");
                    details.push("Successfully restarted Cursor".to_string());
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] Failed to restart Cursor: {}", e);
                    details.push(format!("Warning: Failed to restart Cursor: {}", e));

                    // 不要因为启动失败而让整个操作失败
                    log_info!(
                        "ℹ️ [DEBUG] Account switch completed successfully despite Cursor restart failure"
                    );
                }
            }
        } else {
            log_info!("⏭️ [DEBUG] Auto-restart disabled, skipping Cursor startup");
            details.push("Skipped Cursor restart (auto-restart disabled)".to_string());
        }

        SwitchAccountResult {
            success: true,
            message: format!("Successfully switched to account: {}", email),
            details,
        }
    }

    /// Inject email to SQLite database with complete email fields
    fn inject_email_to_sqlite(email: &str) -> Result<()> {
        log_info!(
            "🔍 [DEBUG] inject_email_to_sqlite called with email: {}",
            email
        );

        let (_, sqlite_path) = Self::get_cursor_paths()?;
        log_debug!("🔍 [DEBUG] SQLite path: {:?}", sqlite_path);

        if !sqlite_path.exists() {
            log_info!(
                "❌ [DEBUG] SQLite database not found at path: {:?}",
                sqlite_path
            );
            return Err(anyhow!("SQLite database not found"));
        }

        log_debug!("🔍 [DEBUG] Opening SQLite connection...");
        let conn = Connection::open(&sqlite_path)?;
        log_info!("✅ [DEBUG] SQLite connection opened successfully");

        // Set database optimization parameters (skip PRAGMA for now to avoid issues)
        log_debug!("🔍 [DEBUG] Email - Skipping PRAGMA settings to avoid compatibility issues");

        // Begin transaction
        log_debug!("🔍 [DEBUG] Email - Beginning transaction...");
        conn.execute("BEGIN TRANSACTION", [])?;
        log_info!("✅ [DEBUG] Email - Transaction begun successfully");

        // Complete list of email fields to update - based on CursorPool_Client implementation
        let email_fields = vec![
            ("cursorAuth/cachedEmail", email), // Primary email field
            ("cursor.email", email),           // Additional email field
        ];

        let mut success_count = 0;

        for (key, value) in email_fields {
            log_debug!("🔍 [DEBUG] Processing email field: {} = {}", key, value);

            // Check if record exists using direct query
            log_debug!("🔍 [DEBUG] Checking if record exists for key: {}", key);
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM ItemTable WHERE key = ?",
                [key],
                |row| row.get(0),
            )?;
            log_debug!("🔍 [DEBUG] Record exists check result: {}", exists);

            if exists > 0 {
                // Update existing record
                log_info!(
                    "🔍 [DEBUG] Email - Updating existing record for key: {}",
                    key
                );
                match conn.execute("UPDATE ItemTable SET value = ? WHERE key = ?", [value, key]) {
                    Ok(rows_affected) => {
                        if rows_affected > 0 {
                            log_info!(
                                "✅ [DEBUG] Updated email field: {} (rows affected: {})",
                                key,
                                rows_affected
                            );
                            success_count += 1;
                        }
                    }
                    Err(e) => {
                        log_error!("❌ [DEBUG] Failed to update email field {}: {}", key, e);
                    }
                }
            } else {
                // Insert new record
                log_debug!("🔍 [DEBUG] Email - Inserting new record for key: {}", key);
                match conn.execute(
                    "INSERT INTO ItemTable (key, value) VALUES (?, ?)",
                    [key, value],
                ) {
                    Ok(_) => {
                        log_info!("✅ [DEBUG] Inserted new email field: {}", key);
                        success_count += 1;
                    }
                    Err(e) => {
                        log_error!("❌ [DEBUG] Failed to insert email field {}: {}", key, e);
                    }
                }
            }
        }

        if success_count > 0 {
            // Commit transaction
            log_info!(
                "🔍 [DEBUG] Email - Committing transaction with {} successful updates",
                success_count
            );
            conn.execute("COMMIT", [])?;
            log_info!(
                "✅ [DEBUG] Successfully updated {} email fields",
                success_count
            );
        } else {
            // Rollback transaction
            log_error!("❌ [DEBUG] Email - Rolling back transaction, no successful updates");
            conn.execute("ROLLBACK", [])?;
            return Err(anyhow!("Failed to update any email fields"));
        }

        Ok(())
    }

    /// Inject token to SQLite database with complete authentication fields
    fn inject_token_to_sqlite(token: &str) -> Result<()> {
        log_info!(
            "🔍 [DEBUG] inject_token_to_sqlite called with token length: {}",
            token.len()
        );

        let (_, sqlite_path) = Self::get_cursor_paths()?;
        log_info!(
            "🔍 [DEBUG] Token injection - SQLite path: {:?}",
            sqlite_path
        );

        if !sqlite_path.exists() {
            log_info!(
                "❌ [DEBUG] Token injection - SQLite database not found at path: {:?}",
                sqlite_path
            );
            return Err(anyhow!("SQLite database not found"));
        }

        log_debug!("🔍 [DEBUG] Token injection - Opening SQLite connection...");
        let conn = Connection::open(&sqlite_path)?;
        log_info!("✅ [DEBUG] Token injection - SQLite connection opened successfully");

        // Process token - handle formats like "user_01XXX%3A%3Atoken" or "user_01XXX::token"
        let processed_token = if token.contains("%3A%3A") {
            token.split("%3A%3A").nth(1).unwrap_or(token)
        } else if token.contains("::") {
            token.split("::").nth(1).unwrap_or(token)
        } else {
            token
        };

        log_info!(
            "Processing token: original length {}, processed length {}",
            token.len(),
            processed_token.len()
        );

        // Set database optimization parameters (skip PRAGMA for now to avoid issues)
        log_debug!("🔍 [DEBUG] Token - Skipping PRAGMA settings to avoid compatibility issues");

        // Begin transaction
        log_debug!("🔍 [DEBUG] Token - Beginning transaction...");
        conn.execute("BEGIN TRANSACTION", [])?;
        log_info!("✅ [DEBUG] Token - Transaction begun successfully");

        // Complete list of authentication fields to update - this is the key fix!
        let auth_fields = vec![
            ("cursorAuth/accessToken", processed_token),
            ("cursorAuth/refreshToken", processed_token), // refreshToken = accessToken
            ("cursor.accessToken", processed_token),      // Additional token field
            ("cursorAuth/cachedSignUpType", "Auth_0"),    // Authentication type - CRITICAL!
        ];

        let mut success_count = 0;

        for (key, value) in auth_fields {
            log_debug!("🔍 [DEBUG] Processing token field: {} = {}", key, value);

            // Check if record exists using direct query
            log_info!(
                "🔍 [DEBUG] Token - Checking if record exists for key: {}",
                key
            );
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM ItemTable WHERE key = ?",
                [key],
                |row| row.get(0),
            )?;
            log_debug!("🔍 [DEBUG] Token - Record exists check result: {}", exists);

            if exists > 0 {
                // Update existing record
                log_info!(
                    "🔍 [DEBUG] Token - Updating existing record for key: {}",
                    key
                );
                match conn.execute("UPDATE ItemTable SET value = ? WHERE key = ?", [value, key]) {
                    Ok(rows_affected) => {
                        if rows_affected > 0 {
                            log_info!(
                                "✅ [DEBUG] Updated token field: {} (rows affected: {})",
                                key,
                                rows_affected
                            );
                            success_count += 1;
                        }
                    }
                    Err(e) => {
                        log_error!("❌ [DEBUG] Failed to update token field {}: {}", key, e);
                    }
                }
            } else {
                // Insert new record
                log_debug!("🔍 [DEBUG] Token - Inserting new record for key: {}", key);
                match conn.execute(
                    "INSERT INTO ItemTable (key, value) VALUES (?, ?)",
                    [key, value],
                ) {
                    Ok(_) => {
                        log_info!("✅ [DEBUG] Inserted new token field: {}", key);
                        success_count += 1;
                    }
                    Err(e) => {
                        log_error!("❌ [DEBUG] Failed to insert token field {}: {}", key, e);
                    }
                }
            }
        }

        if success_count > 0 {
            // Commit transaction
            log_info!(
                "🔍 [DEBUG] Token - Committing transaction with {} successful updates",
                success_count
            );
            conn.execute("COMMIT", [])?;
            log_info!(
                "✅ [DEBUG] Successfully updated {} authentication fields",
                success_count
            );
        } else {
            // Rollback transaction
            log_error!("❌ [DEBUG] Token - Rolling back transaction, no successful updates");
            conn.execute("ROLLBACK", [])?;
            return Err(anyhow!("Failed to update any authentication fields"));
        }

        Ok(())
    }

    /// Inject token to SQLite database with custom auth type
    fn inject_token_to_sqlite_with_auth_type(token: &str, auth_type: &str) -> Result<()> {
        let (_, sqlite_path) = Self::get_cursor_paths()?;

        if !sqlite_path.exists() {
            return Err(anyhow!("SQLite database not found"));
        }

        let conn = Connection::open(&sqlite_path)?;

        // Process token - handle formats like "user_01XXX%3A%3Atoken" or "user_01XXX::token"
        let processed_token = if token.contains("%3A%3A") {
            token.split("%3A%3A").nth(1).unwrap_or(token)
        } else if token.contains("::") {
            token.split("::").nth(1).unwrap_or(token)
        } else {
            token
        };

        log_info!(
            "Processing token with auth type {}: original length {}, processed length {}",
            auth_type,
            token.len(),
            processed_token.len()
        );

        // Set database optimization parameters (skip PRAGMA for now to avoid issues)
        log_info!(
            "🔍 [DEBUG] Token with auth type - Skipping PRAGMA settings to avoid compatibility issues"
        );

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", [])?;

        // Complete list of authentication fields to update with custom auth type
        let auth_fields = vec![
            ("cursorAuth/accessToken", processed_token),
            ("cursorAuth/refreshToken", processed_token), // refreshToken = accessToken
            ("cursor.accessToken", processed_token),      // Additional token field
            ("cursorAuth/cachedSignUpType", auth_type),   // Custom authentication type
        ];

        let mut success_count = 0;

        for (key, value) in auth_fields {
            // Check if record exists using direct query
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM ItemTable WHERE key = ?",
                [key],
                |row| row.get(0),
            )?;

            if exists > 0 {
                // Update existing record
                match conn.execute("UPDATE ItemTable SET value = ? WHERE key = ?", [value, key]) {
                    Ok(rows_affected) => {
                        if rows_affected > 0 {
                            log_info!("Updated field: {} (rows affected: {})", key, rows_affected);
                            success_count += 1;
                        }
                    }
                    Err(e) => {
                        log_info!("Failed to update field {}: {}", key, e);
                    }
                }
            } else {
                // Insert new record
                match conn.execute(
                    "INSERT INTO ItemTable (key, value) VALUES (?, ?)",
                    [key, value],
                ) {
                    Ok(_) => {
                        log_info!("Inserted new field: {}", key);
                        success_count += 1;
                    }
                    Err(e) => {
                        log_info!("Failed to insert field {}: {}", key, e);
                    }
                }
            }
        }

        if success_count > 0 {
            // Commit transaction
            conn.execute("COMMIT", [])?;
            log_info!(
                "Successfully updated {} authentication fields with auth type {}",
                success_count,
                auth_type
            );
        } else {
            // Rollback transaction
            conn.execute("ROLLBACK", [])?;
            return Err(anyhow!("Failed to update any authentication fields"));
        }

        Ok(())
    }

    /// Check if Cursor is running
    pub fn is_cursor_running() -> bool {
        use std::process::Command;

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("tasklist")
                .args(&["/FI", "IMAGENAME eq Cursor.exe"])
                .output();

            if let Ok(output) = output {
                let output_str = String::from_utf8_lossy(&output.stdout);
                return output_str.contains("Cursor.exe");
            }
        }

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("pgrep").args(&["-f", "Cursor"]).output();

            if let Ok(output) = output {
                return !output.stdout.is_empty();
            }
        }

        #[cfg(target_os = "linux")]
        {
            // 更精确地匹配 Cursor IDE 进程，排除 auto-cursor
            // 方法1: 尝试匹配 cursor 可执行文件（通常在 .cursor-server 或 AppImage 中）
            let output = Command::new("pgrep").args(&["-f", "cursor.*--"]).output();

            if let Ok(output) = output {
                if !output.stdout.is_empty() {
                    return true;
                }
            }

            // 方法2: 尝试匹配包含 .cursor 配置目录的进程
            let output2 = Command::new("pgrep").args(&["-f", "\\.cursor"]).output();

            if let Ok(output2) = output2 {
                if !output2.stdout.is_empty() {
                    // 需要排除 auto-cursor 进程
                    let pids = String::from_utf8_lossy(&output2.stdout);
                    let current_pid = std::process::id();

                    for pid in pids.lines() {
                        if let Ok(pid_num) = pid.trim().parse::<u32>() {
                            if pid_num != current_pid {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Force close Cursor processes
    pub fn force_close_cursor() -> Result<()> {
        use std::process::Command;

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("taskkill")
                .args(&["/F", "/IM", "Cursor.exe"])
                .output();

            match output {
                Ok(_) => {
                    log_info!("✅ [DEBUG] Windows: Cursor processes terminated");
                    Ok(())
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] Windows: Failed to terminate Cursor: {}", e);
                    Err(anyhow!("Failed to terminate Cursor on Windows: {}", e))
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("pkill").args(&["-f", "Cursor"]).output();

            match output {
                Ok(_) => {
                    log_info!("✅ [DEBUG] macOS: Cursor processes terminated");
                    Ok(())
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] macOS: Failed to terminate Cursor: {}", e);
                    Err(anyhow!("Failed to terminate Cursor on macOS: {}", e))
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // 更精确地终止 Cursor IDE 进程，避免误杀 auto-cursor
            // 方法1: 尝试终止匹配 cursor.*-- 的进程（Cursor IDE 的特征）
            let output1 = Command::new("pkill").args(&["-f", "cursor.*--"]).output();

            // 方法2: 获取所有包含 .cursor 的进程，排除当前进程后终止
            let current_pid = std::process::id();
            let pgrep_output = Command::new("pgrep").args(&["-f", "\\.cursor"]).output();

            if let Ok(pgrep_result) = pgrep_output {
                let pids = String::from_utf8_lossy(&pgrep_result.stdout);
                for pid in pids.lines() {
                    if let Ok(pid_num) = pid.trim().parse::<u32>() {
                        if pid_num != current_pid {
                            // 终止该进程
                            let _ = Command::new("kill").args(&["-9", &pid.trim()]).output();
                        }
                    }
                }
            }

            // 检查是否有 Cursor AppImage 进程
            let appimage_output = Command::new("pkill")
                .args(&["-f", "cursor.*AppImage"])
                .output();

            match (output1, appimage_output) {
                (Ok(_), Ok(_)) => {
                    log_info!("✅ [DEBUG] Linux: Cursor processes terminated");
                    Ok(())
                }
                _ => {
                    log_info!("✅ [DEBUG] Linux: Attempted to terminate Cursor processes");
                    Ok(())
                }
            }
        }
    }

    /// Force kill all Cursor processes using more aggressive methods
    pub fn force_kill_cursor_processes() -> Result<u32> {
        use std::process::Command;
        let mut killed_count = 0u32;

        #[cfg(target_os = "windows")]
        {
            // Windows: Use gentler approach - only try basic taskkill without aggressive methods
            log_info!("🔍 [DEBUG] Windows: Using gentle process termination approach");

            // First try to find if any Cursor processes are running
            let check_output = Command::new("tasklist")
                .args(&["/FI", "IMAGENAME eq Cursor*"])
                .output();

            match check_output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("Cursor") {
                        log_info!(
                            "🔍 [DEBUG] Windows: Found Cursor processes, attempting gentle termination"
                        );

                        // Try gentle termination first
                        if let Ok(gentle_output) = Command::new("taskkill")
                            .args(&["/IM", "Cursor.exe"])
                            .output()
                        {
                            if gentle_output.status.success() {
                                killed_count += 1;
                                log_info!("✅ [DEBUG] Windows: Gently terminated Cursor processes");
                            } else {
                                log_debug!(
                                    "ℹ️ [DEBUG] Windows: Gentle termination had no effect (process may not exist)"
                                );
                            }
                        }

                        // Wait a moment for gentle shutdown
                        std::thread::sleep(std::time::Duration::from_millis(1000));

                        // Check if processes are still running, if so, try force kill as last resort
                        if let Ok(recheck_output) = Command::new("tasklist")
                            .args(&["/FI", "IMAGENAME eq Cursor*"])
                            .output()
                        {
                            let recheck_stdout = String::from_utf8_lossy(&recheck_output.stdout);
                            if recheck_stdout.contains("Cursor") {
                                log_warn!(
                                    "⚠️ [DEBUG] Windows: Processes still running, trying force termination as last resort"
                                );

                                if let Ok(force_output) = Command::new("taskkill")
                                    .args(&["/F", "/IM", "Cursor.exe"])
                                    .output()
                                {
                                    if force_output.status.success() {
                                        killed_count += 1;
                                        log_info!(
                                            "✅ [DEBUG] Windows: Force terminated remaining processes"
                                        );
                                    } else {
                                        log_debug!(
                                            "ℹ️ [DEBUG] Windows: Force termination completed (may have been no processes to kill)"
                                        );
                                    }
                                }
                            } else {
                                log_info!(
                                    "✅ [DEBUG] Windows: All Cursor processes terminated successfully"
                                );
                            }
                        }
                    } else {
                        log_info!("ℹ️ [DEBUG] Windows: No Cursor processes found to terminate");
                    }
                }
                Err(e) => {
                    log_debug!(
                        "⚠️ [DEBUG] Windows: Could not check for Cursor processes: {}",
                        e
                    );
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Use multiple aggressive methods for macOS
            let commands = vec![
                ("pkill", vec!["-9", "-f", "Cursor"]),
                ("pkill", vec!["-9", "-f", "cursor"]),
                ("pkill", vec!["-9", "-f", "/Applications/Cursor.app"]),
                ("killall", vec!["-9", "Cursor"]),
                ("killall", vec!["-9", "cursor"]),
            ];

            for (cmd, args) in commands {
                match Command::new(cmd).args(&args).output() {
                    Ok(output) => {
                        if output.status.success() {
                            killed_count += 1;
                            log_info!("✅ [DEBUG] macOS: Successfully executed {} {:?}", cmd, args);
                        }
                    }
                    Err(e) => {
                        log_debug!(
                            "⚠️ [DEBUG] macOS: Failed to execute {} {:?}: {}",
                            cmd,
                            args,
                            e
                        );
                    }
                }
            }

            // Additional method: find and kill by process name pattern
            if let Ok(output) = Command::new("pgrep").args(&["-f", "Cursor"]).output() {
                let pids = String::from_utf8_lossy(&output.stdout);
                for pid in pids.lines() {
                    if let Ok(_) = Command::new("kill").args(&["-9", pid.trim()]).output() {
                        killed_count += 1;
                        log_info!("✅ [DEBUG] macOS: Force killed PID {}", pid.trim());
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Use multiple aggressive methods for Linux
            let current_pid = std::process::id();

            // Method 1: Kill by pattern matching
            let patterns = vec![
                "cursor",
                "Cursor",
                "\\.cursor",
                "cursor.*--",
                "cursor.*AppImage",
            ];

            for pattern in patterns {
                if let Ok(output) = Command::new("pgrep").args(&["-f", pattern]).output() {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    for pid in pids.lines() {
                        if let Ok(pid_num) = pid.trim().parse::<u32>() {
                            if pid_num != current_pid {
                                if let Ok(_) =
                                    Command::new("kill").args(&["-9", &pid.trim()]).output()
                                {
                                    killed_count += 1;
                                    log_info!(
                                        "✅ [DEBUG] Linux: Force killed PID {} (pattern: {})",
                                        pid.trim(),
                                        pattern
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // Method 2: Use pkill with force
            let pkill_commands = vec![
                vec!["-9", "-f", "cursor"],
                vec!["-9", "-f", "Cursor"],
                vec!["-9", "-f", "\\.cursor"],
                vec!["-9", "-f", "cursor.*--"],
                vec!["-9", "-f", "cursor.*AppImage"],
            ];

            for args in pkill_commands {
                if let Ok(_) = Command::new("pkill").args(&args).output() {
                    killed_count += 1;
                    log_info!("✅ [DEBUG] Linux: Successfully executed pkill {:?}", args);
                }
            }
        }

        log_info!(
            "✅ [DEBUG] Force kill completed, total operations: {}",
            killed_count
        );
        Ok(killed_count)
    }

    /// Start Cursor application
    pub fn start_cursor() -> Result<()> {
        use crate::machine_id::MachineIdRestorer;
        use std::process::Command;

        #[cfg(target_os = "windows")]
        {
            // First, try to use custom path if available
            if let Ok(restorer) = MachineIdRestorer::new() {
                if let Some(custom_path) = restorer.get_custom_cursor_path() {
                    log_info!("🎯 [DEBUG] 检测到自定义Cursor路径: {}", custom_path);

                    // 尝试多种可能的 Cursor.exe 位置
                    let custom_path_buf = std::path::PathBuf::from(&custom_path);
                    let possible_exe_paths = vec![
                        // 如果自定义路径直接指向 resources/app，则 exe 在上两级目录
                        custom_path_buf
                            .parent()
                            .and_then(|p| p.parent())
                            .map(|p| p.join("Cursor.exe")),
                        // 如果自定义路径指向安装目录，则 exe 在同级目录
                        Some(custom_path_buf.join("Cursor.exe")),
                        // 如果自定义路径指向 Cursor.exe 本身
                        if custom_path.ends_with("Cursor.exe") {
                            Some(custom_path_buf.clone())
                        } else {
                            None
                        },
                    ];

                    for exe_path_opt in possible_exe_paths {
                        if let Some(exe_path) = exe_path_opt {
                            log_info!("🔍 [DEBUG] 检查自定义Cursor.exe路径: {:?}", exe_path);

                            if exe_path.exists() {
                                match Command::new(&exe_path).spawn() {
                                    Ok(_) => {
                                        log_info!(
                                            "✅ [DEBUG] Windows: Successfully started Cursor from custom path: {:?}",
                                            exe_path
                                        );
                                        return Ok(());
                                    }
                                    Err(e) => {
                                        log_error!(
                                            "❌ [DEBUG] Windows: Failed to start Cursor from custom path {}: {}",
                                            exe_path.display(),
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }

                    log_warn!(
                        "⚠️ [DEBUG] Windows: Could not find Cursor.exe using custom path, falling back to default paths"
                    );
                }
            }

            // Try multiple possible paths for Cursor on Windows
            let possible_paths = vec![
                r"C:\Users\%USERNAME%\AppData\Local\Programs\cursor\Cursor.exe",
                r"C:\Program Files\Cursor\Cursor.exe",
                r"C:\Program Files (x86)\Cursor\Cursor.exe",
            ];

            for path in possible_paths {
                let expanded_path =
                    path.replace("%USERNAME%", &std::env::var("USERNAME").unwrap_or_default());
                if std::path::Path::new(&expanded_path).exists() {
                    match Command::new(&expanded_path).spawn() {
                        Ok(_) => {
                            log_info!(
                                "✅ [DEBUG] Windows: Successfully started Cursor from {}",
                                expanded_path
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            log_debug!(
                                "⚠️ [DEBUG] Windows: Failed to start Cursor from {}: {}",
                                expanded_path,
                                e
                            );
                        }
                    }
                }
            }

            // Try using 'start' command as fallback
            match Command::new("cmd").args(&["/C", "start", "cursor"]).spawn() {
                Ok(_) => {
                    log_info!("✅ [DEBUG] Windows: Started Cursor using 'start cursor' command");
                    Ok(())
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] Windows: Failed to start Cursor: {}", e);
                    Err(anyhow!("Failed to start Cursor on Windows: {}", e))
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Try to open Cursor app on macOS
            match Command::new("open").args(&["-a", "Cursor"]).spawn() {
                Ok(_) => {
                    log_info!("✅ [DEBUG] macOS: Successfully started Cursor");
                    Ok(())
                }
                Err(e) => {
                    // Try alternative path
                    match Command::new("open")
                        .args(&["/Applications/Cursor.app"])
                        .spawn()
                    {
                        Ok(_) => {
                            log_info!(
                                "✅ [DEBUG] macOS: Successfully started Cursor from /Applications/Cursor.app"
                            );
                            Ok(())
                        }
                        Err(e2) => {
                            log_error!(
                                "❌ [DEBUG] macOS: Failed to start Cursor: {} (also tried: {})",
                                e,
                                e2
                            );
                            Err(anyhow!("Failed to start Cursor on macOS: {}", e))
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Try multiple methods to start Cursor on Linux
            let commands = vec![
                ("cursor", vec![]),
                ("Cursor", vec![]),
                ("/usr/bin/cursor", vec![]),
                ("/usr/local/bin/cursor", vec![]),
                ("flatpak", vec!["run", "com.cursor.Cursor"]),
                ("snap", vec!["run", "cursor"]),
            ];

            for (cmd, args) in commands {
                match Command::new(cmd).args(&args).spawn() {
                    Ok(_) => {
                        log_info!(
                            "✅ [DEBUG] Linux: Successfully started Cursor using {} {:?}",
                            cmd,
                            args
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        log_debug!(
                            "⚠️ [DEBUG] Linux: Failed to start Cursor using {} {:?}: {}",
                            cmd,
                            args,
                            e
                        );
                    }
                }
            }

            // Try to find and execute Cursor AppImage
            if let Ok(output) = Command::new("find")
                .args(&[
                    "/home",
                    "-name",
                    "*cursor*.AppImage",
                    "-type",
                    "f",
                    "2>/dev/null",
                ])
                .output()
            {
                let appimages = String::from_utf8_lossy(&output.stdout);
                for appimage in appimages.lines() {
                    if let Ok(_) = Command::new(appimage.trim()).spawn() {
                        log_info!(
                            "✅ [DEBUG] Linux: Successfully started Cursor AppImage: {}",
                            appimage.trim()
                        );
                        return Ok(());
                    }
                }
            }

            log_error!("❌ [DEBUG] Linux: Failed to start Cursor using any method");
            Err(anyhow!("Failed to start Cursor on Linux"))
        }
    }

    /// Update storage.json with new email and token (CRITICAL for authentication!)
    fn update_storage_json(email: &str, token: &str) -> Result<()> {
        use fs2::FileExt;

        log_info!(
            "🔍 [DEBUG] Updating storage.json with email: {}, token length: {}",
            email,
            token.len()
        );

        let (storage_path, _) = Self::get_cursor_paths()?;
        log_debug!("🔍 [DEBUG] Storage.json path: {:?}", storage_path);

        if !storage_path.exists() {
            log_info!(
                "❌ [DEBUG] storage.json not found at path: {:?}",
                storage_path
            );
            return Err(anyhow!("storage.json not found"));
        }

        // 使用文件锁保护并发访问
        let file = fs::File::options()
            .read(true)
            .write(true)
            .open(&storage_path)?;

        log_debug!("🔒 [DEBUG] Acquiring exclusive lock on storage.json");
        file.lock_exclusive()?;

        let content = fs::read_to_string(&storage_path)?;
        let mut data: serde_json::Value = serde_json::from_str(&content)?;
        log_info!("✅ [DEBUG] Successfully read and parsed storage.json (with lock)");

        // Process token - handle formats like "user_01XXX%3A%3Atoken" or "user_01XXX::token"
        let processed_token = if token.contains("%3A%3A") {
            token.split("%3A%3A").nth(1).unwrap_or(token)
        } else if token.contains("::") {
            token.split("::").nth(1).unwrap_or(token)
        } else {
            token
        };
        log_info!(
            "🔍 [DEBUG] Processed token length: {}",
            processed_token.len()
        );

        // Update ALL critical authentication fields in storage.json
        if let Some(obj) = data.as_object_mut() {
            // Core authentication fields - CRITICAL!
            obj.insert(
                "cursorAuth/cachedEmail".to_string(),
                serde_json::Value::String(email.to_string()),
            );
            obj.insert(
                "cursorAuth/accessToken".to_string(),
                serde_json::Value::String(processed_token.to_string()),
            );
            obj.insert(
                "cursorAuth/refreshToken".to_string(),
                serde_json::Value::String(processed_token.to_string()),
            );
            obj.insert(
                "cursorAuth/cachedSignUpType".to_string(),
                serde_json::Value::String("Auth_0".to_string()),
            );

            // Additional fields for compatibility
            obj.insert(
                "cursor.email".to_string(),
                serde_json::Value::String(email.to_string()),
            );
            obj.insert(
                "cursor.accessToken".to_string(),
                serde_json::Value::String(processed_token.to_string()),
            );

            log_info!("✅ [DEBUG] Updated all authentication fields in storage.json");
        }

        let updated_content = serde_json::to_string_pretty(&data)?;
        fs::write(&storage_path, updated_content)?;
        log_info!("✅ [DEBUG] Successfully wrote updated storage.json");

        // 文件锁会在 file 变量离开作用域时自动释放
        file.unlock()?;
        log_debug!("🔓 [DEBUG] Released exclusive lock on storage.json");

        Ok(())
    }

    /// Logout current account - clear all authentication data
    pub fn logout_current_account() -> LogoutResult {
        let mut details = Vec::new();
        let mut success = true;

        log_debug!("🔍 [DEBUG] Starting logout process...");

        // 1. Force close Cursor if running
        if Self::is_cursor_running() {
            details.push("Cursor is running, attempting to close...".to_string());
            match Self::force_close_cursor() {
                Ok(()) => {
                    details.push("Successfully closed Cursor".to_string());
                    // Wait for process to fully terminate
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
                Err(e) => {
                    details.push(format!("Warning: Failed to close Cursor: {}", e));
                }
            }
        } else {
            details.push("Cursor is not running".to_string());
        }

        // 2. Clear SQLite database authentication data
        match Self::clear_sqlite_auth_data() {
            Ok(()) => {
                details.push("Successfully cleared SQLite authentication data".to_string());
            }
            Err(e) => {
                success = false;
                details.push(format!("Failed to clear SQLite data: {}", e));
            }
        }

        // 3. Clear storage.json authentication data
        match Self::clear_storage_json_auth_data() {
            Ok(()) => {
                details.push("Successfully cleared storage.json authentication data".to_string());
            }
            Err(e) => {
                details.push(format!("Warning: Failed to clear storage.json: {}", e));
            }
        }

        // 4. Wait for changes to be written
        std::thread::sleep(std::time::Duration::from_millis(500));

        LogoutResult {
            success,
            message: if success {
                "Successfully logged out. Please restart Cursor to complete the logout process."
                    .to_string()
            } else {
                "Logout completed with some warnings. Please restart Cursor.".to_string()
            },
            details,
        }
    }

    /// Clear authentication data from SQLite database
    fn clear_sqlite_auth_data() -> Result<()> {
        log_debug!("🔍 [DEBUG] Clearing SQLite authentication data...");

        let (_, sqlite_path) = Self::get_cursor_paths()?;

        if !sqlite_path.exists() {
            log_error!("❌ [DEBUG] SQLite database not found");
            return Err(anyhow!("SQLite database not found"));
        }

        let conn = Connection::open(&sqlite_path)?;
        log_info!("✅ [DEBUG] SQLite connection opened successfully");

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", [])?;

        // List of authentication fields to clear
        let auth_fields = vec![
            "cursorAuth/accessToken",
            "cursorAuth/refreshToken",
            "cursorAuth/cachedEmail",
            "cursorAuth/cachedSignUpType",
            "cursor.email",
            "cursor.accessToken",
        ];

        let mut cleared_count = 0;
        for field in auth_fields {
            match conn.execute("DELETE FROM ItemTable WHERE key = ?", [field]) {
                Ok(changes) => {
                    if changes > 0 {
                        log_info!("✅ [DEBUG] Cleared field: {}", field);
                        cleared_count += 1;
                    } else {
                        log_info!("ℹ️ [DEBUG] Field not found: {}", field);
                    }
                }
                Err(e) => {
                    log_error!("❌ [DEBUG] Failed to clear field {}: {}", field, e);
                }
            }
        }

        // Commit transaction
        conn.execute("COMMIT", [])?;
        log_info!("✅ [DEBUG] Transaction committed successfully");
        log_info!("📊 [DEBUG] Cleared {} authentication fields", cleared_count);

        Ok(())
    }

    /// Clear authentication data from storage.json
    fn clear_storage_json_auth_data() -> Result<()> {
        log_debug!("🔍 [DEBUG] Clearing storage.json authentication data...");

        let (storage_path, _) = Self::get_cursor_paths()?;

        if !storage_path.exists() {
            log_error!("❌ [DEBUG] storage.json not found");
            return Err(anyhow!("storage.json not found"));
        }

        let content = fs::read_to_string(&storage_path)?;
        let mut data: serde_json::Value = serde_json::from_str(&content)?;
        log_info!("✅ [DEBUG] Successfully read storage.json");

        // List of authentication fields to remove
        let auth_fields = vec![
            "cursorAuth/cachedEmail",
            "cursorAuth/accessToken",
            "cursorAuth/refreshToken",
            "cursorAuth/cachedSignUpType",
            "cursor.email",
            "cursor.accessToken",
        ];

        let mut removed_count = 0;
        if let Some(obj) = data.as_object_mut() {
            for field in auth_fields {
                if obj.remove(field).is_some() {
                    log_info!("✅ [DEBUG] Removed field: {}", field);
                    removed_count += 1;
                } else {
                    log_info!("ℹ️ [DEBUG] Field not found: {}", field);
                }
            }
        }

        let updated_content = serde_json::to_string_pretty(&data)?;
        fs::write(&storage_path, updated_content)?;
        log_info!("✅ [DEBUG] Successfully updated storage.json");
        log_info!("📊 [DEBUG] Removed {} authentication fields", removed_count);

        Ok(())
    }

    /// Edit an existing account
    pub fn edit_account(
        email: String,
        new_token: Option<String>,
        new_refresh_token: Option<String>,
        new_workos_cursor_session_token: Option<String>,
    ) -> Result<()> {
        log_info!(
            "🔍 [DEBUG] AccountManager::edit_account called for email: {}",
            email
        );

        let mut accounts = Self::load_accounts()?;
        log_debug!("🔍 [DEBUG] Loaded {} accounts", accounts.len());

        let account = accounts.iter_mut().find(|acc| acc.email == email);

        match account {
            Some(acc) => {
                log_debug!("🔍 [DEBUG] Found account to edit: {}", acc.email);

                let mut updated = false;
                if let Some(token) = new_token {
                    log_debug!("🔍 [DEBUG] Updating token (length: {})", token.len());
                    acc.token = token;
                    updated = true;
                }
                if let Some(refresh_token) = new_refresh_token {
                    log_info!(
                        "🔍 [DEBUG] Updating refresh_token (length: {})",
                        refresh_token.len()
                    );
                    acc.refresh_token = Some(refresh_token);
                    updated = true;
                }
                if let Some(workos_token) = new_workos_cursor_session_token {
                    log_info!(
                        "🔍 [DEBUG] Updating workos_cursor_session_token (length: {})",
                        workos_token.len()
                    );
                    acc.workos_cursor_session_token = Some(workos_token);
                    updated = true;
                }

                if updated {
                    log_debug!("🔍 [DEBUG] Saving updated accounts to file...");
                    Self::save_accounts(&accounts)?;
                    log_info!("✅ [DEBUG] Account updated and saved successfully");
                } else {
                    log_info!("ℹ️ [DEBUG] No changes to save");
                }

                Ok(())
            }
            None => {
                log_error!("❌ [DEBUG] Account not found: {}", email);
                Err(anyhow!("Account not found"))
            }
        }
    }

    /// Update custom tags for an account
    pub fn update_custom_tags(email: String, custom_tags: Vec<CustomTag>) -> Result<()> {
        log_info!(
            "🔍 [DEBUG] AccountManager::update_custom_tags called for email: {}",
            email
        );

        let mut accounts = Self::load_accounts()?;
        log_debug!("🔍 [DEBUG] Loaded {} accounts", accounts.len());

        let account = accounts.iter_mut().find(|acc| acc.email == email);

        match account {
            Some(acc) => {
                log_debug!("🔍 [DEBUG] Found account to update tags: {}", acc.email);
                acc.custom_tags = Some(custom_tags);

                log_debug!("🔍 [DEBUG] Saving updated accounts with custom tags...");
                Self::save_accounts(&accounts)?;
                log_info!("✅ [DEBUG] Account custom tags updated successfully");

                Ok(())
            }
            None => {
                log_error!("❌ [DEBUG] Account not found: {}", email);
                Err(anyhow!("Account not found"))
            }
        }
    }

    /// Remove an account
    pub fn remove_account(email: String) -> Result<()> {
        let mut accounts = Self::load_accounts()?;

        let initial_len = accounts.len();
        accounts.retain(|acc| acc.email != email);

        if accounts.len() == initial_len {
            return Err(anyhow!("Account not found"));
        }

        Self::save_accounts(&accounts)?;
        Ok(())
    }

    /// Export accounts to a specified directory
    pub fn export_accounts(export_path: String) -> Result<String> {
        log_info!(
            "🔍 [DEBUG] AccountManager::export_accounts called with path: {}",
            export_path
        );

        let account_file = Self::get_account_file_path()?;
        log_debug!("🔍 [DEBUG] Source account file: {:?}", account_file);

        if !account_file.exists() {
            return Err(anyhow!("Account file does not exist"));
        }

        let export_file_path = PathBuf::from(&export_path).join("account.json");
        log_debug!("🔍 [DEBUG] Export destination: {:?}", export_file_path);

        // Ensure the export directory exists
        if let Some(parent) = export_file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create export directory: {}", e))?;
        }

        // Copy the account file to the export location
        fs::copy(&account_file, &export_file_path)
            .map_err(|e| anyhow!("Failed to copy account file: {}", e))?;

        log_info!(
            "✅ [DEBUG] Account file exported successfully to: {:?}",
            export_file_path
        );
        Ok(export_file_path.to_string_lossy().to_string())
    }

    /// Import accounts from a specified file
    pub fn import_accounts(import_file_path: String) -> Result<String> {
        log_info!(
            "🔍 [DEBUG] AccountManager::import_accounts called with file: {}",
            import_file_path
        );

        let import_path = PathBuf::from(&import_file_path);
        if !import_path.exists() {
            return Err(anyhow!("Import file does not exist"));
        }

        // Validate the imported file by trying to parse it
        let import_content = fs::read_to_string(&import_path)
            .map_err(|e| anyhow!("Failed to read import file: {}", e))?;

        let _imported_accounts: Vec<AccountInfo> = serde_json::from_str(&import_content)
            .map_err(|e| anyhow!("Invalid account file format: {}", e))?;

        let current_account_file = Self::get_account_file_path()?;
        log_info!(
            "🔍 [DEBUG] Current account file: {:?}",
            current_account_file
        );

        // Create backup of current account file if it exists
        if current_account_file.exists() {
            let backup_path = current_account_file.with_file_name("account_back.json");
            log_debug!("🔍 [DEBUG] Creating backup at: {:?}", backup_path);

            fs::copy(&current_account_file, &backup_path)
                .map_err(|e| anyhow!("Failed to create backup: {}", e))?;

            log_info!("✅ [DEBUG] Backup created successfully");
        }

        // Copy the imported file to replace the current account file
        fs::copy(&import_path, &current_account_file)
            .map_err(|e| anyhow!("Failed to import account file: {}", e))?;

        log_info!("✅ [DEBUG] Account file imported successfully");
        Ok(format!(
            "Successfully imported {} accounts",
            _imported_accounts.len()
        ))
    }
}
