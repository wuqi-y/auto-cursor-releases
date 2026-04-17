use crate::{log_debug, log_error, log_info, log_warn};
use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use dirs;
use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAuthInfo {
    pub is_authorized: bool,
    pub token_length: usize,
    pub token_valid: bool,
    pub api_status: Option<u16>,
    pub error_message: Option<String>,
    pub checksum: Option<String>,
    pub account_info: Option<AccountInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub email: Option<String>,
    pub username: Option<String>,
    pub subscription_type: Option<String>,
    pub subscription_status: Option<String>,
    pub trial_days_remaining: Option<i32>,
    pub usage_info: Option<String>,
    pub aggregated_usage: Option<AggregatedUsageData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedUsageData {
    pub aggregations: Vec<ModelUsage>,
    pub total_input_tokens: String,
    pub total_output_tokens: String,
    pub total_cache_write_tokens: String,
    pub total_cache_read_tokens: String,
    pub total_cost_cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model_intent: String,
    pub input_tokens: String,
    pub output_tokens: String,
    pub cache_write_tokens: String,
    pub cache_read_tokens: String,
    pub total_cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRequest {
    pub start_date: u64,
    pub end_date: u64,
    pub team_id: i32,
}

// 用户分析数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalyticsData {
    #[serde(rename = "dailyMetrics")]
    pub daily_metrics: Vec<DailyMetric>,
    pub period: Period,
    #[serde(rename = "totalMembersInTeam")]
    pub total_members_in_team: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMetric {
    pub date: String,
    #[serde(rename = "activeUsers", default)]
    pub active_users: Option<i32>,
    #[serde(rename = "acceptedLinesAdded", default)]
    pub accepted_lines_added: Option<i32>,
    #[serde(rename = "acceptedLinesDeleted", default)]
    pub accepted_lines_deleted: Option<i32>,
    #[serde(rename = "totalApplies", default)]
    pub total_applies: Option<i32>,
    #[serde(rename = "totalAccepts", default)]
    pub total_accepts: Option<i32>,
    #[serde(rename = "totalTabsShown", default)]
    pub total_tabs_shown: Option<i32>,
    #[serde(rename = "totalTabsAccepted", default)]
    pub total_tabs_accepted: Option<i32>,
    #[serde(rename = "composerRequests", default)]
    pub composer_requests: Option<i32>,
    #[serde(rename = "agentRequests", default)]
    pub agent_requests: Option<i32>,
    #[serde(rename = "subscriptionIncludedReqs", default)]
    pub subscription_included_reqs: Option<i32>,
    #[serde(rename = "modelUsage", default)]
    pub model_usage: Option<Vec<ModelCount>>,
    #[serde(rename = "extensionUsage", default)]
    pub extension_usage: Option<Vec<NameCount>>,
    #[serde(rename = "tabExtensionUsage", default)]
    pub tab_extension_usage: Option<Vec<NameCount>>,
    #[serde(rename = "clientVersionUsage", default)]
    pub client_version_usage: Option<Vec<NameCount>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Period {
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCount {
    pub name: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameCount {
    pub name: String,
    pub count: i32,
}

// 过滤的使用事件数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredUsageEventsData {
    #[serde(rename = "totalUsageEventsCount")]
    pub total_usage_events_count: i32,
    #[serde(rename = "usageEventsDisplay")]
    pub usage_events_display: Vec<UsageEventDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEventDisplay {
    pub timestamp: String,
    pub model: String,
    pub kind: String,
    #[serde(rename = "requestsCosts", default)]
    pub requests_costs: Option<f64>,
    #[serde(rename = "usageBasedCosts")]
    pub usage_based_costs: String,
    #[serde(rename = "isTokenBasedCall")]
    pub is_token_based_call: bool,
    #[serde(rename = "tokenUsage", default)]
    pub token_usage: Option<TokenUsageDetail>,
    #[serde(rename = "owningUser")]
    pub owning_user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageDetail {
    #[serde(rename = "inputTokens")]
    pub input_tokens: Option<i32>,
    #[serde(rename = "outputTokens")]
    pub output_tokens: Option<i32>,
    #[serde(rename = "cacheWriteTokens")]
    pub cache_write_tokens: Option<i32>,
    #[serde(rename = "cacheReadTokens")]
    pub cache_read_tokens: Option<i32>,
    #[serde(rename = "totalCents")]
    pub total_cents: Option<f64>,
}

// 过滤使用事件请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredUsageRequest {
    #[serde(rename = "teamId")]
    pub team_id: i32,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub page: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
}

// 用户分析请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalyticsRequest {
    #[serde(rename = "teamId")]
    pub team_id: i32,
    #[serde(rename = "userId")]
    pub user_id: i32,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCheckResult {
    pub success: bool,
    pub user_info: Option<UserAuthInfo>,
    pub message: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token: Option<String>,
    pub source: String,
    pub found: bool,
    pub message: String,
}

pub struct AuthChecker;

impl AuthChecker {
    pub fn new() -> Self {
        Self
    }

    /// Find Cursor installation paths by searching common locations
    fn find_cursor_paths() -> Result<Vec<PathBuf>> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

        let mut possible_paths = Vec::new();

        #[cfg(target_os = "macos")]
        {
            possible_paths.extend([
                home_dir.join("Library/Application Support/Cursor"),
                home_dir.join("Library/Application Support/cursor"),
                PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/out/vs/workbench"),
            ]);
        }

        #[cfg(target_os = "windows")]
        {
            possible_paths.extend([
                home_dir.join("AppData/Roaming/Cursor"),
                home_dir.join("AppData/Local/Cursor"),
                home_dir.join("AppData/Roaming/cursor"),
                home_dir.join("AppData/Local/cursor"),
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            possible_paths.extend([
                home_dir.join(".config/Cursor"),
                home_dir.join(".config/cursor"),
                home_dir.join(".cursor"),
                PathBuf::from("/opt/cursor"),
                PathBuf::from("/usr/share/cursor"),
            ]);
        }

        // Filter to only existing paths
        let existing_paths: Vec<PathBuf> = possible_paths
            .into_iter()
            .filter(|path| path.exists())
            .collect();

        Ok(existing_paths)
    }

    /// Get Cursor paths for different platforms
    fn get_cursor_paths() -> Result<(PathBuf, PathBuf, PathBuf)> {
        let cursor_paths = Self::find_cursor_paths()?;

        if cursor_paths.is_empty() {
            return Err(anyhow!("No Cursor installation found"));
        }

        // Try each found path to see if it contains the expected structure
        for base_path in &cursor_paths {
            let storage_path = base_path.join("User/globalStorage/storage.json");
            let sqlite_path = base_path.join("User/globalStorage/state.vscdb"); // 修正：指向具体的 SQLite 文件
            let session_path = base_path.join("Session Storage");

            // If at least one of these paths exists, use this base path
            if storage_path.exists() || sqlite_path.exists() || session_path.exists() {
                return Ok((storage_path, sqlite_path, session_path));
            }
        }

        // If no valid structure found, return the first path anyway for error reporting
        let base_path = &cursor_paths[0];
        let storage_path = base_path.join("User/globalStorage/storage.json");
        let sqlite_path = base_path.join("User/globalStorage/state.vscdb"); // 修正：指向具体的 SQLite 文件
        let session_path = base_path.join("Session Storage");

        Ok((storage_path, sqlite_path, session_path))
    }

    /// Try to get token from storage.json
    fn get_token_from_storage(storage_path: &PathBuf) -> Result<Option<String>> {
        if !storage_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(storage_path)?;
        let storage_data: serde_json::Value = serde_json::from_str(&content)?;

        // Try to get cursorAuth/accessToken first (most likely location)
        if let Some(token) = storage_data
            .get("cursorAuth/accessToken")
            .and_then(|v| v.as_str())
        {
            if !token.is_empty() && token.len() > 20 {
                return Ok(Some(token.to_string()));
            }
        }

        // Try other possible keys containing "token"
        if let Some(obj) = storage_data.as_object() {
            for (key, value) in obj {
                if key.to_lowercase().contains("token") {
                    if let Some(token) = value.as_str() {
                        if !token.is_empty() && token.len() > 20 {
                            return Ok(Some(token.to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Try to get token from SQLite database
    fn get_token_from_sqlite(sqlite_path: &PathBuf) -> Result<Option<String>> {
        if !sqlite_path.exists() {
            return Ok(None);
        }

        let conn = Connection::open(sqlite_path)?;

        let mut stmt = conn.prepare("SELECT value FROM ItemTable WHERE key LIKE '%token%'")?;
        let rows = stmt.query_map([], |row| Ok(row.get::<_, String>(0)?))?;

        for row in rows {
            if let Ok(value) = row {
                if value.len() > 20 {
                    // First try to return the value directly if it looks like a token
                    if !value.starts_with('{') && !value.starts_with('[') {
                        return Ok(Some(value));
                    }

                    // Try to parse as JSON
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&value) {
                        if let Some(token) = json_value.get("token").and_then(|v| v.as_str()) {
                            if !token.is_empty() && token.len() > 20 {
                                return Ok(Some(token.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Try to get token from session storage
    fn get_token_from_session(session_path: &PathBuf) -> Result<Option<String>> {
        if !session_path.exists() {
            return Ok(None);
        }

        let entries = fs::read_dir(session_path)?;
        let token_regex = Regex::new(r#""token":"([^"]+)""#)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                if let Ok(content) = fs::read(&path) {
                    // Try to decode as UTF-8, ignore errors
                    let content_str = String::from_utf8_lossy(&content);

                    if let Some(captures) = token_regex.captures(&content_str) {
                        if let Some(token) = captures.get(1) {
                            let token_str = token.as_str();
                            if !token_str.is_empty() && token_str.len() > 20 {
                                return Ok(Some(token_str.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Try to get token from environment variables
    fn get_token_from_env() -> Option<String> {
        std::env::var("CURSOR_TOKEN")
            .ok()
            .or_else(|| std::env::var("CURSOR_AUTH_TOKEN").ok())
            .filter(|token| !token.is_empty())
    }

    /// Debug method to show all possible Cursor paths
    pub fn debug_cursor_paths() -> Result<Vec<String>> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

        let mut debug_info = Vec::new();
        debug_info.push(format!("Home directory: {}", home_dir.display()));

        let cursor_paths = Self::find_cursor_paths()?;
        debug_info.push(format!(
            "Found {} Cursor installation paths:",
            cursor_paths.len()
        ));

        for (i, path) in cursor_paths.iter().enumerate() {
            debug_info.push(format!(
                "  {}. {} (exists: {})",
                i + 1,
                path.display(),
                path.exists()
            ));

            // Check subdirectories
            let storage_path = path.join("User/globalStorage/storage.json");
            let sqlite_path = path.join("User/workspaceStorage");
            let session_path = path.join("Session Storage");

            debug_info.push(format!(
                "     Storage: {} (exists: {})",
                storage_path.display(),
                storage_path.exists()
            ));
            debug_info.push(format!(
                "     SQLite:  {} (exists: {})",
                sqlite_path.display(),
                sqlite_path.exists()
            ));
            debug_info.push(format!(
                "     Session: {} (exists: {})",
                session_path.display(),
                session_path.exists()
            ));

            // List contents of User directory if it exists
            let user_dir = path.join("User");
            if user_dir.exists() {
                debug_info.push(format!("     User directory contents:"));
                if let Ok(entries) = fs::read_dir(&user_dir) {
                    for entry in entries.flatten() {
                        debug_info
                            .push(format!("       - {}", entry.file_name().to_string_lossy()));
                    }
                }
            }
        }

        Ok(debug_info)
    }

    /// Auto-detect and get token from various sources
    pub fn get_token_auto() -> TokenInfo {
        // Get Cursor paths first
        let paths = match Self::get_cursor_paths() {
            Ok(paths) => paths,
            Err(e) => {
                return TokenInfo {
                    token: None,
                    source: "Error".to_string(),
                    found: false,
                    message: format!("Error getting Cursor paths: {}", e),
                };
            }
        };

        let (storage_path, sqlite_path, session_path) = paths;

        // Try SQLite database first (highest priority)
        match Self::get_token_from_sqlite(&sqlite_path) {
            Ok(Some(token)) => {
                return TokenInfo {
                    token: Some(token),
                    source: "Cursor SQLite Database".to_string(),
                    found: true,
                    message: format!("Token found in SQLite database: {}", sqlite_path.display()),
                };
            }
            Ok(None) => {
                // Continue to next method
            }
            Err(e) => {
                // Log error but continue to next method
                log_error!("Error reading SQLite database: {}", e);
            }
        }

        // Try storage.json second
        match Self::get_token_from_storage(&storage_path) {
            Ok(Some(token)) => {
                return TokenInfo {
                    token: Some(token),
                    source: "Cursor Storage (storage.json)".to_string(),
                    found: true,
                    message: format!("Token found in storage file: {}", storage_path.display()),
                };
            }
            Ok(None) => {
                // Continue to next method
            }
            Err(e) => {
                // Log error but continue to next method
                log_error!("Error reading storage.json: {}", e);
            }
        }

        // Try session storage
        match Self::get_token_from_session(&session_path) {
            Ok(Some(token)) => {
                return TokenInfo {
                    token: Some(token),
                    source: "Cursor Session Storage".to_string(),
                    found: true,
                    message: format!("Token found in session storage: {}", session_path.display()),
                };
            }
            Ok(None) => {
                // Continue to next method
            }
            Err(e) => {
                // Log error but continue
                log_error!("Error reading session storage: {}", e);
            }
        }

        // Try environment variables last
        if let Some(token) = Self::get_token_from_env() {
            return TokenInfo {
                token: Some(token),
                source: "Environment Variable".to_string(),
                found: true,
                message: "Token found in environment variables".to_string(),
            };
        }

        // No token found
        TokenInfo {
            token: None,
            source: "None".to_string(),
            found: false,
            message: format!(
                "No token found in any location. Searched:\n- SQLite: {}\n- Storage: {}\n- Session: {}\n- Environment variables",
                sqlite_path.display(),
                storage_path.display(),
                session_path.display()
            ),
        }
    }

    /// Generate a SHA-256 hash of input + salt and return as hex
    fn generate_hashed64_hex(input: &str, salt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}{}", input, salt).as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Obfuscate bytes using the algorithm from utils.js
    fn obfuscate_bytes(mut byte_array: Vec<u8>) -> Vec<u8> {
        let mut t = 165u8;
        for (r, byte) in byte_array.iter_mut().enumerate() {
            *byte = ((*byte ^ t).wrapping_add((r % 256) as u8)) & 0xFF;
            t = *byte;
        }
        byte_array
    }

    /// Generate Cursor checksum from token using the algorithm
    fn generate_cursor_checksum(token: &str) -> Result<String> {
        let clean_token = token.trim();

        // Generate machineId and macMachineId
        let machine_id = Self::generate_hashed64_hex(clean_token, "machineId");
        let mac_machine_id = Self::generate_hashed64_hex(clean_token, "macMachineId");

        // Get timestamp and convert to byte array
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64 / 1000000;

        // Convert timestamp to bytes and take last 6 bytes
        let timestamp_bytes = timestamp.to_be_bytes();
        let byte_array = timestamp_bytes[2..].to_vec(); // Take last 6 bytes

        // Obfuscate bytes and encode as base64
        let obfuscated_bytes = Self::obfuscate_bytes(byte_array);
        let encoded_checksum = general_purpose::STANDARD.encode(&obfuscated_bytes);

        // Combine final checksum
        Ok(format!(
            "{}{}/{}",
            encoded_checksum, machine_id, mac_machine_id
        ))
    }

    /// Public wrapper for clean_token (for use in other modules)
    pub fn clean_token_public(token: &str) -> Result<String> {
        Self::clean_token(token)
    }

    /// Public wrapper for generate_cursor_checksum (for use in other modules)
    pub fn generate_cursor_checksum_public(token: &str) -> Result<String> {
        Self::generate_cursor_checksum(token)
    }

    /// Clean and validate token
    fn clean_token(token: &str) -> Result<String> {
        let mut clean_token = token.to_string();

        // Handle URL encoded tokens
        if clean_token.contains("%3A%3A") {
            clean_token = clean_token
                .split("%3A%3A")
                .nth(1)
                .ok_or_else(|| anyhow!("Invalid token format"))?
                .to_string();
        } else if clean_token.contains("::") {
            clean_token = clean_token
                .split("::")
                .nth(1)
                .ok_or_else(|| anyhow!("Invalid token format"))?
                .to_string();
        }

        clean_token = clean_token.trim().to_string();

        if clean_token.is_empty() || clean_token.len() < 10 {
            return Err(anyhow!("Token is too short or empty"));
        }

        Ok(clean_token)
    }

    /// Check if token looks like a valid JWT
    fn is_jwt_like(token: &str) -> bool {
        token.starts_with("eyJ") && token.contains('.') && token.len() > 100
    }

    /// Get email from local storage files
    fn get_email_from_local_storage() -> Option<String> {
        // Try to get email from storage.json first
        if let Some(email) = Self::get_email_from_storage() {
            return Some(email);
        }

        // If not found in storage.json, try SQLite database
        if let Some(email) = Self::get_email_from_sqlite() {
            return Some(email);
        }

        None
    }

    /// Get email from storage.json
    fn get_email_from_storage() -> Option<String> {
        use fs2::FileExt;

        if let Some(storage_path) = Self::get_cursor_storage_path() {
            // 使用共享锁进行读取操作
            if let Ok(file) = std::fs::File::open(&storage_path) {
                if let Ok(_) = file.lock_shared() {
                    if let Ok(content) = std::fs::read_to_string(&storage_path) {
                        if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&content) {
                            // Try cursorAuth/cachedEmail first
                            if let Some(email) = json_data.get("cursorAuth/cachedEmail") {
                                if let Some(email_str) = email.as_str() {
                                    log_info!("📧 从storage.json找到邮箱: {}", email_str);
                                    let _ = file.unlock();
                                    return Some(email_str.to_string());
                                }
                            }

                            // Try other email fields
                            if let Some(obj) = json_data.as_object() {
                                for (key, value) in obj {
                                    if key.to_lowercase().contains("email") {
                                        if let Some(email_str) = value.as_str() {
                                            if email_str.contains('@') {
                                                log_info!(
                                                    "📧 从storage.json的{}字段找到邮箱: {}",
                                                    key,
                                                    email_str
                                                );
                                                let _ = file.unlock();
                                                return Some(email_str.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let _ = file.unlock();
                    }
                }
            }
        }
        None
    }

    /// Get email from SQLite database
    fn get_email_from_sqlite() -> Option<String> {
        if let Some(sqlite_path) = Self::get_cursor_sqlite_path() {
            match rusqlite::Connection::open(&sqlite_path) {
                Ok(conn) => {
                    log_debug!("🔍 正在从SQLite数据库查找邮箱: {}", sqlite_path);

                    // Query records containing email or cursorAuth
                    let query = "SELECT value FROM ItemTable WHERE key LIKE '%email%' OR key LIKE '%cursorAuth%'";

                    match conn.prepare(query) {
                        Ok(mut stmt) => {
                            match stmt.query_map([], |row| {
                                let value: String = row.get(0)?;
                                Ok(value)
                            }) {
                                Ok(rows) => {
                                    for row_result in rows {
                                        if let Ok(value) = row_result {
                                            // If it's a string and contains @, it might be an email
                                            if value.contains('@')
                                                && value.len() > 5
                                                && value.len() < 100
                                            {
                                                log_info!("📧 从SQLite直接找到邮箱: {}", value);
                                                return Some(value);
                                            }

                                            // Try to parse as JSON
                                            if let Ok(json_data) =
                                                serde_json::from_str::<serde_json::Value>(&value)
                                            {
                                                if let Some(obj) = json_data.as_object() {
                                                    // Check for email field
                                                    if let Some(email) = obj.get("email") {
                                                        if let Some(email_str) = email.as_str() {
                                                            log_info!(
                                                                "📧 从SQLite JSON email字段找到邮箱: {}",
                                                                email_str
                                                            );
                                                            return Some(email_str.to_string());
                                                        }
                                                    }

                                                    // Check for cachedEmail field
                                                    if let Some(cached_email) =
                                                        obj.get("cachedEmail")
                                                    {
                                                        if let Some(email_str) =
                                                            cached_email.as_str()
                                                        {
                                                            log_info!(
                                                                "📧 从SQLite JSON cachedEmail字段找到邮箱: {}",
                                                                email_str
                                                            );
                                                            return Some(email_str.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log_error!("❌ SQLite查询执行失败: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log_error!("❌ SQLite查询准备失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ 无法打开SQLite数据库: {}", e);
                }
            }
        }
        None
    }

    /// Get Cursor SQLite database path
    fn get_cursor_sqlite_path() -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            let home_dir = std::env::var("HOME").ok()?;
            let sqlite_path = format!(
                "{}/Library/Application Support/Cursor/User/globalStorage/state.vscdb",
                home_dir
            );
            log_debug!("🔍 检查macOS SQLite路径: {}", sqlite_path);
            if std::path::Path::new(&sqlite_path).exists() {
                log_info!("✅ 找到SQLite文件: {}", sqlite_path);
                Some(sqlite_path)
            } else {
                log_error!("❌ SQLite文件不存在: {}", sqlite_path);
                None
            }
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").ok()?;
            let sqlite_path = format!("{}\\Cursor\\User\\globalStorage\\state.vscdb", appdata);
            log_debug!("🔍 检查Windows SQLite路径: {}", sqlite_path);
            if std::path::Path::new(&sqlite_path).exists() {
                log_info!("✅ 找到SQLite文件: {}", sqlite_path);
                Some(sqlite_path)
            } else {
                log_error!("❌ SQLite文件不存在: {}", sqlite_path);
                None
            }
        }

        #[cfg(target_os = "linux")]
        {
            let home_dir = std::env::var("HOME").ok()?;
            let sqlite_path = format!("{}/.config/Cursor/User/globalStorage/state.vscdb", home_dir);
            log_debug!("🔍 检查Linux SQLite路径: {}", sqlite_path);
            if std::path::Path::new(&sqlite_path).exists() {
                log_info!("✅ 找到SQLite文件: {}", sqlite_path);
                Some(sqlite_path)
            } else {
                log_error!("❌ SQLite文件不存在: {}", sqlite_path);
                None
            }
        }
    }

    /// Get Cursor storage.json path
    fn get_cursor_storage_path() -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            let home_dir = std::env::var("HOME").ok()?;
            let storage_path = format!(
                "{}/Library/Application Support/Cursor/User/globalStorage/storage.json",
                home_dir
            );
            log_debug!("🔍 检查macOS存储路径: {}", storage_path);
            if std::path::Path::new(&storage_path).exists() {
                log_info!("✅ 找到存储文件: {}", storage_path);
                Some(storage_path)
            } else {
                log_error!("❌ 存储文件不存在: {}", storage_path);
                None
            }
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").ok()?;
            let storage_path = format!("{}\\Cursor\\User\\globalStorage\\storage.json", appdata);
            log_debug!("🔍 检查Windows存储路径: {}", storage_path);
            if std::path::Path::new(&storage_path).exists() {
                log_info!("✅ 找到存储文件: {}", storage_path);
                Some(storage_path)
            } else {
                log_error!("❌ 存储文件不存在: {}", storage_path);
                None
            }
        }

        #[cfg(target_os = "linux")]
        {
            let home_dir = std::env::var("HOME").ok()?;
            let storage_path = format!(
                "{}/.config/Cursor/User/globalStorage/storage.json",
                home_dir
            );
            log_debug!("🔍 检查Linux存储路径: {}", storage_path);
            if std::path::Path::new(&storage_path).exists() {
                log_info!("✅ 找到存储文件: {}", storage_path);
                Some(storage_path)
            } else {
                log_error!("❌ 存储文件不存在: {}", storage_path);
                None
            }
        }
    }

    /// Get aggregated usage data from Cursor API
    async fn get_aggregated_usage_data(
        workos_session_token: &str,
        start_date: u64,
        end_date: u64,
        team_id: i32,
        details: &mut Vec<String>,
    ) -> Result<Option<AggregatedUsageData>> {
        let client = reqwest::Client::new();
        log_info!("当前webtoken: {}", workos_session_token);

        details.push("Attempting to get aggregated usage data...".to_string());
        log_debug!("🔍 正在获取聚合用量数据...");

        let mut usage_headers = reqwest::header::HeaderMap::new();
        usage_headers.insert("Accept", "*/*".parse()?);
        usage_headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse()?);
        usage_headers.insert(
            "Accept-Language",
            "en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7".parse()?,
        );
        usage_headers.insert("Content-Type", "application/json".parse()?);
        usage_headers.insert("Origin", "https://cursor.com".parse()?);
        usage_headers.insert("Referer", "https://cursor.com/cn/dashboard".parse()?);
        usage_headers.insert(
            "Sec-CH-UA",
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\""
                .parse()?,
        );
        usage_headers.insert("Sec-CH-UA-Arch", "\"x86\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Bitness", "\"64\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        usage_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Platform-Version", "\"15.3.1\"".parse()?);
        usage_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        usage_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        usage_headers.insert("Sec-Fetch-Site", "same-origin".parse()?);
        usage_headers.insert(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36".parse()?,
        );
        // Use WorkOS Session Token from account list
        usage_headers.insert(
            "Cookie",
            format!("WorkosCursorSessionToken={}", workos_session_token).parse()?,
        );

        let request_body = serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "teamId": team_id
        });

        let usage_response = client
            .post("https://cursor.com/api/dashboard/get-aggregated-usage-events")
            .headers(usage_headers)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match usage_response {
            Ok(resp) => {
                let status = resp.status();
                log_info!("📡 聚合用量API响应状态: {}", status);
                details.push(format!("Aggregated usage API response status: {}", status));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            log_info!("📦 聚合用量响应数据长度: {} bytes", body.len());
                            log_info!("📝 聚合用量响应内容: {}", body);
                            details.push(format!(
                                "Aggregated usage response body length: {} bytes",
                                body.len()
                            ));

                            // Try to parse JSON response
                            if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&body)
                            {
                                log_info!("✅ 成功解析聚合用量JSON数据");

                                // Parse aggregated usage data according to the new structure
                                let mut aggregations = Vec::new();

                                if let Some(agg_array) =
                                    json_data.get("aggregations").and_then(|v| v.as_array())
                                {
                                    for agg in agg_array {
                                        if let Some(model_intent) =
                                            agg.get("modelIntent").and_then(|v| v.as_str())
                                        {
                                            let model_usage = ModelUsage {
                                                model_intent: model_intent.to_string(),
                                                input_tokens: agg
                                                    .get("inputTokens")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("0")
                                                    .to_string(),
                                                output_tokens: agg
                                                    .get("outputTokens")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("0")
                                                    .to_string(),
                                                cache_write_tokens: agg
                                                    .get("cacheWriteTokens")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("0")
                                                    .to_string(),
                                                cache_read_tokens: agg
                                                    .get("cacheReadTokens")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("0")
                                                    .to_string(),
                                                total_cents: agg
                                                    .get("totalCents")
                                                    .and_then(|v| v.as_f64())
                                                    .unwrap_or(0.0),
                                            };
                                            aggregations.push(model_usage);
                                        }
                                    }
                                }

                                let aggregated_usage = AggregatedUsageData {
                                    aggregations,
                                    total_input_tokens: json_data
                                        .get("totalInputTokens")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("0")
                                        .to_string(),
                                    total_output_tokens: json_data
                                        .get("totalOutputTokens")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("0")
                                        .to_string(),
                                    total_cache_write_tokens: json_data
                                        .get("totalCacheWriteTokens")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("0")
                                        .to_string(),
                                    total_cache_read_tokens: json_data
                                        .get("totalCacheReadTokens")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("0")
                                        .to_string(),
                                    total_cost_cents: json_data
                                        .get("totalCostCents")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0),
                                };

                                return Ok(Some(aggregated_usage));
                            } else {
                                log_error!("❌ 无法解析聚合用量JSON数据");
                                details
                                    .push("Failed to parse aggregated usage JSON data".to_string());
                            }
                        }
                        Err(e) => {
                            log_error!("❌ 读取聚合用量响应体失败: {}", e);
                            details.push(format!(
                                "Failed to read aggregated usage response body: {}",
                                e
                            ));
                        }
                    }
                } else {
                    log_error!("❌ 聚合用量API失败，状态码: {}", status);
                    details.push(format!(
                        "Aggregated usage API failed with status: {}",
                        status
                    ));
                }
            }
            Err(e) => {
                log_error!("❌ 聚合用量API请求失败: {}", e);
                details.push(format!("Aggregated usage API request failed: {}", e));
            }
        }

        Ok(None)
    }

    /// Get user analytics data from Cursor API
    async fn get_user_analytics_data(
        workos_session_token: &str,
        team_id: i32,
        user_id: i32,
        start_date: &str,
        end_date: &str,
        details: &mut Vec<String>,
    ) -> Result<Option<UserAnalyticsData>> {
        let client = reqwest::Client::builder()
            .gzip(true)
            .deflate(true)
            .brotli(true)
            .build()?;

        let mut analytics_headers = reqwest::header::HeaderMap::new();
        analytics_headers.insert("Accept", "application/json, text/plain, */*".parse()?);
        analytics_headers.insert("Accept-Encoding", "gzip, deflate, br".parse()?);
        analytics_headers.insert("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".parse()?);
        analytics_headers.insert("Cache-Control", "no-cache".parse()?);
        analytics_headers.insert("Content-Type", "application/json".parse()?);
        analytics_headers.insert("Origin", "https://cursor.com".parse()?);
        analytics_headers.insert("Pragma", "no-cache".parse()?);
        analytics_headers.insert("Referer", "https://cursor.com/dashboard".parse()?);
        analytics_headers.insert(
            "Sec-CH-UA",
            "\"Chromium\";v=\"131\", \"Google Chrome\";v=\"131\", \"Not_A Brand\";v=\"24\""
                .parse()?,
        );
        analytics_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        analytics_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        analytics_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        analytics_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        analytics_headers.insert("Sec-Fetch-Site", "same-origin".parse()?);
        analytics_headers.insert("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".parse()?);
        analytics_headers.insert(
            "Cookie",
            format!("WorkosCursorSessionToken={}", workos_session_token).parse()?,
        );

        let request_body = UserAnalyticsRequest {
            team_id,
            user_id,
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
        };

        log_info!("🔄 发送用户分析API请求到: https://cursor.com/api/dashboard/get-user-analytics");
        log_info!("📦 请求参数: {:?}", request_body);

        let analytics_response = client
            .post("https://cursor.com/api/dashboard/get-user-analytics")
            .headers(analytics_headers)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match analytics_response {
            Ok(resp) => {
                let status = resp.status();
                log_info!("📡 用户分析API响应状态: {}", status);
                details.push(format!("User analytics API response status: {}", status));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            log_info!("📦 用户分析响应数据长度: {} bytes", body.len());
                            log_info!("📝 用户分析响应内容: {}", body);
                            details.push(format!(
                                "User analytics response body length: {} bytes",
                                body.len()
                            ));

                            // Try to parse JSON response
                            match serde_json::from_str::<UserAnalyticsData>(&body) {
                                Ok(analytics_data) => {
                                    log_info!("✅ 成功解析用户分析数据");
                                    details.push(
                                        "Successfully parsed user analytics data".to_string(),
                                    );
                                    return Ok(Some(analytics_data));
                                }
                                Err(e) => {
                                    log_error!("❌ 解析用户分析数据失败: {}", e);
                                    details.push(format!(
                                        "Failed to parse user analytics data: {}",
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            log_error!("❌ 读取用户分析响应失败: {}", e);
                            details.push(format!("Failed to read user analytics response: {}", e));
                        }
                    }
                } else {
                    log_error!("❌ 用户分析API返回错误状态码: {}", status);
                    details.push(format!(
                        "User analytics API returned error status: {}",
                        status
                    ));
                }
            }
            Err(e) => {
                log_error!("❌ 用户分析API请求失败: {}", e);
                details.push(format!("User analytics API request failed: {}", e));
            }
        }

        Ok(None)
    }

    /// Get filtered usage events data from Cursor API
    async fn get_filtered_usage_events(
        workos_session_token: &str,
        team_id: i32,
        start_date: &str,
        end_date: &str,
        page: i32,
        page_size: i32,
        details: &mut Vec<String>,
    ) -> Result<Option<FilteredUsageEventsData>> {
        let client = reqwest::Client::builder()
            .gzip(true)
            .deflate(true)
            .brotli(true)
            .build()?;

        let mut events_headers = reqwest::header::HeaderMap::new();
        events_headers.insert("Accept", "application/json, text/plain, */*".parse()?);
        events_headers.insert("Accept-Encoding", "gzip, deflate, br".parse()?);
        events_headers.insert("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".parse()?);
        events_headers.insert("Cache-Control", "no-cache".parse()?);
        events_headers.insert("Content-Type", "application/json".parse()?);
        events_headers.insert("Origin", "https://cursor.com".parse()?);
        events_headers.insert("Pragma", "no-cache".parse()?);
        events_headers.insert("Referer", "https://cursor.com/dashboard".parse()?);
        events_headers.insert(
            "Sec-CH-UA",
            "\"Chromium\";v=\"131\", \"Google Chrome\";v=\"131\", \"Not_A Brand\";v=\"24\""
                .parse()?,
        );
        events_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        events_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        events_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        events_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        events_headers.insert("Sec-Fetch-Site", "same-origin".parse()?);
        events_headers.insert("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".parse()?);
        events_headers.insert(
            "Cookie",
            format!("WorkosCursorSessionToken={}", workos_session_token).parse()?,
        );

        let request_body = FilteredUsageRequest {
            team_id,
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
            page,
            page_size,
        };

        log_info!(
            "🔄 发送过滤使用事件API请求到: https://cursor.com/api/dashboard/get-filtered-usage-events"
        );
        log_info!("📦 请求参数: {:?}", request_body);

        let events_response = client
            .post("https://cursor.com/api/dashboard/get-filtered-usage-events")
            .headers(events_headers)
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match events_response {
            Ok(resp) => {
                let status = resp.status();
                log_info!("📡 过滤使用事件API响应状态: {}", status);
                details.push(format!(
                    "Filtered usage events API response status: {}",
                    status
                ));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            log_info!("📦 过滤使用事件响应数据长度: {} bytes", body.len());
                            log_info!("📝 过滤使用事件响应内容: {}", body);
                            details.push(format!(
                                "Filtered usage events response body length: {} bytes",
                                body.len()
                            ));

                            // Try to parse JSON response
                            match serde_json::from_str::<FilteredUsageEventsData>(&body) {
                                Ok(events_data) => {
                                    log_info!("✅ 成功解析过滤使用事件数据");
                                    details.push(
                                        "Successfully parsed filtered usage events data"
                                            .to_string(),
                                    );
                                    return Ok(Some(events_data));
                                }
                                Err(e) => {
                                    log_error!("❌ 解析过滤使用事件数据失败: {}", e);
                                    details.push(format!(
                                        "Failed to parse filtered usage events data: {}",
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            log_error!("❌ 读取过滤使用事件响应失败: {}", e);
                            details.push(format!(
                                "Failed to read filtered usage events response: {}",
                                e
                            ));
                        }
                    }
                } else {
                    log_error!("❌ 过滤使用事件API返回错误状态码: {}", status);
                    details.push(format!(
                        "Filtered usage events API returned error status: {}",
                        status
                    ));
                }
            }
            Err(e) => {
                log_error!("❌ 过滤使用事件API请求失败: {}", e);
                details.push(format!("Filtered usage events API request failed: {}", e));
            }
        }

        Ok(None)
    }

    /// Find WorkOS Session Token for a given access token from account manager
    fn find_workos_session_token(access_token: &str) -> Option<String> {
        // Use account manager to find WorkOS session token
        match crate::account_manager::AccountManager::load_accounts() {
            Ok(accounts) => {
                for account in accounts {
                    if account.token == access_token {
                        return account.workos_cursor_session_token;
                    }
                }
            }
            Err(e) => {
                log_error!("❌ 无法加载账户列表: {}", e);
            }
        }
        None
    }

    /// Get account information from Cursor API
    async fn get_account_info(
        token: &str,
        _checksum: &str,
        details: &mut Vec<String>,
    ) -> Result<Option<AccountInfo>> {
        let client = reqwest::Client::new();

        let mut account_info = AccountInfo {
            email: None,
            username: None,
            subscription_type: None,
            subscription_status: None,
            trial_days_remaining: None,
            usage_info: None,
            aggregated_usage: None,
        };

        // First try to get email from local storage (highest priority)
        if let Some(local_email) = Self::get_email_from_local_storage() {
            account_info.email = Some(local_email.clone());
            details.push(format!("Email found in local storage: {}", local_email));
            log_info!("📧 从本地存储获取到邮箱: {}", local_email);
        } else {
            log_warn!("⚠️ 本地存储中未找到邮箱，将尝试从API获取");
            details.push("Email not found in local storage, will try API".to_string());
        }

        // Try to get subscription info using the correct API endpoint
        details.push("Attempting to get subscription info...".to_string());
        log_debug!("🔍 正在获取订阅信息...");

        let mut subscription_headers = reqwest::header::HeaderMap::new();
        subscription_headers.insert("Accept", "*/*".parse()?);
        subscription_headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse()?);
        subscription_headers.insert(
            "Accept-Language",
            "en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7".parse()?,
        );
        subscription_headers.insert("Content-Type", "application/json".parse()?);
        subscription_headers.insert("Origin", "https://cursor.com".parse()?);
        subscription_headers.insert("Referer", "https://cursor.com/cn/dashboard".parse()?);
        subscription_headers.insert(
            "Sec-CH-UA",
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\""
                .parse()?,
        );
        subscription_headers.insert("Sec-CH-UA-Arch", "\"x86\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Bitness", "\"64\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        subscription_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Platform-Version", "\"15.3.1\"".parse()?);
        subscription_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        subscription_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        subscription_headers.insert("Sec-Fetch-Site", "cross-site".parse()?);
        subscription_headers.insert(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36".parse()?,
        );
        subscription_headers.insert("Authorization", format!("Bearer {}", token).parse()?);

        let subscription_response = client
            .get("https://api2.cursor.sh/auth/full_stripe_profile")
            .headers(subscription_headers)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match subscription_response {
            Ok(resp) => {
                let status = resp.status();
                log_info!("📡 订阅API响应状态: {}", status);
                details.push(format!("Subscription API response status: {}", status));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            log_info!("📦 订阅响应数据长度: {} bytes", body.len());
                            log_info!("📝 订阅响应内容: {}", body);
                            details.push(format!(
                                "Subscription response body length: {} bytes",
                                body.len()
                            ));
                            details.push(format!("Subscription response content: {}", body));

                            // Try to parse JSON response
                            if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&body)
                            {
                                log_info!("✅ 成功解析订阅JSON数据");
                                log_info!(
                                    "🔍 JSON数据结构: {}",
                                    serde_json::to_string_pretty(&json_data)
                                        .unwrap_or_else(|_| "无法格式化".to_string())
                                );

                                // Extract email from customer info
                                if let Some(customer) = json_data.get("customer") {
                                    if let Some(email) = customer.get("email") {
                                        if let Some(email_str) = email.as_str() {
                                            account_info.email = Some(email_str.to_string());
                                            log_info!("� 找到邮箱: {}", email_str);
                                        }
                                    }
                                }

                                // Extract subscription type and status
                                if let Some(membership_type) = json_data.get("membershipType") {
                                    if let Some(membership_str) = membership_type.as_str() {
                                        account_info.subscription_type =
                                            Some(membership_str.to_string());
                                        log_info!("� 订阅类型: {}", membership_str);
                                    }
                                }

                                if let Some(subscription_status) =
                                    json_data.get("subscriptionStatus")
                                {
                                    if let Some(status_str) = subscription_status.as_str() {
                                        account_info.subscription_status =
                                            Some(status_str.to_string());
                                        log_info!("📊 订阅状态: {}", status_str);
                                    }
                                }

                                // Extract trial days remaining
                                if let Some(days_remaining) = json_data.get("daysRemainingOnTrial")
                                {
                                    if let Some(days) = days_remaining.as_i64() {
                                        account_info.trial_days_remaining = Some(days as i32);
                                        log_info!("⏰ 试用剩余天数: {}", days);
                                    }
                                }

                                account_info.usage_info = Some("订阅信息获取成功".to_string());
                            } else {
                                log_error!("❌ 无法解析订阅JSON数据");
                                account_info.subscription_status = Some("数据解析失败".to_string());
                            }
                        }
                        Err(e) => {
                            log_error!("❌ 读取订阅响应体失败: {}", e);
                            details
                                .push(format!("Failed to read subscription response body: {}", e));
                        }
                    }
                } else {
                    log_error!("❌ 订阅API失败，状态码: {}", status);
                    details.push(format!("Subscription API failed with status: {}", status));
                }
            }
            Err(e) => {
                log_error!("❌ 订阅API请求失败: {}", e);
                details.push(format!("Subscription API request failed: {}", e));
            }
        }

        // Try to get usage info using the correct API endpoint
        details.push("Attempting to get usage info...".to_string());
        log_debug!("🔍 正在获取使用情况信息...");

        let mut usage_headers = reqwest::header::HeaderMap::new();
        usage_headers.insert("Accept", "*/*".parse()?);
        usage_headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse()?);
        usage_headers.insert(
            "Accept-Language",
            "en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7".parse()?,
        );
        usage_headers.insert("Content-Type", "application/json".parse()?);
        usage_headers.insert("Origin", "https://cursor.com".parse()?);
        usage_headers.insert("Referer", "https://cursor.com/cn/dashboard".parse()?);
        usage_headers.insert(
            "Sec-CH-UA",
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\""
                .parse()?,
        );
        usage_headers.insert("Sec-CH-UA-Arch", "\"x86\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Bitness", "\"64\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        usage_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        usage_headers.insert("Sec-CH-UA-Platform-Version", "\"15.3.1\"".parse()?);
        usage_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        usage_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        usage_headers.insert("Sec-Fetch-Site", "same-origin".parse()?);
        usage_headers.insert(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36".parse()?,
        );
        // Use Cookie authentication for usage API
        // Try to find real WorkOS Session Token from account manager, fallback to legacy format
        let workos_cookie = if let Some(workos_token) = Self::find_workos_session_token(token) {
            format!("WorkosCursorSessionToken={}", workos_token)
        } else {
            // Fallback to legacy format if no WorkOS token found
            format!(
                "WorkosCursorSessionToken=user_01OOOOOOOOOOOOOOOOOOOOOOOO%3A%3A{}",
                token
            )
        };
        usage_headers.insert("Cookie", workos_cookie.parse()?);

        let user_response = client
            .get("https://cursor.com/api/usage")
            .headers(usage_headers)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match user_response {
            Ok(resp) => {
                let status = resp.status();
                log_info!("📡 使用情况API响应状态: {}", status);
                details.push(format!("Usage API response status: {}", status));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            log_info!("📦 使用情况响应数据长度: {} bytes", body.len());
                            log_info!("📝 使用情况响应内容: {}", body);
                            details
                                .push(format!("Usage response body length: {} bytes", body.len()));
                            details.push(format!("Usage response content: {}", body));

                            // Try to parse JSON response
                            if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&body)
                            {
                                log_info!("✅ 成功解析使用情况JSON数据");

                                // Extract GPT-4 usage (Premium)
                                if let Some(gpt4_data) = json_data.get("gpt-4") {
                                    if let Some(premium_usage) = gpt4_data.get("numRequestsTotal") {
                                        if let Some(max_usage) = gpt4_data.get("maxRequestUsage") {
                                            let usage_text = format!(
                                                "Premium: {}/{}",
                                                premium_usage.as_i64().unwrap_or(0),
                                                max_usage.as_i64().unwrap_or(999)
                                            );
                                            log_info!("⭐ {}", usage_text);

                                            if account_info.usage_info.is_some() {
                                                account_info.usage_info = Some(format!(
                                                    "{}, {}",
                                                    account_info.usage_info.as_ref().unwrap(),
                                                    usage_text
                                                ));
                                            } else {
                                                account_info.usage_info = Some(usage_text);
                                            }
                                        }
                                    }
                                }

                                // Extract GPT-3.5 usage (Basic)
                                if let Some(gpt35_data) = json_data.get("gpt-3.5-turbo") {
                                    if let Some(basic_usage) = gpt35_data.get("numRequestsTotal") {
                                        let usage_text = format!(
                                            "Basic: {}/无限制",
                                            basic_usage.as_i64().unwrap_or(0)
                                        );
                                        log_info!("� {}", usage_text);

                                        if account_info.usage_info.is_some() {
                                            account_info.usage_info = Some(format!(
                                                "{}, {}",
                                                account_info.usage_info.as_ref().unwrap(),
                                                usage_text
                                            ));
                                        } else {
                                            account_info.usage_info = Some(usage_text);
                                        }
                                    }
                                }

                                account_info.username = Some("Cursor用户".to_string());
                            } else {
                                log_error!("❌ 无法解析使用情况JSON数据");
                                if account_info.usage_info.is_none() {
                                    account_info.usage_info =
                                        Some("使用情况数据解析失败".to_string());
                                }
                            }
                        }
                        Err(e) => {
                            log_error!("❌ 读取使用情况响应体失败: {}", e);
                            details.push(format!("Failed to read usage response body: {}", e));
                        }
                    }
                } else {
                    log_error!("❌ 使用情况API失败，状态码: {}", status);
                    details.push(format!("Usage API failed with status: {}", status));
                }
            }
            Err(e) => {
                log_error!("❌ 使用情况API请求失败: {}", e);
                details.push(format!("Usage API request failed: {}", e));
            }
        }

        // Try to get aggregated usage data if we can find a WorkOS Session Token
        if let Some(workos_token) = Self::find_workos_session_token(token) {
            details.push(
                "Found WorkOS Session Token, attempting to get aggregated usage data..."
                    .to_string(),
            );
            log_info!(
                "✅ 找到 WorkOS Session Token，正在获取聚合用量数据...{}",
                workos_token
            );

            // Default to last 30 days
            let end_date = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let start_date = end_date - (30 * 24 * 60 * 60 * 1000); // 30 days ago

            match Self::get_aggregated_usage_data(&workos_token, start_date, end_date, -1, details)
                .await
            {
                Ok(Some(aggregated_usage)) => {
                    account_info.aggregated_usage = Some(aggregated_usage);
                    details.push("Successfully retrieved aggregated usage data".to_string());
                    log_info!("✅ 成功获取聚合用量数据");
                }
                Ok(None) => {
                    details.push("No aggregated usage data available".to_string());
                    log_warn!("⚠️ 无聚合用量数据");
                }
                Err(e) => {
                    details.push(format!("Failed to get aggregated usage data: {}", e));
                    log_error!("❌ 获取聚合用量数据失败: {}", e);
                }
            }
        } else {
            details.push("No WorkOS Session Token found for aggregated usage data".to_string());
            log_warn!("⚠️ 未找到 WorkOS Session Token，无法获取聚合用量数据");
        }

        Ok(Some(account_info))
    }

    /// Get aggregated usage data for a specific time period
    pub async fn get_usage_for_period(
        token: &str,
        start_date: u64,
        end_date: u64,
        team_id: i32,
    ) -> Result<Option<AggregatedUsageData>> {
        let mut details = Vec::new();

        // Find WorkOS Session Token for the given access token
        if let Some(workos_token) = Self::find_workos_session_token(token) {
            details.push("Found WorkOS Session Token for usage data request".to_string());
            log_info!("✅ 找到 WorkOS Session Token，获取指定时间段用量数据");

            Self::get_aggregated_usage_data(
                &workos_token,
                start_date,
                end_date,
                team_id,
                &mut details,
            )
            .await
        } else {
            log_error!("❌ 未找到对应的 WorkOS Session Token");
            Err(anyhow!(
                "No WorkOS Session Token found for the given access token"
            ))
        }
    }

    /// Get user analytics data for a given period
    pub async fn get_user_analytics(
        token: &str,
        team_id: i32,
        user_id: i32,
        start_date: &str,
        end_date: &str,
    ) -> Result<Option<UserAnalyticsData>> {
        let mut details = Vec::new();

        // Find WorkOS Session Token for the given access token
        if let Some(workos_token) = Self::find_workos_session_token(token) {
            details.push("Found WorkOS Session Token for user analytics request".to_string());
            log_info!("✅ 找到 WorkOS Session Token，获取用户分析数据");

            return Self::get_user_analytics_data(
                &workos_token,
                team_id,
                user_id,
                start_date,
                end_date,
                &mut details,
            )
            .await;
        } else {
            log_error!("❌ 未找到对应的 WorkOS Session Token");
            Err(anyhow!(
                "No WorkOS Session Token found for the given access token"
            ))
        }
    }

    /// Get filtered usage events for a given period
    pub async fn get_usage_events(
        token: &str,
        team_id: i32,
        start_date: &str,
        end_date: &str,
        page: i32,
        page_size: i32,
    ) -> Result<Option<FilteredUsageEventsData>> {
        let mut details = Vec::new();

        // Find WorkOS Session Token for the given access token
        if let Some(workos_token) = Self::find_workos_session_token(token) {
            details.push("Found WorkOS Session Token for usage events request".to_string());
            log_info!("✅ 找到 WorkOS Session Token，获取使用事件数据");

            return Self::get_filtered_usage_events(
                &workos_token,
                team_id,
                start_date,
                end_date,
                page,
                page_size,
                &mut details,
            )
            .await;
        } else {
            log_error!("❌ 未找到对应的 WorkOS Session Token");
            Err(anyhow!(
                "No WorkOS Session Token found for the given access token"
            ))
        }
    }

    /// Get subscription info only (lightweight, for account list)
    pub async fn get_subscription_info_only(token: &str) -> Result<AuthCheckResult> {
        let mut details = Vec::new();
        details.push("Starting subscription info check...".to_string());

        // Clean and validate token
        let clean_token = match Self::clean_token(token) {
            Ok(token) => {
                details.push(format!(
                    "Token cleaned successfully, length: {} characters",
                    token.len()
                ));
                token
            }
            Err(e) => {
                return Ok(AuthCheckResult {
                    success: false,
                    user_info: None,
                    message: "Invalid token format".to_string(),
                    details: vec![format!("Token validation failed: {}", e)],
                });
            }
        };

        // Check if token looks like JWT
        let is_authorized = if Self::is_jwt_like(&clean_token) {
            details.push("Token appears to be in JWT format".to_string());
            true
        } else {
            details.push("Token is not in JWT format".to_string());
            false
        };

        if !is_authorized {
            return Ok(AuthCheckResult {
                success: false,
                user_info: Some(UserAuthInfo {
                    is_authorized: false,
                    token_length: clean_token.len(),
                    token_valid: false,
                    api_status: None,
                    error_message: Some("Invalid token format".to_string()),
                    checksum: None,
                    account_info: None,
                }),
                message: "Invalid token format".to_string(),
                details,
            });
        }

        // Only get subscription info from full_stripe_profile
        let client = reqwest::Client::new();
        let mut account_info = AccountInfo {
            email: None,
            username: None,
            subscription_type: None,
            subscription_status: None,
            trial_days_remaining: None,
            usage_info: None,
            aggregated_usage: None,
        };

        // Get subscription info
        let mut subscription_headers = reqwest::header::HeaderMap::new();
        subscription_headers.insert("Accept", "*/*".parse()?);
        subscription_headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse()?);
        subscription_headers.insert(
            "Accept-Language",
            "en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7".parse()?,
        );
        subscription_headers.insert("Content-Type", "application/json".parse()?);
        subscription_headers.insert("Origin", "https://cursor.com".parse()?);
        subscription_headers.insert("Referer", "https://cursor.com/cn/dashboard".parse()?);
        subscription_headers.insert(
            "Sec-CH-UA",
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\""
                .parse()?,
        );
        subscription_headers.insert("Sec-CH-UA-Arch", "\"x86\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Bitness", "\"64\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Mobile", "?0".parse()?);
        subscription_headers.insert("Sec-CH-UA-Platform", "\"macOS\"".parse()?);
        subscription_headers.insert("Sec-CH-UA-Platform-Version", "\"15.3.1\"".parse()?);
        subscription_headers.insert("Sec-Fetch-Dest", "empty".parse()?);
        subscription_headers.insert("Sec-Fetch-Mode", "cors".parse()?);
        subscription_headers.insert("Sec-Fetch-Site", "cross-site".parse()?);
        subscription_headers.insert(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36".parse()?,
        );
        subscription_headers.insert("Authorization", format!("Bearer {}", clean_token).parse()?);

        let subscription_response = client
            .get("https://api2.cursor.sh/auth/full_stripe_profile")
            .headers(subscription_headers)
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        match subscription_response {
            Ok(resp) => {
                let status = resp.status();
                details.push(format!("Subscription API response status: {}", status));

                if status.is_success() {
                    match resp.text().await {
                        Ok(body) => {
                            details.push(format!(
                                "Subscription response body length: {} bytes",
                                body.len()
                            ));

                            // Parse JSON response
                            if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&body)
                            {
                                // Extract email from customer info
                                if let Some(customer) = json_data.get("customer") {
                                    if let Some(email) = customer.get("email") {
                                        if let Some(email_str) = email.as_str() {
                                            account_info.email = Some(email_str.to_string());
                                        }
                                    }
                                }

                                // Extract subscription type
                                if let Some(membership_type) = json_data.get("membershipType") {
                                    if let Some(membership_str) = membership_type.as_str() {
                                        account_info.subscription_type =
                                            Some(membership_str.to_string());
                                    }
                                }

                                // Extract subscription status
                                if let Some(subscription_status) =
                                    json_data.get("subscriptionStatus")
                                {
                                    if let Some(status_str) = subscription_status.as_str() {
                                        account_info.subscription_status =
                                            Some(status_str.to_string());
                                    }
                                }

                                // Extract trial days remaining
                                if let Some(days_remaining) = json_data.get("daysRemainingOnTrial")
                                {
                                    if let Some(days) = days_remaining.as_i64() {
                                        account_info.trial_days_remaining = Some(days as i32);
                                    }
                                }

                                details
                                    .push("Subscription info retrieved successfully".to_string());
                            } else {
                                details.push("Failed to parse subscription JSON data".to_string());
                            }
                        }
                        Err(e) => {
                            details
                                .push(format!("Failed to read subscription response body: {}", e));
                        }
                    }
                } else {
                    details.push(format!("Subscription API failed with status: {}", status));
                }
            }
            Err(e) => {
                details.push(format!("Subscription API request failed: {}", e));
            }
        }

        let user_info = UserAuthInfo {
            is_authorized: true,
            token_length: clean_token.len(),
            token_valid: true,
            api_status: None,
            error_message: None,
            checksum: None,
            account_info: Some(account_info),
        };

        Ok(AuthCheckResult {
            success: true,
            user_info: Some(user_info),
            message: "Subscription info check completed".to_string(),
            details,
        })
    }

    /// Get basic user info without account details (lightweight check)
    pub async fn get_user_info(token: &str) -> Result<AuthCheckResult> {
        let mut details = Vec::new();
        details.push("Starting user info check...".to_string());

        // Clean and validate token
        let clean_token = match Self::clean_token(token) {
            Ok(token) => {
                details.push(format!(
                    "Token cleaned successfully, length: {} characters",
                    token.len()
                ));
                token
            }
            Err(e) => {
                return Ok(AuthCheckResult {
                    success: false,
                    user_info: None,
                    message: "Invalid token format".to_string(),
                    details: vec![format!("Token validation failed: {}", e)],
                });
            }
        };

        // Generate checksum
        let checksum = match Self::generate_cursor_checksum(&clean_token) {
            Ok(checksum) => {
                details.push("Checksum generated successfully".to_string());
                checksum
            }
            Err(e) => {
                details.push(format!("Failed to generate checksum: {}", e));
                return Ok(AuthCheckResult {
                    success: false,
                    user_info: None,
                    message: "Failed to generate checksum".to_string(),
                    details,
                });
            }
        };

        // Check if token looks like JWT
        let is_authorized = if Self::is_jwt_like(&clean_token) {
            details.push("Token appears to be in JWT format, considering it valid".to_string());
            true
        } else {
            details.push("Token is not in JWT format".to_string());
            false
        };

        // Get account info if authorized
        let account_info = if is_authorized {
            details.push("Fetching account information...".to_string());
            match Self::get_account_info(&clean_token, &checksum, &mut details).await {
                Ok(info) => {
                    details.push("Account information retrieved successfully".to_string());
                    info
                }
                Err(e) => {
                    details.push(format!("Failed to get account info: {}", e));
                    None
                }
            }
        } else {
            None
        };

        let user_info = UserAuthInfo {
            is_authorized,
            token_length: clean_token.len(),
            token_valid: Self::is_jwt_like(&clean_token),
            api_status: None,
            error_message: None,
            checksum: Some(checksum),
            account_info,
        };

        let success = user_info.is_authorized;
        let message = if success {
            "User info check completed successfully".to_string()
        } else {
            "User info check failed".to_string()
        };

        Ok(AuthCheckResult {
            success,
            user_info: Some(user_info),
            message,
            details,
        })
    }

    /// Check user authorization with the given token
    pub async fn check_user_authorized(token: &str) -> Result<AuthCheckResult> {
        let mut details = Vec::new();
        details.push("Starting authorization check...".to_string());

        // Clean and validate token
        let clean_token = match Self::clean_token(token) {
            Ok(token) => {
                details.push(format!(
                    "Token cleaned successfully, length: {} characters",
                    token.len()
                ));
                token
            }
            Err(e) => {
                return Ok(AuthCheckResult {
                    success: false,
                    user_info: None,
                    message: "Invalid token format".to_string(),
                    details: vec![format!("Token validation failed: {}", e)],
                });
            }
        };

        // Generate checksum
        let checksum = match Self::generate_cursor_checksum(&clean_token) {
            Ok(checksum) => {
                details.push("Checksum generated successfully".to_string());
                checksum
            }
            Err(e) => {
                details.push(format!("Failed to generate checksum: {}", e));
                return Ok(AuthCheckResult {
                    success: false,
                    user_info: None,
                    message: "Failed to generate checksum".to_string(),
                    details,
                });
            }
        };

        // Create HTTP client
        let client = reqwest::Client::new();

        // Create request headers
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("accept-encoding", "gzip".parse()?);
        headers.insert("authorization", format!("Bearer {}", clean_token).parse()?);
        headers.insert("connect-protocol-version", "1".parse()?);
        headers.insert("content-type", "application/proto".parse()?);
        headers.insert("user-agent", "connect-es/1.6.1".parse()?);
        headers.insert("x-cursor-checksum", checksum.parse()?);
        headers.insert("x-cursor-client-version", "0.48.7".parse()?);
        headers.insert("x-cursor-timezone", "Asia/Shanghai".parse()?);
        headers.insert("x-ghost-mode", "false".parse()?);
        headers.insert("Host", "api2.cursor.sh".parse()?);

        details.push("Making API request to check usage information...".to_string());

        // Make the API request
        let response = client
            .post(
                "https://api2.cursor.sh/aiserver.v1.DashboardService/GetUsageBasedPremiumRequests",
            )
            .headers(headers)
            .body(vec![]) // Empty body
            .timeout(std::time::Duration::from_secs(40))
            .send()
            .await;

        let user_info = match response {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                details.push(format!("API response status: {}", status_code));

                let is_authorized = match status_code {
                    200 => {
                        details.push("User is authorized (200 OK)".to_string());
                        true
                    }
                    401 | 403 => {
                        details.push("User is unauthorized (401/403)".to_string());
                        false
                    }
                    _ => {
                        details.push(format!("Unexpected status code: {}", status_code));
                        // If token looks like JWT, consider it potentially valid
                        if Self::is_jwt_like(&clean_token) {
                            details.push("Token appears to be in JWT format, considering it potentially valid".to_string());
                            true
                        } else {
                            false
                        }
                    }
                };

                // Get account info if authorized
                let account_info = if is_authorized {
                    details.push("Fetching account information...".to_string());
                    match Self::get_account_info(&clean_token, &checksum, &mut details).await {
                        Ok(info) => {
                            details.push("Account information retrieved successfully".to_string());
                            info
                        }
                        Err(e) => {
                            details.push(format!("Failed to get account info: {}", e));
                            None
                        }
                    }
                } else {
                    None
                };

                UserAuthInfo {
                    is_authorized,
                    token_length: clean_token.len(),
                    token_valid: Self::is_jwt_like(&clean_token),
                    api_status: Some(status_code),
                    error_message: None,
                    checksum: Some(checksum),
                    account_info,
                }
            }
            Err(e) => {
                details.push(format!("API request failed: {}", e));

                // If token looks like JWT, consider it potentially valid even if API fails
                let is_authorized = if Self::is_jwt_like(&clean_token) {
                    details.push("Token appears to be in JWT format, considering it potentially valid despite API failure".to_string());
                    true
                } else {
                    false
                };

                UserAuthInfo {
                    is_authorized,
                    token_length: clean_token.len(),
                    token_valid: Self::is_jwt_like(&clean_token),
                    api_status: None,
                    error_message: Some(e.to_string()),
                    checksum: Some(checksum),
                    account_info: None,
                }
            }
        };

        let success = user_info.is_authorized;
        let message = if success {
            "User authorization check completed successfully".to_string()
        } else {
            "User authorization check failed".to_string()
        };

        Ok(AuthCheckResult {
            success,
            user_info: Some(user_info),
            message,
            details,
        })
    }
}
