use crate::{log_debug, log_error, log_info, log_warn};
use anyhow::{Context, Result};
use chrono::Local;
use dirs;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MachineIds {
    #[serde(rename = "telemetry.devDeviceId")]
    pub dev_device_id: String,
    #[serde(rename = "telemetry.macMachineId")]
    pub mac_machine_id: String,
    #[serde(rename = "telemetry.machineId")]
    pub machine_id: String,
    #[serde(rename = "telemetry.sqmId")]
    pub sqm_id: String,
    #[serde(rename = "storage.serviceMachineId")]
    pub service_machine_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub path: String,
    pub filename: String,
    pub timestamp: String,
    pub size: u64,
    pub date_formatted: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<String>,
    pub new_ids: Option<MachineIds>,
}

pub struct MachineIdRestorer {
    pub db_path: PathBuf,
    pub sqlite_path: PathBuf,
}

// 无感换号完整状态结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeamlessSwitchStatus {
    pub workbench_modified: bool,
    pub extension_host_modified: bool,
    pub fully_enabled: bool,
    pub need_reset_warning: bool,
}

impl MachineIdRestorer {
    pub fn new() -> Result<Self> {
        let (db_path, sqlite_path) = Self::get_cursor_paths()?;

        Ok(Self { db_path, sqlite_path })
    }

    // 日志记录方法
    pub fn log_info(&self, message: &str) {
        self.write_log("INFO", message);
    }

    pub fn log_warning(&self, message: &str) {
        self.write_log("WARN", message);
    }

    pub fn log_error(&self, message: &str) {
        self.write_log("ERROR", message);
    }

    pub fn log_debug(&self, message: &str) {
        self.write_log("DEBUG", message);
    }

    fn write_log(&self, level: &str, message: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [{}] {}\n", timestamp, level, message);

        log_info!("{}", log_entry.trim());
    }

    pub fn log_system_info(&self) {
        self.log_info("=== 系统信息 ===");
        self.log_info(&format!("操作系统: {}", std::env::consts::OS));
        self.log_info(&format!("架构: {}", std::env::consts::ARCH));
        self.log_info(&format!(
            "工作目录: {:?}",
            std::env::current_dir().unwrap_or_default()
        ));
        self.log_info(&format!("存储文件路径: {:?}", self.db_path));
        self.log_info(&format!("SQLite路径: {:?}", self.sqlite_path));

        // 检查文件是否存在
        self.log_info(&format!("存储文件是否存在: {}", self.db_path.exists()));
        self.log_info(&format!(
            "SQLite文件是否存在: {}",
            self.sqlite_path.exists()
        ));

        // 获取当前用户
        if let Ok(username) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            self.log_info(&format!("当前用户: {}", username));
        }

        self.log_info("=== 系统信息结束 ===");
    }

    // 测试日志记录功能
    pub fn test_logging(&self) -> Result<String> {
        self.log_info("=== 日志记录功能测试开始 ===");
        self.log_debug("这是一条调试信息");
        self.log_warning("这是一条警告信息");
        self.log_error("这是一条错误信息（测试用）");
        self.log_info("=== 日志记录功能测试完成 ===");

        Ok("日志记录测试完成（已写入应用日志）".to_string())
    }

    // 调试Windows Cursor路径
    pub fn debug_windows_cursor_paths(&self) -> Result<Vec<String>> {
        let mut debug_info = Vec::new();

        self.log_info("=== Windows Cursor路径调试开始 ===");
        debug_info.push("=== Windows Cursor路径调试开始 ===".to_string());

        #[cfg(target_os = "windows")]
        {
            let localappdata =
                std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "LOCALAPPDATA未设置".to_string());

            let info = format!("LOCALAPPDATA: {}", localappdata);
            self.log_info(&info);
            debug_info.push(info);

            // 检查所有可能的路径
            let possible_cursor_paths = vec![
                format!("{}\\Programs\\Cursor\\resources\\app", localappdata),
                format!("{}\\Programs\\cursor\\resources\\app", localappdata),
                format!("{}\\Cursor\\resources\\app", localappdata),
                "C:\\Program Files\\Cursor\\resources\\app".to_string(),
                "C:\\Program Files (x86)\\Cursor\\resources\\app".to_string(),
                format!(
                    "{}\\AppData\\Local\\Programs\\Cursor\\resources\\app",
                    dirs::home_dir().unwrap_or_default().to_string_lossy()
                ),
                "C:\\Cursor\\resources\\app".to_string(),
            ];

            for (i, path) in possible_cursor_paths.iter().enumerate() {
                let path_buf = PathBuf::from(path);
                let package_json = path_buf.join("package.json");
                let main_js = path_buf.join("out").join("main.js");
                let workbench_js = path_buf
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js");

                let path_info = format!(
                    "路径{}: {}\n  - 目录存在: {}\n  - package.json: {}\n  - main.js: {}\n  - workbench.js: {}",
                    i + 1,
                    path,
                    path_buf.exists(),
                    package_json.exists(),
                    main_js.exists(),
                    workbench_js.exists()
                );

                self.log_info(&path_info);
                debug_info.push(path_info);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let info = "此功能仅在Windows上可用".to_string();
            self.log_warning(&info);
            debug_info.push(info);
        }

        self.log_info("=== Windows Cursor路径调试结束 ===");
        debug_info.push("=== Windows Cursor路径调试结束 ===".to_string());

        Ok(debug_info)
    }

    // 设置自定义 Cursor 路径
    pub fn set_custom_cursor_path(&self, path: &str) -> Result<String> {
        let custom_path = PathBuf::from(path);

        // 验证路径是否有效
        let package_json = custom_path.join("package.json");
        let main_js = custom_path.join("out").join("main.js");
        let workbench_js = custom_path
            .join("out")
            .join("vs")
            .join("workbench")
            .join("workbench.desktop.main.js");

        let validation_info = format!(
            "路径验证结果:\n- 目录存在: {}\n- package.json: {}\n- main.js: {}\n- workbench.js: {}",
            custom_path.exists(),
            package_json.exists(),
            main_js.exists(),
            workbench_js.exists()
        );

        self.log_info(&format!("设置自定义Cursor路径: {}", path));
        self.log_info(&validation_info);

        // 保存自定义路径到配置文件
        let config_file = self.get_custom_path_config_file()?;
        fs::write(&config_file, path)?;

        self.log_info("自定义Cursor路径已保存");

        Ok(validation_info)
    }

    // 获取自定义 Cursor 路径
    pub fn get_custom_cursor_path(&self) -> Option<String> {
        match self.get_custom_path_config_file() {
            Ok(config_file) => {
                if config_file.exists() {
                    match fs::read_to_string(&config_file) {
                        Ok(path) => {
                            let path = path.trim();
                            if !path.is_empty() {
                                self.log_info(&format!("读取到自定义Cursor路径: {}", path));
                                return Some(path.to_string());
                            }
                        }
                        Err(e) => {
                            self.log_warning(&format!("读取自定义路径配置失败: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                self.log_warning(&format!("获取自定义路径配置文件路径失败: {}", e));
            }
        }
        None
    }

    // 清除自定义 Cursor 路径
    pub fn clear_custom_cursor_path(&self) -> Result<String> {
        let config_file = self.get_custom_path_config_file()?;

        if config_file.exists() {
            fs::remove_file(&config_file)?;
            self.log_info("自定义Cursor路径已清除");
            Ok("自定义Cursor路径已清除".to_string())
        } else {
            self.log_info("没有设置自定义Cursor路径");
            Ok("没有设置自定义Cursor路径".to_string())
        }
    }

    // 获取 Cursor 版本信息
    pub fn get_cursor_version(&self) -> Result<String> {
        // 首先尝试从自定义路径获取
        if let Some(custom_path) = self.get_custom_cursor_path() {
            let custom_path_buf = PathBuf::from(&custom_path);
            if let Ok(version) = self.read_version_from_path(&custom_path_buf) {
                self.log_info(&format!("从自定义路径获取到Cursor版本: {}", version));
                return Ok(version);
            }
        }

        // 如果自定义路径没有找到，尝试默认路径
        match Self::get_cursor_app_paths() {
            Ok((package_json, _)) => {
                let app_path = package_json
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("无法获取应用目录"))?;

                if let Ok(version) = self.read_version_from_path(app_path) {
                    self.log_info(&format!("从默认路径获取到Cursor版本: {}", version));
                    return Ok(version);
                }
            }
            Err(e) => {
                self.log_warning(&format!("获取默认Cursor路径失败: {}", e));
            }
        }

        Err(anyhow::anyhow!("无法获取Cursor版本信息"))
    }

    // 从指定路径读取版本信息
    fn read_version_from_path(&self, app_path: &Path) -> Result<String> {
        let package_json_path = app_path.join("package.json");

        if !package_json_path.exists() {
            return Err(anyhow::anyhow!(
                "package.json 文件不存在: {:?}",
                package_json_path
            ));
        }

        let content =
            fs::read_to_string(&package_json_path).context("读取 package.json 文件失败")?;

        let json: serde_json::Value =
            serde_json::from_str(&content).context("解析 package.json 文件失败")?;

        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("package.json 中未找到版本信息"))?;

        Ok(version.to_string())
    }

    // 获取自定义路径配置文件路径
    fn get_custom_path_config_file(&self) -> Result<PathBuf> {
        let exe_dir = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not get exe directory"))?
            .to_path_buf();

        Ok(exe_dir.join("custom_cursor_path.txt"))
    }

    // 获取自定义浏览器路径配置文件路径
    fn get_custom_browser_path_config_file(&self) -> Result<PathBuf> {
        let exe_dir = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not get exe directory"))?
            .to_path_buf();

        Ok(exe_dir.join("custom_browser_path.txt"))
    }

    // 设置自定义浏览器路径
    pub fn set_custom_browser_path(&self, path: &str) -> Result<String> {
        let custom_path = PathBuf::from(path);

        // 验证路径是否有效（检查是否是可执行文件）
        let validation_info = format!(
            "浏览器路径验证结果:\n- 文件存在: {}\n- 是否为文件: {}",
            custom_path.exists(),
            custom_path.is_file()
        );

        self.log_info(&format!("设置自定义浏览器路径: {}", path));
        self.log_info(&validation_info);

        // 保存自定义路径到配置文件
        let config_file = self.get_custom_browser_path_config_file()?;
        fs::write(&config_file, path)?;

        self.log_info("自定义浏览器路径已保存");

        Ok(validation_info)
    }

    // 获取自定义浏览器路径
    pub fn get_custom_browser_path(&self) -> Option<String> {
        match self.get_custom_browser_path_config_file() {
            Ok(config_file) => {
                if config_file.exists() {
                    match fs::read_to_string(&config_file) {
                        Ok(path) => {
                            let path = path.trim();
                            if !path.is_empty() {
                                self.log_info(&format!("读取到自定义浏览器路径: {}", path));
                                return Some(path.to_string());
                            }
                        }
                        Err(e) => {
                            self.log_warning(&format!("读取自定义浏览器路径配置失败: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                self.log_warning(&format!("获取自定义浏览器路径配置文件路径失败: {}", e));
            }
        }
        None
    }

    // 清除自定义浏览器路径
    pub fn clear_custom_browser_path(&self) -> Result<String> {
        let config_file = self.get_custom_browser_path_config_file()?;

        if config_file.exists() {
            fs::remove_file(&config_file)?;
            self.log_info("自定义浏览器路径已清除");
            Ok("自定义浏览器路径已清除".to_string())
        } else {
            self.log_info("没有设置自定义浏览器路径");
            Ok("没有设置自定义浏览器路径".to_string())
        }
    }

    #[cfg(target_os = "windows")]
    fn get_cursor_paths() -> Result<(PathBuf, PathBuf)> {
        let appdata = std::env::var("APPDATA").context("APPDATA environment variable not set")?;

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
        let home = dirs::home_dir().context("Could not find home directory")?;

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
        let home = dirs::home_dir().context("Could not find home directory")?;

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

    pub fn find_backups(&self) -> Result<Vec<BackupInfo>> {
        let db_dir = self
            .db_path
            .parent()
            .context("Could not get parent directory")?;
        let db_name = self
            .db_path
            .file_name()
            .context("Could not get filename")?
            .to_string_lossy();

        let mut backups = Vec::new();

        // Read directory and filter backup files
        if let Ok(entries) = fs::read_dir(db_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();

                        // Check if this is a backup file
                        // Support multiple backup formats: .bak.timestamp, .backup.timestamp, .restore_bak.timestamp
                        let is_backup = filename_str.starts_with(&*db_name)
                            && (filename_str.contains(".bak.")
                                || filename_str.contains(".backup.")
                                || filename_str.contains(".restore_bak."));

                        if is_backup {
                            if let Ok(metadata) = fs::metadata(&path) {
                                // Extract timestamp from filename
                                let timestamp_str =
                                    if let Some(bak_pos) = filename_str.find(".bak.") {
                                        &filename_str[bak_pos + 5..]
                                    } else if let Some(backup_pos) = filename_str.find(".backup.") {
                                        &filename_str[backup_pos + 8..]
                                    } else if let Some(restore_bak_pos) =
                                        filename_str.find(".restore_bak.")
                                    {
                                        &filename_str[restore_bak_pos + 12..]
                                    } else {
                                        "unknown"
                                    };

                                let date_formatted = Self::format_timestamp(timestamp_str);

                                backups.push(BackupInfo {
                                    path: path.to_string_lossy().to_string(),
                                    filename: filename_str.to_string(),
                                    timestamp: timestamp_str.to_string(),
                                    size: metadata.len(),
                                    date_formatted,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    fn format_timestamp(timestamp_str: &str) -> String {
        if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y%m%d_%H%M%S")
        {
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            "Unknown date".to_string()
        }
    }

    pub fn extract_ids_from_backup(&self, backup_path: &str) -> Result<MachineIds> {
        let content = fs::read_to_string(backup_path).context("Failed to read backup file")?;

        let data: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse backup JSON")?;

        let dev_device_id = data
            .get("telemetry.devDeviceId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mac_machine_id = data
            .get("telemetry.macMachineId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let machine_id = data
            .get("telemetry.machineId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let sqm_id = data
            .get("telemetry.sqmId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let service_machine_id = data
            .get("storage.serviceMachineId")
            .and_then(|v| v.as_str())
            .unwrap_or(&dev_device_id)
            .to_string();

        Ok(MachineIds {
            dev_device_id,
            mac_machine_id,
            machine_id,
            sqm_id,
            service_machine_id,
        })
    }

    pub fn create_backup(&self) -> Result<String> {
        if !self.db_path.exists() {
            return Err(anyhow::anyhow!("Current storage.json file not found"));
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = format!("{}.bak.{}", self.db_path.to_string_lossy(), timestamp);

        fs::copy(&self.db_path, &backup_path).context("Failed to create backup")?;

        Ok(backup_path)
    }

    pub fn update_storage_file(&self, ids: &MachineIds) -> Result<()> {
        if !self.db_path.exists() {
            return Err(anyhow::anyhow!("Current storage.json file not found"));
        }

        // Read current file
        let content =
            fs::read_to_string(&self.db_path).context("Failed to read current storage file")?;

        let mut data: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse current storage JSON")?;

        // Update IDs
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "telemetry.devDeviceId".to_string(),
                serde_json::Value::String(ids.dev_device_id.clone()),
            );
            obj.insert(
                "telemetry.macMachineId".to_string(),
                serde_json::Value::String(ids.mac_machine_id.clone()),
            );
            obj.insert(
                "telemetry.machineId".to_string(),
                serde_json::Value::String(ids.machine_id.clone()),
            );
            obj.insert(
                "telemetry.sqmId".to_string(),
                serde_json::Value::String(ids.sqm_id.clone()),
            );
            obj.insert(
                "storage.serviceMachineId".to_string(),
                serde_json::Value::String(ids.service_machine_id.clone()),
            );
        }

        // Write updated file
        let updated_content =
            serde_json::to_string_pretty(&data).context("Failed to serialize updated data")?;

        fs::write(&self.db_path, updated_content)
            .context("Failed to write updated storage file")?;

        Ok(())
    }

    pub fn update_sqlite_db(&self, _ids: &MachineIds) -> Result<Vec<String>> {
        // SQLite functionality removed for simplicity
        // Return a note that this feature is not implemented
        Ok(vec![
            "SQLite database update skipped (feature not implemented)".to_string(),
        ])
    }

    pub fn get_machine_id_path() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let appdata =
                std::env::var("APPDATA").context("APPDATA environment variable not set")?;
            Ok(PathBuf::from(appdata).join("Cursor").join("machineId"))
        }

        #[cfg(target_os = "macos")]
        {
            let home = dirs::home_dir().context("Could not find home directory")?;
            Ok(home
                .join("Library")
                .join("Application Support")
                .join("Cursor")
                .join("machineId"))
        }

        #[cfg(target_os = "linux")]
        {
            let home = dirs::home_dir().context("Could not find home directory")?;
            Ok(home.join(".config").join("Cursor").join("machineId"))
        }
    }

    pub fn update_machine_id_file(&self, dev_device_id: &str) -> Result<()> {
        let machine_id_path = Self::get_machine_id_path()?;

        // Create directory if not exists
        if let Some(parent) = machine_id_path.parent() {
            fs::create_dir_all(parent).context("Failed to create machine ID directory")?;
        }

        // Backup existing file if it exists
        if machine_id_path.exists() {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            let backup_path = format!("{}.bak.{}", machine_id_path.to_string_lossy(), timestamp);
            let _ = fs::copy(&machine_id_path, backup_path);
        }

        // Write new ID
        fs::write(&machine_id_path, dev_device_id).context("Failed to write machine ID file")?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub fn update_system_ids(&self, ids: &MachineIds) -> Result<Vec<String>> {
        use winreg::RegKey;
        use winreg::enums::*;

        let mut results = Vec::new();

        // Update MachineGuid
        if !ids.dev_device_id.is_empty() {
            match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
                "SOFTWARE\\Microsoft\\Cryptography",
                KEY_WRITE | KEY_WOW64_64KEY,
            ) {
                Ok(key) => {
                    if key.set_value("MachineGuid", &ids.dev_device_id).is_ok() {
                        results.push("Windows MachineGuid updated successfully".to_string());
                    } else {
                        results.push("Failed to update Windows MachineGuid".to_string());
                    }
                }
                Err(_) => {
                    results.push("Permission denied: Cannot update Windows MachineGuid".to_string())
                }
            }
        }

        // Update SQMClient MachineId
        if !ids.sqm_id.is_empty() {
            match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
                "SOFTWARE\\Microsoft\\SQMClient",
                KEY_WRITE | KEY_WOW64_64KEY,
            ) {
                Ok(key) => {
                    if key.set_value("MachineId", &ids.sqm_id).is_ok() {
                        results.push("Windows SQM MachineId updated successfully".to_string());
                    } else {
                        results.push("Failed to update Windows SQM MachineId".to_string());
                    }
                }
                Err(_) => results
                    .push("SQMClient registry key not found or permission denied".to_string()),
            }
        }

        Ok(results)
    }

    #[cfg(target_os = "macos")]
    pub fn update_system_ids(&self, ids: &MachineIds) -> Result<Vec<String>> {
        let mut results = Vec::new();

        if !ids.mac_machine_id.is_empty() {
            let uuid_file =
                "/var/root/Library/Preferences/SystemConfiguration/com.apple.platform.uuid.plist";

            if Path::new(uuid_file).exists() {
                let cmd = format!(
                    "sudo plutil -replace \"UUID\" -string \"{}\" \"{}\"",
                    ids.mac_machine_id, uuid_file
                );

                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            results.push("macOS platform UUID updated successfully".to_string());
                        } else {
                            results.push("Failed to execute plutil command".to_string());
                        }
                    }
                    Err(_) => {
                        results.push("Failed to update macOS platform UUID".to_string());
                    }
                }
            } else {
                results.push("macOS platform UUID file not found".to_string());
            }
        }

        Ok(results)
    }

    #[cfg(target_os = "linux")]
    pub fn update_system_ids(&self, _ids: &MachineIds) -> Result<Vec<String>> {
        Ok(vec!["Linux system ID updates not implemented".to_string()])
    }

    pub fn generate_new_machine_ids(&self) -> Result<MachineIds> {
        // Generate new UUID for dev device ID
        let dev_device_id = Uuid::new_v4().to_string();

        // Generate new machineId (64 characters of hexadecimal)
        let mut machine_id_data = [0u8; 32];
        rand::thread_rng().fill(&mut machine_id_data);
        let machine_id = format!("{:x}", Sha256::digest(&machine_id_data));

        // Generate new macMachineId (128 characters of hexadecimal)
        let mut mac_machine_id_data = [0u8; 64];
        rand::thread_rng().fill(&mut mac_machine_id_data);
        let mac_machine_id = format!("{:x}", Sha512::digest(&mac_machine_id_data));

        // Generate new sqmId
        let sqm_id = format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase());

        Ok(MachineIds {
            dev_device_id: dev_device_id.clone(),
            mac_machine_id,
            machine_id,
            sqm_id,
            service_machine_id: dev_device_id, // Same as dev_device_id
        })
    }

    pub fn reset_machine_ids(&self) -> Result<ResetResult> {
        let mut details = Vec::new();
        let mut success = true;

        self.log_info("开始机器ID重置流程...");
        details.push("Starting machine ID reset process...".to_string());

        // 检查存储文件是否存在
        self.log_debug(&format!("检查存储文件: {:?}", self.db_path));
        if !self.db_path.exists() {
            let error_msg = format!("Storage file not found: {}", self.db_path.display());
            self.log_error(&error_msg);
            return Ok(ResetResult {
                success: false,
                message: error_msg,
                details,
                new_ids: None,
            });
        }
        self.log_info("存储文件存在，继续处理");

        // 创建当前状态的备份
        self.log_info("创建备份文件...");
        match self.create_backup() {
            Ok(backup_path) => {
                let backup_msg = format!("Created backup at: {}", backup_path);
                self.log_info(&backup_msg);
                details.push(backup_msg);
            }
            Err(e) => {
                let warning_msg = format!("Warning: Failed to create backup: {}", e);
                self.log_warning(&warning_msg);
                details.push(warning_msg);
            }
        }

        // 生成新的机器ID
        self.log_info("生成新的机器ID...");
        let new_ids = match self.generate_new_machine_ids() {
            Ok(ids) => {
                self.log_info(&format!("生成的新ID: dev_device_id={}, machine_id长度={}, mac_machine_id长度={}, sqm_id={}", 
                    ids.dev_device_id, ids.machine_id.len(), ids.mac_machine_id.len(), ids.sqm_id));
                details.push("Generated new machine IDs".to_string());
                ids
            }
            Err(e) => {
                let error_msg = format!("Failed to generate new IDs: {}", e);
                self.log_error(&error_msg);
                return Ok(ResetResult {
                    success: false,
                    message: error_msg,
                    details,
                    new_ids: None,
                });
            }
        };

        // 更新存储文件
        self.log_info("更新存储文件...");
        if let Err(e) = self.update_storage_file(&new_ids) {
            success = false;
            let error_msg = format!("Failed to update storage file: {}", e);
            self.log_error(&error_msg);
            details.push(error_msg);
        } else {
            let success_msg = "Successfully updated storage.json".to_string();
            self.log_info(&success_msg);
            details.push(success_msg);
        }

        // 更新SQLite数据库
        self.log_info("更新SQLite数据库...");
        match self.update_sqlite_db(&new_ids) {
            Ok(sqlite_results) => {
                for result in &sqlite_results {
                    self.log_debug(&format!("SQLite更新结果: {}", result));
                }
                details.extend(sqlite_results);
            }
            Err(e) => {
                let warning_msg = format!("Warning: Failed to update SQLite database: {}", e);
                self.log_warning(&warning_msg);
                details.push(warning_msg);
            }
        }

        // 更新机器ID文件
        self.log_info("更新机器ID文件...");
        if let Err(e) = self.update_machine_id_file(&new_ids.dev_device_id) {
            let warning_msg = format!("Warning: Failed to update machine ID file: {}", e);
            self.log_warning(&warning_msg);
            details.push(warning_msg);
        } else {
            let success_msg = "Successfully updated machine ID file".to_string();
            self.log_info(&success_msg);
            details.push(success_msg);
        }

        // 更新系统ID
        self.log_info("更新系统ID...");
        match self.update_system_ids(&new_ids) {
            Ok(system_results) => {
                for result in &system_results {
                    self.log_debug(&format!("系统ID更新结果: {}", result));
                }
                details.extend(system_results);
            }
            Err(e) => {
                let warning_msg = format!("Warning: Failed to update system IDs: {}", e);
                self.log_warning(&warning_msg);
                details.push(warning_msg);
            }
        }

        let message = if success {
            "Machine IDs reset successfully".to_string()
        } else {
            "Machine ID reset completed with some errors".to_string()
        };

        self.log_info(&format!("机器ID重置完成: {}", message));

        Ok(ResetResult {
            success,
            message,
            details,
            new_ids: Some(new_ids),
        })
    }

    pub fn get_cursor_app_paths() -> Result<(PathBuf, PathBuf)> {
        // 首先检查是否有自定义路径
        if let Ok(restorer) = MachineIdRestorer::new() {
            if let Some(custom_path) = restorer.get_custom_cursor_path() {
                let custom_path_buf = PathBuf::from(&custom_path);
                let package_json = custom_path_buf.join("package.json");
                let main_js = custom_path_buf.join("out").join("main.js");

                log_info!("🎯 [DEBUG] 使用自定义路径: {:?}", custom_path_buf);
                log_info!(
                    "🎯 [DEBUG] 自定义路径验证 - package.json存在: {}, main.js存在: {}",
                    package_json.exists(),
                    main_js.exists()
                );

                if package_json.exists() && main_js.exists() {
                    log_info!("✅ [DEBUG] 自定义路径有效，使用自定义路径");
                    return Ok((package_json, main_js));
                } else {
                    log_error!("❌ [DEBUG] 自定义路径无效，继续使用自动搜索");
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let localappdata = std::env::var("LOCALAPPDATA")
                .context("LOCALAPPDATA environment variable not set")?;

            // Windows上Cursor可能的安装路径
            let possible_cursor_paths = vec![
                // 方式1: LOCALAPPDATA路径 (用户安装)
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("Cursor")
                    .join("resources")
                    .join("app"),
                // 方式2: LOCALAPPDATA路径的替代结构
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("cursor")
                    .join("resources")
                    .join("app"),
                // 方式3: 直接在Cursor目录下
                PathBuf::from(&localappdata)
                    .join("Cursor")
                    .join("resources")
                    .join("app"),
                // 方式4: 系统Program Files路径 (管理员安装)
                PathBuf::from("C:\\Program Files\\Cursor\\resources\\app"),
                PathBuf::from("C:\\Program Files (x86)\\Cursor\\resources\\app"),
                // 方式5: 用户程序目录
                dirs::home_dir()
                    .unwrap_or_default()
                    .join("AppData\\Local\\Programs\\Cursor\\resources\\app"),
                // 方式6: 便携版路径
                PathBuf::from("C:\\Cursor\\resources\\app"),
            ];

            // 搜索存在的路径
            for (i, cursor_path) in possible_cursor_paths.iter().enumerate() {
                let package_json = cursor_path.join("package.json");
                let main_js = cursor_path.join("out").join("main.js");

                log_debug!("🔍 [DEBUG] Windows路径搜索 {}: {:?}", i + 1, cursor_path);
                log_info!(
                    "🔍 [DEBUG] package.json: {:?}, 存在: {}",
                    package_json,
                    package_json.exists()
                );
                log_info!(
                    "🔍 [DEBUG] main.js: {:?}, 存在: {}",
                    main_js,
                    main_js.exists()
                );

                if package_json.exists() && main_js.exists() {
                    log_info!(
                        "✅ [DEBUG] 找到有效的Windows Cursor安装路径: {:?}",
                        cursor_path
                    );
                    return Ok((package_json, main_js));
                }
            }

            // 如果都找不到，返回最可能的路径用于错误提示
            let default_path = PathBuf::from(&localappdata)
                .join("Programs")
                .join("Cursor")
                .join("resources")
                .join("app");
            let package_json = default_path.join("package.json");
            let main_js = default_path.join("out").join("main.js");

            Ok((package_json, main_js))
        }

        #[cfg(target_os = "macos")]
        {
            let cursor_path = PathBuf::from("/Applications/Cursor.app/Contents/Resources/app");

            let package_json = cursor_path.join("package.json");
            let main_js = cursor_path.join("out").join("main.js");

            Ok((package_json, main_js))
        }

        #[cfg(target_os = "linux")]
        {
            let possible_paths = vec![
                PathBuf::from("/opt/Cursor/resources/app"),
                PathBuf::from("/usr/share/cursor/resources/app"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".local/share/cursor/resources/app"),
                PathBuf::from("/usr/lib/cursor/app"),
            ];

            for cursor_path in possible_paths {
                let package_json = cursor_path.join("package.json");
                let main_js = cursor_path.join("out").join("main.js");

                if package_json.exists() && main_js.exists() {
                    return Ok((package_json, main_js));
                }
            }

            Err(anyhow::anyhow!(
                "Could not find Cursor installation on Linux"
            ))
        }
    }

    // 获取 product.json 文件路径
    pub fn get_product_json_path() -> Result<PathBuf> {
        // 首先检查是否有自定义路径
        if let Ok(restorer) = MachineIdRestorer::new() {
            if let Some(custom_path) = restorer.get_custom_cursor_path() {
                let custom_product_json = PathBuf::from(&custom_path).join("product.json");

                log_info!("🎯 [DEBUG] 使用自定义product.json路径: {:?}", custom_product_json);
                log_info!(
                    "🎯 [DEBUG] 自定义product.json存在: {}",
                    custom_product_json.exists()
                );

                if custom_product_json.exists() {
                    log_info!("✅ [DEBUG] 自定义product.json路径有效");
                    return Ok(custom_product_json);
                } else {
                    log_error!("❌ [DEBUG] 自定义product.json路径无效，继续使用自动搜索");
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let localappdata = std::env::var("LOCALAPPDATA")
                .context("LOCALAPPDATA environment variable not set")?;

            let possible_product_paths = vec![
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("product.json"),
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("cursor")
                    .join("resources")
                    .join("app")
                    .join("product.json"),
                PathBuf::from(&localappdata)
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("product.json"),
                PathBuf::from("C:\\Program Files\\Cursor\\resources\\app\\product.json"),
                PathBuf::from("C:\\Program Files (x86)\\Cursor\\resources\\app\\product.json"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join("AppData\\Local\\Programs\\Cursor\\resources\\app\\product.json"),
                PathBuf::from("C:\\Cursor\\resources\\app\\product.json"),
            ];

            for (i, product_path) in possible_product_paths.iter().enumerate() {
                log_info!(
                    "🔍 [DEBUG] Windows product.json路径搜索 {}: {:?}",
                    i + 1,
                    product_path
                );
                log_debug!("🔍 [DEBUG] product.json存在: {}", product_path.exists());

                if product_path.exists() {
                    log_info!(
                        "✅ [DEBUG] 找到有效的Windows product.json路径: {:?}",
                        product_path
                    );
                    return Ok(product_path.clone());
                }
            }

            let default_path = PathBuf::from(&localappdata)
                .join("Programs")
                .join("Cursor")
                .join("resources")
                .join("app")
                .join("product.json");

            Ok(default_path)
        }

        #[cfg(target_os = "macos")]
        {
            let product_path = PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/product.json");
            Ok(product_path)
        }

        #[cfg(target_os = "linux")]
        {
            let possible_base_paths = vec![
                PathBuf::from("/opt/Cursor/resources/app"),
                PathBuf::from("/usr/share/cursor/resources/app"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".local/share/cursor/resources/app"),
                PathBuf::from("/usr/lib/cursor/app"),
            ];

            for base_path in possible_base_paths {
                let product_path = base_path.join("product.json");

                if product_path.exists() {
                    return Ok(product_path);
                }
            }

            Err(anyhow::anyhow!(
                "Could not find Cursor product.json on Linux"
            ))
        }
    }

    // 修改 product.json 文件，删除 checksums 字段
    pub fn modify_product_json() -> Result<String> {
        let product_path = Self::get_product_json_path()?;

        if !product_path.exists() {
            log_error!("❌ [DEBUG] product.json文件不存在: {:?}", product_path);
            return Err(anyhow::anyhow!("product.json文件不存在: {:?}", product_path));
        }

        log_info!("📖 [DEBUG] 读取product.json文件: {:?}", product_path);

        // 读取文件内容
        let content = fs::read_to_string(&product_path)
            .context("Failed to read product.json file")?;

        let mut data: serde_json::Value = serde_json::from_str(&content)
            .context("Failed to parse product.json file")?;

        // 检查是否有 checksums 字段
        if data.get("checksums").is_some() {
            log_info!("🔍 [DEBUG] 发现checksums字段，准备删除");

            // 创建备份文件
            let backup_path = product_path.with_file_name("product.json.wuqi.back");
            if !backup_path.exists() {
                log_info!("📋 [DEBUG] 创建备份文件: {:?}", backup_path);
                
                // 设置文件为可写
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&product_path)?.permissions();
                    perms.set_mode(0o644);
                    fs::set_permissions(&product_path, perms)?;
                }

                #[cfg(windows)]
                {
                    let mut perms = fs::metadata(&product_path)?.permissions();
                    perms.set_readonly(false);
                    fs::set_permissions(&product_path, perms)?;
                }

                fs::copy(&product_path, &backup_path)
                    .context("Failed to create backup of product.json")?;
                log_info!("✅ [DEBUG] 备份文件创建成功");
            } else {
                log_info!("ℹ️ [DEBUG] 备份文件已存在，跳过备份");
            }

            // 删除 checksums 字段
            if let Some(obj) = data.as_object_mut() {
                obj.remove("checksums");
                log_info!("✅ [DEBUG] 已删除checksums字段");
            }

            // 设置文件为可写
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&product_path)?.permissions();
                perms.set_mode(0o644);
                fs::set_permissions(&product_path, perms)?;
            }

            #[cfg(windows)]
            {
                let mut perms = fs::metadata(&product_path)?.permissions();
                perms.set_readonly(false);
                fs::set_permissions(&product_path, perms)?;
            }

            // 写回文件
            let updated_content = serde_json::to_string_pretty(&data)
                .context("Failed to serialize updated product.json")?;
            fs::write(&product_path, updated_content)
                .context("Failed to write updated product.json")?;

            log_info!("✅ [DEBUG] 已成功更新product.json文件");

            // 设置文件为只读
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&product_path)?.permissions();
                perms.set_mode(0o444);
                fs::set_permissions(&product_path, perms)?;
            }

            #[cfg(windows)]
            {
                let mut perms = fs::metadata(&product_path)?.permissions();
                perms.set_readonly(true);
                fs::set_permissions(&product_path, perms)?;
            }

            log_info!("🔒 [DEBUG] 文件已设置为只读");

            Ok(format!("已成功删除product.json中的checksums字段: {:?}", product_path))
        } else {
            log_info!("ℹ️ [DEBUG] product.json中没有checksums字段，无需处理");
            Ok("product.json中没有checksums字段，无需处理".to_string())
        }
    }

    pub fn get_workbench_js_path() -> Result<PathBuf> {
        // 首先检查是否有自定义路径
        if let Ok(restorer) = MachineIdRestorer::new() {
            if let Some(custom_path) = restorer.get_custom_cursor_path() {
                let custom_workbench = PathBuf::from(&custom_path)
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js");

                log_info!("🎯 [DEBUG] 使用自定义workbench路径: {:?}", custom_workbench);
                log_info!(
                    "🎯 [DEBUG] 自定义workbench存在: {}",
                    custom_workbench.exists()
                );

                if custom_workbench.exists() {
                    log_info!("✅ [DEBUG] 自定义workbench路径有效");
                    return Ok(custom_workbench);
                } else {
                    log_error!("❌ [DEBUG] 自定义workbench路径无效，继续使用自动搜索");
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let localappdata = std::env::var("LOCALAPPDATA")
                .context("LOCALAPPDATA environment variable not set")?;

            // Windows上Cursor workbench可能的路径
            let possible_workbench_paths = vec![
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js"),
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js"),
                PathBuf::from(&localappdata)
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js"),
                PathBuf::from("C:\\Program Files\\Cursor\\resources\\app\\out\\vs\\workbench\\workbench.desktop.main.js"),
                PathBuf::from("C:\\Program Files (x86)\\Cursor\\resources\\app\\out\\vs\\workbench\\workbench.desktop.main.js"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join("AppData\\Local\\Programs\\Cursor\\resources\\app\\out\\vs\\workbench\\workbench.desktop.main.js"),
                PathBuf::from("C:\\Cursor\\resources\\app\\out\\vs\\workbench\\workbench.desktop.main.js"),
            ];

            for (i, workbench_path) in possible_workbench_paths.iter().enumerate() {
                log_info!(
                    "🔍 [DEBUG] Windows workbench路径搜索 {}: {:?}",
                    i + 1,
                    workbench_path
                );
                log_debug!("🔍 [DEBUG] workbench存在: {}", workbench_path.exists());

                if workbench_path.exists() {
                    log_info!(
                        "✅ [DEBUG] 找到有效的Windows workbench路径: {:?}",
                        workbench_path
                    );
                    return Ok(workbench_path.clone());
                }
            }

            let default_path = PathBuf::from(&localappdata)
                .join("Programs")
                .join("Cursor")
                .join("resources")
                .join("app")
                .join("out")
                .join("vs")
                .join("workbench")
                .join("workbench.desktop.main.js");

            Ok(default_path)
        }

        #[cfg(target_os = "macos")]
        {
            let workbench_path = PathBuf::from("/Applications/Cursor.app/Contents/Resources/app")
                .join("out")
                .join("vs")
                .join("workbench")
                .join("workbench.desktop.main.js");

            Ok(workbench_path)
        }

        #[cfg(target_os = "linux")]
        {
            let possible_base_paths = vec![
                PathBuf::from("/opt/Cursor/resources/app"),
                PathBuf::from("/usr/share/cursor/resources/app"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".local/share/cursor/resources/app"),
                PathBuf::from("/usr/lib/cursor/app"),
            ];

            for base_path in possible_base_paths {
                let workbench_path = base_path
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("workbench.desktop.main.js");

                if workbench_path.exists() {
                    return Ok(workbench_path);
                }
            }

            Err(anyhow::anyhow!(
                "Could not find Cursor workbench.desktop.main.js on Linux"
            ))
        }
    }

    pub fn get_extension_host_process_js_path() -> Result<PathBuf> {
        // 首先检查是否有自定义路径
        if let Ok(restorer) = MachineIdRestorer::new() {
            if let Some(custom_path) = restorer.get_custom_cursor_path() {
                let custom_extension = PathBuf::from(&custom_path)
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("api")
                    .join("node")
                    .join("extensionHostProcess.js");

                log_info!("🎯 [DEBUG] 使用自定义extensionHostProcess路径: {:?}", custom_extension);
                log_info!(
                    "🎯 [DEBUG] 自定义extensionHostProcess存在: {}",
                    custom_extension.exists()
                );

                if custom_extension.exists() {
                    log_info!("✅ [DEBUG] 自定义extensionHostProcess路径有效");
                    return Ok(custom_extension);
                } else {
                    log_error!("❌ [DEBUG] 自定义extensionHostProcess路径无效，继续使用自动搜索");
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let localappdata = std::env::var("LOCALAPPDATA")
                .context("LOCALAPPDATA environment variable not set")?;

            // Windows上Cursor extensionHostProcess可能的路径
            let possible_extension_paths = vec![
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("api")
                    .join("node")
                    .join("extensionHostProcess.js"),
                PathBuf::from(&localappdata)
                    .join("Programs")
                    .join("cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("api")
                    .join("node")
                    .join("extensionHostProcess.js"),
                PathBuf::from(&localappdata)
                    .join("Cursor")
                    .join("resources")
                    .join("app")
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("api")
                    .join("node")
                    .join("extensionHostProcess.js"),
                PathBuf::from("C:\\Program Files\\Cursor\\resources\\app\\out\\vs\\workbench\\api\\node\\extensionHostProcess.js"),
                PathBuf::from("C:\\Program Files (x86)\\Cursor\\resources\\app\\out\\vs\\workbench\\api\\node\\extensionHostProcess.js"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join("AppData\\Local\\Programs\\Cursor\\resources\\app\\out\\vs\\workbench\\api\\node\\extensionHostProcess.js"),
                PathBuf::from("C:\\Cursor\\resources\\app\\out\\vs\\workbench\\api\\node\\extensionHostProcess.js"),
            ];

            for (i, extension_path) in possible_extension_paths.iter().enumerate() {
                log_info!(
                    "🔍 [DEBUG] Windows extensionHostProcess路径搜索 {}: {:?}",
                    i + 1,
                    extension_path
                );
                log_debug!("🔍 [DEBUG] extensionHostProcess存在: {}", extension_path.exists());

                if extension_path.exists() {
                    log_info!(
                        "✅ [DEBUG] 找到有效的Windows extensionHostProcess路径: {:?}",
                        extension_path
                    );
                    return Ok(extension_path.clone());
                }
            }

            let default_path = PathBuf::from(&localappdata)
                .join("Programs")
                .join("Cursor")
                .join("resources")
                .join("app")
                .join("out")
                .join("vs")
                .join("workbench")
                .join("api")
                .join("node")
                .join("extensionHostProcess.js");

            Ok(default_path)
        }

        #[cfg(target_os = "macos")]
        {
            let extension_path = PathBuf::from("/Applications/Cursor.app/Contents/Resources/app")
                .join("out")
                .join("vs")
                .join("workbench")
                .join("api")
                .join("node")
                .join("extensionHostProcess.js");

            Ok(extension_path)
        }

        #[cfg(target_os = "linux")]
        {
            let possible_base_paths = vec![
                PathBuf::from("/opt/Cursor/resources/app"),
                PathBuf::from("/usr/share/cursor/resources/app"),
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".local/share/cursor/resources/app"),
                PathBuf::from("/usr/lib/cursor/app"),
            ];

            for base_path in possible_base_paths {
                let extension_path = base_path
                    .join("out")
                    .join("vs")
                    .join("workbench")
                    .join("api")
                    .join("node")
                    .join("extensionHostProcess.js");
                if extension_path.exists() {
                    return Ok(extension_path);
                }
            }

            let default_path = PathBuf::from("/opt/Cursor/resources/app")
                .join("out")
                .join("vs")
                .join("workbench")
                .join("api")
                .join("node")
                .join("extensionHostProcess.js");

            Ok(default_path)
        }
    }

    // 检查 extensionHostProcess.js 文件是否已经被修改过（无感重置ID功能）
    pub fn check_extension_host_modified() -> Result<bool> {
        let extension_path = Self::get_extension_host_process_js_path()?;

        if !extension_path.exists() {
            log_warn!("⚠️ [DEBUG] extensionHostProcess文件不存在: {:?}", extension_path);
            return Ok(false);
        }

        let content = fs::read_to_string(&extension_path)
            .context("Failed to read extensionHostProcess file")?;

        // 检查是否包含注入标记
        let has_injection = content.contains("SEAMLESS SWITCH EXTENSION INJECTION - START");

        log_info!("🔍 [DEBUG] 检查extensionHostProcess修改状态:");
        log_info!("🔍 [DEBUG] - 注入代码存在: {}", has_injection);
        log_info!("🔍 [DEBUG] - 文件路径: {:?}", extension_path);

        Ok(has_injection)
    }

    // 检查workbench文件是否已经被修改过（无感换号功能）
    pub fn check_seamless_switch_status() -> Result<bool> {
        let workbench_path = Self::get_workbench_js_path()?;

        if !workbench_path.exists() {
            log_error!("❌ [DEBUG] workbench文件不存在: {:?}", workbench_path);
            return Err(anyhow::anyhow!("workbench文件不存在: {:?}", workbench_path));
        }

        let content =
            fs::read_to_string(&workbench_path).context("Failed to read workbench file")?;

        // 检查是否包含四个关键的修改标记
        // 注意：workbench 的参数名会随版本变化（如 (s,r)/(e,t)/($,_) 等），不要写死参数列表
        let has_wuqi_hook = content.contains("this.storeAccessRefreshToken=window.wuqi=(");
        let has_store_hook = content.contains("this.database.getItems()));await (async function hookStore(e){if(e.get(\"releaseNotes/lastVersion\"))window.store=e})(this)");
        let has_error_hook =
            content.contains("static[Symbol.hasInstance](e){window.erroHook&&window.erroHook(e);");
        let has_polling_script = content.contains("// 无感换号轮询功能 - START");

        log_info!("🔍 [DEBUG] 检查无感换号状态:");
        log_info!("🔍 [DEBUG] - wuqi hook存在: {}", has_wuqi_hook);
        log_info!("🔍 [DEBUG] - store hook存在: {}", has_store_hook);
        log_info!("🔍 [DEBUG] - error hook存在: {}", has_error_hook);
        log_info!("🔍 [DEBUG] - 轮询脚本存在: {}", has_polling_script);
        log_info!("🔍 [DEBUG] - 文件路径: {:?}", workbench_path);

        Ok(has_wuqi_hook && has_store_hook && has_error_hook && has_polling_script)
    }
    
    // 获取完整的无感换号状态（包括 extensionHostProcess 修改状态）
    pub fn get_seamless_switch_full_status() -> Result<SeamlessSwitchStatus> {
        let workbench_modified = Self::check_seamless_switch_status().unwrap_or(false);
        let extension_host_modified = Self::check_extension_host_modified().unwrap_or(false);
        
        // 如果 workbench 已修改但 extensionHostProcess 未修改，需要警告
        let need_reset_warning = workbench_modified && !extension_host_modified;
        
        // 只有两者都修改了才算完全启用
        let fully_enabled = workbench_modified && extension_host_modified;
        
        log_info!("🔍 [DEBUG] 无感换号完整状态:");
        log_info!("🔍 [DEBUG] - workbench已修改: {}", workbench_modified);
        log_info!("🔍 [DEBUG] - extensionHost已修改: {}", extension_host_modified);
        log_info!("🔍 [DEBUG] - 完全启用: {}", fully_enabled);
        log_info!("🔍 [DEBUG] - 需要重置警告: {}", need_reset_warning);
        
        Ok(SeamlessSwitchStatus {
            workbench_modified,
            extension_host_modified,
            fully_enabled,
            need_reset_warning,
        })
    }

    // 修改 extensionHostProcess.js 文件
    pub fn modify_extension_host_process() -> Result<()> {
        log_info!("🔧 [DEBUG] 开始修改 extensionHostProcess.js 文件...");
        
        let extension_path = Self::get_extension_host_process_js_path()?;

        if !extension_path.exists() {
            log_error!("❌ [DEBUG] extensionHostProcess文件不存在: {:?}", extension_path);
            return Err(anyhow::anyhow!("extensionHostProcess文件不存在: {:?}", extension_path));
        }

        // 创建备份
        let backup_path = extension_path.with_file_name("extensionHostProcess.js.wuqi.back");
        if !backup_path.exists() {
            log_info!("📋 [DEBUG] 创建extensionHostProcess备份文件: {:?}", backup_path);
            fs::copy(&extension_path, &backup_path).context("Failed to create extensionHostProcess backup")?;
        } else {
            log_info!("ℹ️ [DEBUG] extensionHostProcess备份文件已存在，跳过备份");
        }

        // 从备份文件读取原始内容
        let mut content = fs::read_to_string(&backup_path).context("Failed to read extensionHostProcess backup")?;

        log_info!("📖 [DEBUG] 从extensionHostProcess备份文件读取原始内容");

        // 使用正则表达式替换 header.set("x-cursor-checksum",...) 
        // 模式：header.set("x-cursor-checksum", VAR1===void 0?`${VAR2}${VAR3}`:`${VAR2}${VAR3}/${VAR1}`)
        // 替换为：header.set("x-cursor-checksum", VAR1===void 0?`${VAR2}${global.MachineId||VAR3}`:`${VAR2}${global.MachineId||VAR3}/${global.MacMachineId||VAR1}`)
        
        use regex::Regex;
        
        // 匹配模式：header.set("x-cursor-checksum", 变量名===void 0?`${变量1}${变量2}`:`${变量1}${变量2}/${变量名}`)
        let re = Regex::new(
            r#"header\.set\("x-cursor-checksum",\s*(\w+)\s*===\s*void\s+0\s*\?\s*`\$\{(\w+)\}\$\{(\w+)\}`\s*:\s*`\$\{(\w+)\}\$\{(\w+)\}/\$\{(\w+)\}`\)"#
        ).context("Failed to create regex")?;
        
        // 执行正则替换
        if re.is_match(&content) {
            content = re.replace_all(&content, |caps: &regex::Captures| {
                let var1 = &caps[1]; // 第一个变量（可能是 t）
                let var2 = &caps[2]; // 第二个变量（可能是 v）
                let var3 = &caps[3]; // 第三个变量（可能是 e）
                let var4 = &caps[4]; // 重复的 var2
                let var5 = &caps[5]; // 重复的 var3
                let var6 = &caps[6]; // 重复的 var1
                
                log_info!("✅ [DEBUG] 找到匹配，变量名：var1={}, var2={}, var3={}", var1, var2, var3);
                
                // 构建替换后的字符串
                format!(
                    r#"header.set("x-cursor-checksum",{}===void 0?`${{{}}}${{global.MachineId||{}}}`:`${{{}}}${{global.MachineId||{}}}/${{global.MacMachineId||{}}}`)"#,
                    var1, var2, var3, var4, var5, var6
                )
            }).to_string();
            
            log_info!("✅ [DEBUG] 成功替换 x-cursor-checksum 设置代码");
        } else {
            log_warn!("⚠️ [DEBUG] 未找到需要替换的 x-cursor-checksum 代码");
            return Err(anyhow::anyhow!("未找到需要替换的内容，可能extensionHostProcess文件格式已变化"));
        }

        // 注入获取配置的代码（包含轮询功能）
        let injection_code = r#"
// ==================== SEAMLESS SWITCH EXTENSION INJECTION - START ====================
(async function initMachineIdsPolling() {
    try {
        console.log('[Extension] Starting machine IDs polling...');
        
        // 轮询函数
        async function pollMachineIds() {
            try {
                const response = await fetch('http://127.0.0.1:34567/api/seamless-switch/config');
                if (response.ok) {
                    const data = await response.json();
                    if (data.machineIds) {
                        // 只有当值变化时才更新和打印日志
                        const machineIdChanged = global.MachineId !== data.machineIds.machineId;
                        const macMachineIdChanged = global.MacMachineId !== data.machineIds.macMachineId;
                        
                        if (machineIdChanged || macMachineIdChanged) {
                            global.MachineId = data.machineIds.machineId;
                            global.MacMachineId = data.machineIds.macMachineId;
                            console.log('[Extension] Machine IDs updated:', {
                                machineId: global.MachineId?.substring(0, 20) + '...',
                                macMachineId: global.MacMachineId?.substring(0, 20) + '...',
                                changed: { machineId: machineIdChanged, macMachineId: macMachineIdChanged }
                            });
                        }
                    }
                } else {
                    console.warn('[Extension] Failed to fetch machine IDs config:', response.status);
                }
            } catch (error) {
                console.error('[Extension] Error polling machine IDs:', error);
            }
        }
        
        // 立即执行一次
        await pollMachineIds();
        
        // 开始轮询，每5秒一次
        setInterval(pollMachineIds, 5000);
        console.log('[Extension] Machine IDs polling started, interval: 5000ms');
        
    } catch (error) {
        console.error('[Extension] Failed to start machine IDs polling:', error);
    }
})();
// ==================== SEAMLESS SWITCH EXTENSION INJECTION - END ====================
"#;

        // 在文件开头注入代码
        content = format!("{}\n{}", injection_code, content);
        log_info!("✅ [DEBUG] 成功注入机器ID初始化代码");

        // 写入修改后的内容
        fs::write(&extension_path, content).context("Failed to write modified extensionHostProcess file")?;

        log_info!("✅ [DEBUG] extensionHostProcess.js 修改成功");
        Ok(())
    }

    // 恢复 extensionHostProcess.js 文件
    pub fn restore_extension_host_process() -> Result<()> {
        log_info!("🔄 [DEBUG] 开始恢复 extensionHostProcess.js 文件...");
        
        let extension_path = Self::get_extension_host_process_js_path()?;
        let backup_path = extension_path.with_file_name("extensionHostProcess.js.wuqi.back");

        if !backup_path.exists() {
            log_error!("❌ [DEBUG] extensionHostProcess备份文件不存在: {:?}", backup_path);
            return Err(anyhow::anyhow!("extensionHostProcess备份文件不存在，无法恢复"));
        }

        if !extension_path.exists() {
            log_error!("❌ [DEBUG] extensionHostProcess文件不存在: {:?}", extension_path);
            return Err(anyhow::anyhow!("extensionHostProcess文件不存在"));
        }

        // 从备份恢复原始文件
        fs::copy(&backup_path, &extension_path)
            .context("Failed to restore extensionHostProcess file from backup")?;

        log_info!("✅ [DEBUG] extensionHostProcess.js 已从备份恢复");
        Ok(())
    }

    // 启用无感换号功能（修改workbench文件）
    pub fn enable_seamless_switch() -> Result<String> {
        // 第一步：修改 product.json（删除 checksums 字段）
        log_info!("=== 步骤 1: 修改 product.json ===");
        match Self::modify_product_json() {
            Ok(msg) => {
                log_info!("✅ [DEBUG] product.json处理成功: {}", msg);
            }
            Err(e) => {
                log_warn!("⚠️ [DEBUG] product.json处理失败（继续执行）: {}", e);
            }
        }

        // 第二步：修改 workbench 文件
        log_info!("=== 步骤 2: 修改 workbench 文件 ===");
        let workbench_path = Self::get_workbench_js_path()?;

        if !workbench_path.exists() {
            log_error!("❌ [DEBUG] workbench文件不存在: {:?}", workbench_path);
            return Err(anyhow::anyhow!("workbench文件不存在: {:?}", workbench_path));
        }

        // 检查是否已经修改过
        if Self::check_seamless_switch_status()? {
            log_info!("ℹ️ [DEBUG] 无感换号功能已经启用，无需重复修改");
            return Ok("无感换号功能已经启用".to_string());
        }

        // 创建备份
        let backup_path = workbench_path.with_file_name("workbench.desktop.main.js.wuqi.back");
        if !backup_path.exists() {
            log_info!("📋 [DEBUG] 创建备份文件: {:?}", backup_path);
            fs::copy(&workbench_path, &backup_path).context("Failed to create backup file")?;
        } else {
            log_info!("ℹ️ [DEBUG] 备份文件已存在，跳过备份");
        }

        // 从备份文件读取原始内容（确保每次都从干净的原始文件开始）
        let mut content = fs::read_to_string(&backup_path).context("Failed to read backup file")?;

        log_info!("📖 [DEBUG] 从备份文件读取原始内容，准备进行修改");

        log_info!("🔧 [DEBUG] 开始修改workbench文件...");

        // 执行三个替换操作
        let mut modified = false;

        // 第一个替换：this.storeAccessRefreshToken=(任意参数) => this.storeAccessRefreshToken=window.wuqi=(任意参数)
        // 参数名可能包含 `$` / `_`，也可能只有 1 个参数，需兼容压缩后的变量名
        let re = Regex::new(
            r"this\.storeAccessRefreshToken=\(([$_a-zA-Z][$_a-zA-Z0-9]*(?:\s*,\s*[$_a-zA-Z][$_a-zA-Z0-9]*)*)\)"
        )
        .context("Failed to create regex")?;
        if re.is_match(&content) && !content.contains("this.storeAccessRefreshToken=window.wuqi=(") {
            content = re.replace_all(&content, |caps: &regex::Captures| {
                let params = &caps[1];
                format!("this.storeAccessRefreshToken=window.wuqi=({})", params)
            }).to_string();
            log_info!("✅ [DEBUG] 完成第一个替换：添加wuqi hook");
            modified = true;
        }

        // 第二个替换：this.database.getItems())) => this.database.getItems()));await (async function hookStore(e){if(e.get("releaseNotes/lastVersion"))window.store=e})(this)
        if content.contains("this.database.getItems()))") && 
           !content.contains("this.database.getItems()));await (async function hookStore(e){if(e.get(\"releaseNotes/lastVersion\"))window.store=e})(this)") {
            content = content.replace(
                "this.database.getItems()))",
                "this.database.getItems()));await (async function hookStore(e){if(e.get(\"releaseNotes/lastVersion\"))window.store=e})(this)"
            );
            log_info!("✅ [DEBUG] 完成第二个替换：添加store hook");
            modified = true;
        }

        // 第三个替换：static[Symbol.hasInstance](e){ => static[Symbol.hasInstance](e){window.erroHook&&window.erroHook();
        if content.contains("static[Symbol.hasInstance](e){")
            && !content
                .contains("static[Symbol.hasInstance](e){window.erroHook&&window.erroHook(e);")
        {
            content = content.replace(
                "static[Symbol.hasInstance](e){",
                "static[Symbol.hasInstance](e){window.erroHook&&window.erroHook(e);",
            );
            log_info!("✅ [DEBUG] 完成第三个替换：添加error hook");
            modified = true;
        }

        if !modified {
            log_warn!("⚠️ [DEBUG] 未找到需要替换的内容，可能文件格式已变化");
            return Err(anyhow::anyhow!(
                "未找到需要替换的内容，可能workbench文件格式已变化"
            ));
        }

        // 生成新的机器ID用于JavaScript（复用现有函数）
        let new_ids = match MachineIdRestorer::new() {
            Ok(restorer) => match restorer.generate_new_machine_ids() {
                Ok(ids) => {
                    log_info!(
                        "✅ [DEBUG] 成功生成新的机器ID: dev_device_id={}, machine_id长度={}, mac_machine_id长度={}, sqm_id={}",
                        ids.dev_device_id,
                        ids.machine_id.len(),
                        ids.mac_machine_id.len(),
                        ids.sqm_id
                    );
                    ids
                }
                Err(e) => {
                    log_warn!("⚠️ [DEBUG] 生成机器ID失败，使用默认值: {}", e);
                    // 使用默认值
                    MachineIds {
                        dev_device_id: Uuid::new_v4().to_string(),
                        mac_machine_id: "default-mac-id".to_string(),
                        machine_id: "default-machine-id".to_string(),
                        sqm_id: format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase()),
                        service_machine_id: Uuid::new_v4().to_string(),
                    }
                }
            },
            Err(e) => {
                log_warn!("⚠️ [DEBUG] 初始化MachineIdRestorer失败，使用默认值: {}", e);
                // 使用默认值
                MachineIds {
                    dev_device_id: Uuid::new_v4().to_string(),
                    mac_machine_id: "default-mac-id".to_string(),
                    machine_id: "default-machine-id".to_string(),
                    sqm_id: format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase()),
                    service_machine_id: Uuid::new_v4().to_string(),
                }
            }
        };

        // 定义注入标记，用于检测和清理旧的注入
        let seamless_start_marker = "// ==================== SEAMLESS SWITCH INJECTION - START ====================";
        let seamless_end_marker = "// ==================== SEAMLESS SWITCH INJECTION - END ====================";
        
        // 定义旧的 Email 注入标记（来自 inject_email_update_js 函数）
        let old_email_start_marker = "// Email update injection - START";
        let old_email_end_marker = "// Email update injection - END";
        
        // 清理旧的 Email 注入代码（如果存在）
        if let Some(start_pos) = content.find(old_email_start_marker) {
            if let Some(end_pos) = content.find(old_email_end_marker) {
                log_info!("🔍 [DEBUG] 发现旧的 Email 注入代码，正在清理...");
                let before = &content[..start_pos];
                let after = &content[end_pos + old_email_end_marker.len()..];
                content = format!("{}{}", before, after);
                log_info!("✅ [DEBUG] 旧的 Email 注入代码已清理");
            }
        }

        // 在底部追加轮询代码和错误上报函数
        let polling_script = format!(
            r#"
{}
 // ==================== Email 更新函数 ====================
            window.updateEmailDisplay = function(newEmail) {{
                try {{
                    console.warn('Executing email update for:', newEmail);

                    function updateEmail() {{
                        const emailElement = document.querySelector('p[class="cursor-settings-sidebar-header-email"]');
                        if (emailElement) {{
                            // 性能优化：只有当前值与新值不同时才更新
                            if (emailElement.textContent !== newEmail) {{
                                emailElement.textContent = newEmail;
                                console.warn('Email display updated to:', newEmail);
                            }} else {{
                                console.warn('Email already set to:', newEmail, '- skipping update');
                            }}
                            return true;
                        }}
                        return false;
                    }}

                    // Try immediate update
                    if (updateEmail()) {{
                        console.warn('Email updated successfully');
                        return;
                    }}

                    // If immediate update failed, use MutationObserver to watch for element
                    console.warn('Email element not found, setting up DOM observer...');

                    const observer = new MutationObserver(function(mutations) {{
                        mutations.forEach(function(mutation) {{
                            if (mutation.type === 'childList' && mutation.addedNodes.length > 0) {{
                                if (updateEmail()) {{
                                    console.warn('Email updated via DOM observer');
                                    observer.disconnect(); // 性能优化：成功后断开观察
                                }}
                            }}
                        }});
                    }});

                    // Start observing the document for changes
                    if (document.body) {{
                        observer.observe(document.body, {{
                            childList: true,
                            subtree: true
                        }});
                        console.warn('DOM observer started, watching for email element...');
                    }} else {{
                        document.addEventListener('DOMContentLoaded', function() {{
                            observer.observe(document.body, {{
                                childList: true,
                                subtree: true
                            }});
                            console.warn('DOM observer started after DOMContentLoaded');
                        }});
                    }}
                }} catch (e) {{
                    console.warn('Error updating email display:', e);
                }}
            }};
            console.warn('Email update function initialized');
            // ==================== Email 更新函数 - END ====================

 // ==================== VSCode 风格通知系统（右上角）====================
            (function initVSCodeNotifications() {{
                let notificationContainer = null;
                let notificationId = 0;

                function initNotificationContainer() {{
                  if (!notificationContainer) {{
                    notificationContainer = document.createElement('div');
                    notificationContainer.id = 'vscode-notifications';
                    notificationContainer.style.cssText = `
                      position: fixed;
                      top: 20px;
                      right: 20px;
                      z-index: 999999;
                      display: flex;
                      flex-direction: column;
                      gap: 8px;
                      max-width: 400px;
                    `;
                    document.body.appendChild(notificationContainer);
                    
                    // 添加样式
                    if (!document.getElementById('vscode-notification-styles')) {{
                      const style = document.createElement('style');
                      style.id = 'vscode-notification-styles';
                      style.textContent = `
                        @keyframes slideInRight {{
                          from {{ transform: translateX(450px); opacity: 0; }}
                          to {{ transform: translateX(0); opacity: 1; }}
                        }}
                        @keyframes slideOutRight {{
                          from {{ transform: translateX(0); opacity: 1; }}
                          to {{ transform: translateX(450px); opacity: 0; }}
                        }}
                        @keyframes spin {{
                          0% {{ transform: rotate(0deg); }}
                          100% {{ transform: rotate(360deg); }}
                        }}
                        .vscode-notification:hover {{
                          box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3) !important;
                        }}
                        .vscode-notification-close:hover {{
                          background: rgba(255, 255, 255, 0.1) !important;
                        }}
                        @keyframes progressIndeterminate {{
                          0% {{ transform: translateX(-100%); }}
                          100% {{ transform: translateX(400%); }}
                        }}
                      `;
                      document.head.appendChild(style);
                    }}
                  }}
                  return notificationContainer;
                }}

                function createNotification(message, type = 'info', options = {{}}) {{
                  const container = initNotificationContainer();
                  const id = ++notificationId;
                  
                  const notification = document.createElement('div');
                  notification.className = 'vscode-notification';
                  notification.dataset.id = id;
                  notification.style.cssText = `
                    background: #252526;
                    border-left: 3px solid;
                    padding: 12px 16px;
                    border-radius: 4px;
                    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
                    display: flex;
                    align-items: flex-start;
                    gap: 12px;
                    animation: slideInRight 0.3s ease-out;
                    min-width: 350px;
                    cursor: default;
                    transition: box-shadow 0.2s;
                  `;

                  const styles = {{
                    success: {{ color: '#4ec9b0', icon: '✓', borderColor: '#4ec9b0' }},
                    error: {{ color: '#f48771', icon: '✕', borderColor: '#f48771' }},
                    warning: {{ color: '#cca700', icon: '⚠', borderColor: '#cca700' }},
                    info: {{ color: '#3794ff', icon: 'ℹ', borderColor: '#3794ff' }}
                  }};

                  const {{ color, icon, borderColor }} = styles[type] || styles.info;
                  notification.style.borderLeftColor = borderColor;

                  // 图标
                  const iconEl = document.createElement('div');
                  iconEl.style.cssText = `
                    width: 20px;
                    height: 20px;
                    border-radius: 50%;
                    background: ${{color}};
                    color: #252526;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 12px;
                    font-weight: bold;
                    flex-shrink: 0;
                    margin-top: 2px;
                  `;
                  iconEl.textContent = icon;

                  // 内容区域
                  const content = document.createElement('div');
                  content.style.cssText = `
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    gap: 8px;
                  `;

                  // 消息文本
                  const messageEl = document.createElement('div');
                  messageEl.style.cssText = `
                    color: #cccccc;
                    font-size: 13px;
                    line-height: 1.4;
                    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
                  `;
                  messageEl.textContent = message;

                  content.appendChild(messageEl);

                  // 如果有进度条（loading）
                  if (options.loading) {{
                    const progressBar = document.createElement('div');
                    progressBar.style.cssText = `
                      width: 100%;
                      height: 2px;
                      background: rgba(255, 255, 255, 0.1);
                      border-radius: 1px;
                      overflow: hidden;
                      margin-top: 4px;
                    `;
                    
                    const progressFill = document.createElement('div');
                    progressFill.style.cssText = `
                      height: 100%;
                      background: ${{color}};
                      width: 100%;
                      animation: progressIndeterminate 1.5s ease-in-out infinite;
                    `;
                    
                    progressBar.appendChild(progressFill);
                    content.appendChild(progressBar);
                  }}

                  // 关闭按钮
                  const closeBtn = document.createElement('button');
                  closeBtn.className = 'vscode-notification-close';
                  closeBtn.textContent = '✕';
                  closeBtn.style.cssText = `
                    background: transparent;
                    border: none;
                    color: #cccccc;
                    font-size: 16px;
                    cursor: pointer;
                    padding: 4px;
                    width: 24px;
                    height: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    border-radius: 4px;
                    flex-shrink: 0;
                    transition: background 0.2s;
                  `;
                  closeBtn.onclick = () => removeNotification(id);

                  notification.appendChild(iconEl);
                  notification.appendChild(content);
                  notification.appendChild(closeBtn);
                  container.appendChild(notification);

                  // 自动关闭（除非是 loading）
                  if (!options.loading && !options.persistent) {{
                    const duration = options.duration || 5000;
                    setTimeout(() => removeNotification(id), duration);
                  }}

                  return id;
                }}

                function removeNotification(id) {{
                  const container = notificationContainer;
                  if (!container) return;
                  
                  const notification = container.querySelector(`[data-id="${{id}}"]`);
                  if (notification) {{
                    notification.style.animation = 'slideOutRight 0.3s ease-out';
                    setTimeout(() => {{
                      notification.remove();
                      if (container.children.length === 0) {{
                        container.remove();
                        notificationContainer = null;
                      }}
                    }}, 300);
                  }}
                }}

                function removeAllNotifications() {{
                  if (notificationContainer) {{
                    notificationContainer.remove();
                    notificationContainer = null;
                  }}
                }}

                // ==================== 快捷方法 ====================

                window.$success = (msg, duration) => createNotification(msg, 'success', {{ duration }});
                window.$error = (msg, duration) => createNotification(msg, 'error', {{ duration }});
                window.$warning = (msg, duration) => createNotification(msg, 'warning', {{ duration }});
                window.$info = (msg, duration) => createNotification(msg, 'info', {{ duration }});

                // Loading 通知（右上角，可关闭）
                window.$loading = (msg = '加载中...') => {{
                  return createNotification(msg, 'info', {{ loading: true, persistent: true }});
                }};

                // 隐藏特定的 loading
                window.$hideLoading = (id) => {{
                  if (id) {{
                    removeNotification(id);
                  }} else {{
                    // 如果没有指定 id，移除所有通知
                    removeAllNotifications();
                  }}
                }};

                // 清除所有通知
                window.$clearAll = () => removeAllNotifications();
                
                console.warn('VSCode notification system initialized');
            }})();
            // ==================== VSCode 风格通知系统 - END ====================
            

            // 无感换号轮询功能 - START
            (async function seamlessPolling() {{
                try {{
                    console.warn('Starting seamless polling service...');
                    
                    // 防抖相关变量
                    let loadingDebounceTimer = null;
                    let currentLoadingId = null;
                    
                    // 实现错误上报钩子
                    window.erroHook = async function(errorInfo) {{
                        try {{
                            // 如果有参数，转换为JSON字符串
                            let errorData = '';
                            if (errorInfo !== undefined && errorInfo !== null) {{
                                try {{
                                    errorData = JSON.stringify(errorInfo);
                                    
                                    // 检测到限流错误时，先检查是否开启了自动轮换
                                    if(errorData.includes('ERROR_RATE_LIMITED_CHANGEABLE')){{
                                        // 清除之前的防抖定时器
                                        if (loadingDebounceTimer) {{
                                            clearTimeout(loadingDebounceTimer);
                                            console.warn('Debounce: cleared previous timer');
                                        }}
                                        
                                        // 设置新的防抖定时器，3秒后执行
                                        loadingDebounceTimer = setTimeout(async () => {{
                                            try {{
                                                const configResponse = await fetch('http://127.0.0.1:34567/api/auto-switch/config');
                                                if (configResponse.ok) {{
                                                    const config = await configResponse.json();
                                                    if (config.autoSwitchEnabled) {{
                                                        // 如果已经有 loading，先关闭
                                                        if (currentLoadingId) {{
                                                            window.$hideLoading(currentLoadingId);
                                                        }}
                                                        // 显示新的 loading
                                                        currentLoadingId = window.$loading('正在处理无感换号，请稍后...');
                                                        console.warn('Auto switch is enabled, showing loading notification (debounced)');
                                                    }} else {{
                                                        console.warn('Auto switch is disabled, skip loading notification');
                                                    }}
                                                }} else {{
                                                    console.warn('Failed to fetch auto-switch config:', configResponse.status);
                                                }}
                                            }} catch (configError) {{
                                                console.warn('Error checking auto-switch config:', configError);
                                            }}
                                        }}, 3000);
                                        
                                        console.warn('Debounce: timer set for 3000ms');
                                    }}
                                }} catch (e) {{
                                    errorData = String(errorInfo);
                                }}
                            }}
                            
                            // 调用后端接口上报错误
                            const response = await fetch('http://127.0.0.1:34567/api/error-report', {{
                                method: 'POST',
                                headers: {{
                                    'Content-Type': 'application/json',
                                }},
                                body: JSON.stringify({{ error: errorData }})
                            }});
                            
                            if (!response.ok) {{
                                console.warn('Error report failed:', response.status);
                            }}
                        }} catch (error) {{
                            console.warn('Failed to report error:', error);
                        }}
                    }};
                    console.warn('Error hook initialized');
                    
                    async function pollSwitchConfig() {{
                        try {{
                            const response = await fetch('http://127.0.0.1:34567/api/seamless-switch/config');
                            if (!response.ok) {{
                                console.warn('Failed to fetch switch config:', response.status);
                                return;
                            }}
                            
                            const data = await response.json();
                            
                            if (data.isSwitch === 1 && window.store.get("cursorAuth/accessToken") !== data.accessToken) {{
                                console.warn('Switch detected! Executing token switch...');
                                
                                // 清除旧的token
                                if (window.store) {{
                                    await window.store.delete("cursorAuth/accessToken");
                                    await window.store.delete("cursorAuth/refreshToken");
                                    if (data?.email) {{
                                        await window.store.set("cursorAuth/cachedEmail", data?.email);
                                    }}

                                    console.warn('Old tokens deleted');
                                    
                                    // 设置新的机器ID (从接口动态获取)
                                    try {{
                                        if (data.machineIds) {{
                                            await window.store.set('telemetry.devDeviceId', data.machineIds.devDeviceId);
                                            await window.store.set("telemetry.macMachineId", data.machineIds.macMachineId);
                                            await window.store.set("telemetry.machineId", data.machineIds.machineId);
                                            await window.store.set("telemetry.sqmId", data.machineIds.sqmId);
                                            await window.store.set("storage.serviceMachineId", data.machineIds.serviceMachineId);

                                            console.warn('Machine IDs updated successfully from API');
                                            console.warn('New IDs set:', {{
                                                devDeviceId: data.machineIds.devDeviceId,
                                                macMachineId: data.machineIds.macMachineId.substring(0, 20) + '...',
                                                machineId: data.machineIds.machineId.substring(0, 20) + '...',
                                                sqmId: data.machineIds.sqmId,
                                                serviceMachineId: data.machineIds.serviceMachineId
                                            }});
                                        }} else {{
                                            console.warn('Warning: No machineIds in response, using initial IDs');
                                            // 使用初始化时的机器ID作为后备方案
                                            await window.store.set('telemetry.devDeviceId', "{}");
                                            await window.store.set("telemetry.macMachineId", "{}");
                                            await window.store.set("telemetry.machineId", "{}");
                                            await window.store.set("telemetry.sqmId", "{}");
                                            await window.store.set("storage.serviceMachineId", "{}");
                                        }}
                                    }} catch (machineIdError) {{
                                        console.error('Failed to update machine IDs, but continuing with token update:', machineIdError);
                                    }}

                                // 设置新的token (使用accessToken作为两个参数)
                                    if (window.wuqi && data.accessToken) {{
                                        window.wuqi(data.accessToken, data.accessToken);
                                        console.warn('New tokens set via wuqi');
                                    }}
                                    
                                    // 更新 email 显示
                                    if (data.email && window.updateEmailDisplay) {{
                                        window.updateEmailDisplay(data.email);
                                        console.warn('Email update triggered for:', data.email);
                                    }}
                                    
                                    // 清除防抖定时器和 loading
                                    if (loadingDebounceTimer) {{
                                        clearTimeout(loadingDebounceTimer);
                                        loadingDebounceTimer = null;
                                    }}
                                    if (currentLoadingId) {{
                                        window.$hideLoading(currentLoadingId);
                                        currentLoadingId = null;
                                    }}
                                    
                                    window.$success('无感换号成功');
                                }}
                            }}
                        }} catch (error) {{
                            console.warn('Error in polling:', error);
                        }}
                    }}
                    
                    // 开始轮询，每1000ms一次
                    setInterval(pollSwitchConfig, 5000);
                    console.warn('Seamless polling started, interval: 5000ms');
                    
                }} catch (error) {{
                    console.warn('Error starting seamless polling:', error);
                }}
            }})();
            // 无感换号轮询功能 - END
{}
            "#,
            seamless_start_marker,
            new_ids.dev_device_id,
            new_ids.mac_machine_id,
            new_ids.machine_id,
            new_ids.sqm_id,
            new_ids.service_machine_id,
            seamless_end_marker
        );

        // 检查是否已经存在旧的注入，如果存在则先删除
        let final_content = if let Some(start_pos) = content.find(seamless_start_marker) {
            if let Some(end_pos) = content.find(seamless_end_marker) {
                // 找到了完整的旧注入，删除它
                log_info!("🔍 [DEBUG] 发现旧的无感换号注入，正在清理...");
                let before = &content[..start_pos];
                let after = &content[end_pos + seamless_end_marker.len()..];
                // 删除旧的后，追加新的
                format!("{}{}{}", before, polling_script, after)
            } else {
                // 只找到开始标记，没有结束标记，直接追加新的
                log_warn!("⚠️ [DEBUG] 发现不完整的旧注入（只有开始标记），追加新注入");
                format!("{}{}", content, polling_script)
            }
        } else {
            // 没有找到旧的注入，直接追加
            log_info!("📝 [DEBUG] 首次注入无感换号代码");
            format!("{}{}", content, polling_script)
        };

        // 写入修改后的内容
        fs::write(&workbench_path, final_content).context("Failed to write modified workbench file")?;

        log_info!("✅ [DEBUG] workbench文件修改完成");

        // 第三步：修改 extensionHostProcess 文件
        log_info!("=== 步骤 3: 修改 extensionHostProcess 文件 ===");
        match Self::modify_extension_host_process() {
            Ok(_) => {
                log_info!("✅ [DEBUG] extensionHostProcess.js处理成功");
            }
            Err(e) => {
                log_warn!("⚠️ [DEBUG] extensionHostProcess.js处理失败（继续执行）: {}", e);
                // 如果 extensionHostProcess 修改失败，不影响整体流程
            }
        }

        log_info!("✅ [DEBUG] 无感换号功能启用成功");
        Ok("无感换号功能启用成功".to_string())
    }

    // 禁用无感换号功能（恢复备份文件）
    pub fn disable_seamless_switch() -> Result<String> {
        let workbench_path = Self::get_workbench_js_path()?;
        let backup_path = workbench_path.with_file_name("workbench.desktop.main.js.wuqi.back");

        if !backup_path.exists() {
            log_error!("❌ [DEBUG] 备份文件不存在: {:?}", backup_path);
            return Err(anyhow::anyhow!("备份文件不存在，无法恢复"));
        }

        if !workbench_path.exists() {
            log_error!("❌ [DEBUG] workbench文件不存在: {:?}", workbench_path);
            return Err(anyhow::anyhow!("workbench文件不存在"));
        }

        log_info!("🔄 [DEBUG] 从备份恢复workbench文件...");

        // 从备份恢复原始文件
        fs::copy(&backup_path, &workbench_path)
            .context("Failed to restore workbench file from backup")?;

        log_info!("✅ [DEBUG] workbench文件已恢复");

        // 恢复 extensionHostProcess 文件
        log_info!("🔄 [DEBUG] 恢复 extensionHostProcess 文件...");
        match Self::restore_extension_host_process() {
            Ok(_) => {
                log_info!("✅ [DEBUG] extensionHostProcess.js恢复成功");
            }
            Err(e) => {
                log_warn!("⚠️ [DEBUG] extensionHostProcess.js恢复失败（继续执行）: {}", e);
                // 如果 extensionHostProcess 恢复失败，不影响整体流程
            }
        }

        log_info!("✅ [DEBUG] 无感换号功能已禁用，所有文件已恢复");
        Ok("无感换号功能已禁用".to_string())
    }

    pub fn modify_main_js(&self, main_js_path: &Path) -> Result<()> {
        self.log_info(&format!("开始修改main.js文件: {:?}", main_js_path));

        if !main_js_path.exists() {
            let error_msg = format!("main.js file not found: {}", main_js_path.display());
            self.log_error(&error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        // 读取文件内容
        self.log_debug("读取main.js文件内容...");
        let content = fs::read_to_string(main_js_path).context("Failed to read main.js file")?;
        self.log_info(&format!("main.js文件大小: {} 字节", content.len()));

        // 创建备份
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = format!("{}.backup.{}", main_js_path.display(), timestamp);
        self.log_info(&format!("创建main.js备份: {}", backup_path));
        fs::copy(main_js_path, &backup_path).context("Failed to create backup of main.js")?;

        // 应用正则表达式替换
        let patterns = vec![
            (
                r"async getMachineId\(\)\{return [^??]+\?\?([^}]+)\}",
                r"async getMachineId(){return $1}",
            ),
            (
                r"async getMacMachineId\(\)\{return [^??]+\?\?([^}]+)\}",
                r"async getMacMachineId(){return $1}",
            ),
        ];

        let mut modified_content = content.clone();
        let mut patterns_applied = 0;

        for (i, (pattern, replacement)) in patterns.iter().enumerate() {
            self.log_debug(&format!("应用模式 {}: {}", i + 1, pattern));
            let re = Regex::new(pattern)?;
            let before_len = modified_content.len();
            modified_content = re.replace_all(&modified_content, *replacement).to_string();
            let after_len = modified_content.len();

            if before_len != after_len {
                patterns_applied += 1;
                self.log_info(&format!(
                    "模式 {} 已应用，内容长度从 {} 变为 {}",
                    i + 1,
                    before_len,
                    after_len
                ));
            } else {
                self.log_debug(&format!("模式 {} 未找到匹配项", i + 1));
            }
        }

        self.log_info(&format!("总共应用了 {} 个模式", patterns_applied));

        // 写回文件
        self.log_debug("写入修改后的main.js内容...");
        fs::write(main_js_path, modified_content).context("Failed to write modified main.js")?;
        self.log_info("main.js文件修改完成");

        Ok(())
    }

    pub fn inject_email_update_js(&self, email: &str) -> Result<()> {
        match Self::get_workbench_js_path() {
            Ok(workbench_path) => {
                if !workbench_path.exists() {
                    return Err(anyhow::anyhow!(
                        "workbench.desktop.main.js file not found: {}",
                        workbench_path.display()
                    ));
                }

                // Read the file content
                let content = fs::read_to_string(&workbench_path)
                    .context("Failed to read workbench.desktop.main.js file")?;

                // 检查是否已经有无感换号注入，如果有就不再注入邮箱更新代码
                let seamless_start_marker = "// ==================== SEAMLESS SWITCH INJECTION - START ====================";
                if content.contains(seamless_start_marker) {
                    log_info!("🔍 检测到已存在无感换号注入，跳过邮箱更新注入");
                    return Ok(());
                }

                // Create backup only if we haven't created one recently
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let backup_path = format!("{}.backup.{}", workbench_path.display(), timestamp);
                fs::copy(&workbench_path, &backup_path)
                    .context("Failed to create backup of workbench.desktop.main.js")?;

                // Define markers to identify our injected code
                let start_marker = "// Email update injection - START";
                let end_marker = "// Email update injection - END";

                // Create the email update JavaScript code with dynamic email injection
                let email_update_script = format!(
                    r#"
{}
(function() {{
    try {{
        console.warn('Executing email update for: {}');
        const cachedEmail = window.store ? window.store.get("cursorAuth/cachedEmail") : "";
        console.warn('Cached email:', cachedEmail);
        
        function updateEmailDisplay(newEmail) {{
            const emailElement = document.querySelector('p[class="cursor-settings-sidebar-header-email"]');
            if (emailElement) {{
                const targetEmail = cachedEmail || newEmail;
                // 性能优化：只有当前值与新值不同时才更新
                if (emailElement.textContent !== targetEmail) {{
                    emailElement.textContent = targetEmail;
                    console.warn('Email display updated to:', targetEmail);
                }} else {{
                    console.warn('Email already set to:', targetEmail, '- skipping update');
                }}
                return true;
            }}
            return false;
        }}

        // Try immediate update
        if (updateEmailDisplay(cachedEmail || '{}')) {{
            console.warn('Email updated successfully');
            return; // Exit if successful
        }}

        // If immediate update failed, use MutationObserver to watch for element
        console.warn('Email element not found, setting up DOM observer...');

        const observer = new MutationObserver(function(mutations) {{
            mutations.forEach(function(mutation) {{
                // Check if any new nodes were added
                if (mutation.type === 'childList' && mutation.addedNodes.length > 0) {{
                    // Try to update email display
                    if (updateEmailDisplay(cachedEmail || '{}')) {{
                        console.warn('Email updated via DOM observer');
                        observer.disconnect(); // 性能优化：成功后立即断开观察
                    }}
                }}
            }});
        }});

        // Start observing the document for changes
        if (document.body) {{
            observer.observe(document.body, {{
                childList: true,
                subtree: true
            }});
            console.warn('DOM observer started, watching for email element...');
        }} else {{
            // If body not ready, wait for it
            document.addEventListener('DOMContentLoaded', function() {{
                observer.observe(document.body, {{
                    childList: true,
                    subtree: true
                }});
                console.warn('DOM observer started after DOMContentLoaded');
            }});
        }}

        // Observer will automatically stop when email element is found and updated
    }} catch (e) {{
        console.warn('Error updating email display:', e);
    }}
}})();
{}
"#,
                    start_marker, email, email, email, end_marker
                );

                // Check if our injection already exists and remove it
                let modified_content = if let Some(start_pos) = content.find(start_marker) {
                    if let Some(end_pos) = content.find(end_marker) {
                        // Remove existing injection
                        let before = &content[..start_pos];
                        let after = &content[end_pos + end_marker.len()..];
                        format!("{}{}{}", before, email_update_script, after)
                    } else {
                        // Start marker found but no end marker, append new injection
                        format!("{}\n{}", content, email_update_script)
                    }
                } else {
                    // No existing injection, append new one
                    format!("{}\n{}", content, email_update_script)
                };

                // Write back to file
                fs::write(&workbench_path, modified_content)
                    .context("Failed to write modified workbench.desktop.main.js")?;

                log_info!("Email update script injected for: {}", email);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Could not locate workbench.desktop.main.js: {}",
                e
            )),
        }
    }

    pub fn modify_workbench_js(&self, workbench_path: &Path) -> Result<()> {
        self.log_info(&format!(
            "开始修改workbench.desktop.main.js文件: {:?}",
            workbench_path
        ));

        if !workbench_path.exists() {
            let error_msg = format!(
                "workbench.desktop.main.js file not found: {}",
                workbench_path.display()
            );
            self.log_error(&error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }

        // 读取文件内容
        self.log_debug("读取workbench.desktop.main.js文件内容...");
        let content = fs::read_to_string(workbench_path)
            .context("Failed to read workbench.desktop.main.js file")?;
        self.log_info(&format!(
            "workbench.desktop.main.js文件大小: {} 字节",
            content.len()
        ));

        // 创建备份
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = format!("{}.backup.{}", workbench_path.display(), timestamp);
        self.log_info(&format!(
            "创建workbench.desktop.main.js备份: {}",
            backup_path
        ));
        fs::copy(workbench_path, &backup_path)
            .context("Failed to create backup of workbench.desktop.main.js")?;

        // 平台特定模式
        let (button_pattern, button_replacement) = if cfg!(target_os = "windows")
            || cfg!(target_os = "linux")
        {
            (
                r#"$(k,E(Ks,{title:"Upgrade to Pro",size:"small",get codicon(){return F.rocket},get onClick(){return t.pay}}),null)"#,
                r#"$(k,E(Ks,{title:"wuqi-y GitHub",size:"small",get codicon(){return F.rocket},get onClick(){return function(){window.open("https://github.com/wuqi-y/auto-cursor-releases","_blank")}}}),null)"#,
            )
        } else {
            (
                r#"M(x,I(as,{title:"Upgrade to Pro",size:"small",get codicon(){return $.rocket},get onClick(){return t.pay}}),null)"#,
                r#"M(x,I(as,{title:"wuqi-y GitHub",size:"small",get codicon(){return $.rocket},get onClick(){return function(){window.open("https://github.com/wuqi-y/auto-cursor-releases","_blank")}}}),null)"#,
            )
        };

        self.log_info(&format!(
            "当前平台: {}, 使用对应的按钮模式",
            std::env::consts::OS
        ));

        // 应用替换
        let mut modified_content = content.clone();
        let mut replacements_made = 0;

        // 按钮替换
        self.log_debug("应用按钮替换...");
        let before_len = modified_content.len();
        modified_content = modified_content.replace(button_pattern, button_replacement);
        if modified_content.len() != before_len {
            replacements_made += 1;
            self.log_info("按钮替换成功应用");
        } else {
            self.log_warning("按钮模式未找到匹配项");
        }

        // 徽章替换
        self.log_debug("应用徽章替换...");
        let before_len = modified_content.len();
        modified_content = modified_content.replace("<div>Pro Trial", "<div>Pro");
        if modified_content.len() != before_len {
            replacements_made += 1;
            self.log_info("徽章替换成功应用");
        } else {
            self.log_debug("徽章模式未找到匹配项");
        }

        // 隐藏通知
        self.log_debug("应用通知隐藏...");
        let before_len = modified_content.len();
        modified_content =
            modified_content.replace("notifications-toasts", "notifications-toasts hidden");
        if modified_content.len() != before_len {
            replacements_made += 1;
            self.log_info("通知隐藏成功应用");
        } else {
            self.log_debug("通知模式未找到匹配项");
        }

        // Token限制绕过
        self.log_debug("应用Token限制绕过...");
        let before_len = modified_content.len();
        modified_content = modified_content.replace(
            "async getEffectiveTokenLimit(e){const n=e.modelName;if(!n)return 2e5;",
            "async getEffectiveTokenLimit(e){return 9000000;const n=e.modelName;if(!n)return 9e5;",
        );
        if modified_content.len() != before_len {
            replacements_made += 1;
            self.log_info("Token限制绕过成功应用");
        } else {
            self.log_debug("Token限制模式未找到匹配项");
        }

        // Pro状态修改
        self.log_debug("应用Pro状态修改...");
        let before_len = modified_content.len();
        modified_content = modified_content.replace(
            r#"var DWr=ne("<div class=settings__item_description>You are currently signed in with <strong></strong>.");"#,
            r#"var DWr=ne("<div class=settings__item_description>You are currently signed in with <strong></strong>. <h1>Pro</h1>");"#,
        );
        if modified_content.len() != before_len {
            replacements_made += 1;
            self.log_info("Pro状态修改成功应用");
        } else {
            self.log_debug("Pro状态模式未找到匹配项");
        }

        self.log_info(&format!("总共应用了 {} 个替换", replacements_made));

        // 写回文件
        self.log_debug("写入修改后的workbench.desktop.main.js内容...");
        fs::write(workbench_path, modified_content)
            .context("Failed to write modified workbench.desktop.main.js")?;
        self.log_info("workbench.desktop.main.js文件修改完成");

        Ok(())
    }

    pub fn complete_cursor_reset(&self) -> Result<ResetResult> {
        let mut details = Vec::new();
        let mut success = true;

        // 记录系统信息和开始日志
        self.log_system_info();
        self.log_info("开始完整的 Cursor 重置流程...");
        details.push("Starting complete Cursor reset process...".to_string());

        // 第一步：重置机器ID
        self.log_info("=== 步骤 1: 重置机器ID ===");
        match self.reset_machine_ids() {
            Ok(reset_result) => {
                self.log_info(&format!(
                    "机器ID重置结果: success={}, message={}",
                    reset_result.success, reset_result.message
                ));
                for detail in &reset_result.details {
                    self.log_debug(&format!("机器ID重置详情: {}", detail));
                }
                details.extend(reset_result.details);
                if !reset_result.success {
                    success = false;
                    self.log_error("机器ID重置失败");
                } else {
                    self.log_info("机器ID重置成功");
                }
            }
            Err(e) => {
                success = false;
                let error_msg = format!("Failed to reset machine IDs: {}", e);
                self.log_error(&error_msg);
                details.push(error_msg);
            }
        }

        // 第二步：修改 main.js
        self.log_info("=== 步骤 2: 修改 main.js ===");
        match Self::get_cursor_app_paths() {
            Ok((package_json, main_js)) => {
                self.log_info(&format!(
                    "找到Cursor应用路径: package.json={:?}, main.js={:?}",
                    package_json, main_js
                ));
                self.log_info(&format!(
                    "package.json存在: {}, main.js存在: {}",
                    package_json.exists(),
                    main_js.exists()
                ));

                if package_json.exists() && main_js.exists() {
                    self.log_info("开始修改 main.js 文件...");
                    match self.modify_main_js(&main_js) {
                        Ok(()) => {
                            let success_msg = "Successfully modified main.js".to_string();
                            self.log_info(&success_msg);
                            details.push(success_msg);
                        }
                        Err(e) => {
                            let error_msg = format!("Warning: Failed to modify main.js: {}", e);
                            self.log_warning(&error_msg);
                            details.push(error_msg);
                        }
                    }
                } else {
                    let warning_msg = "Warning: Could not find Cursor main.js file".to_string();
                    self.log_warning(&warning_msg);
                    self.log_warning(&format!(
                        "详细检查: package.json路径={:?}, 存在={}",
                        package_json,
                        package_json.exists()
                    ));
                    self.log_warning(&format!(
                        "详细检查: main.js路径={:?}, 存在={}",
                        main_js,
                        main_js.exists()
                    ));
                    details.push(warning_msg);
                }
            }
            Err(e) => {
                let error_msg = format!("Warning: Could not locate Cursor installation: {}", e);
                self.log_error(&error_msg);
                details.push(error_msg);
            }
        }

        // 第三步：修改 product.json（删除 checksums 字段）
        self.log_info("=== 步骤 3: 修改 product.json ===");
        match Self::modify_product_json() {
            Ok(msg) => {
                self.log_info(&format!("✅ product.json处理成功: {}", msg));
                details.push(msg);
            }
            Err(e) => {
                let warning_msg = format!("Warning: Failed to modify product.json: {}", e);
                self.log_warning(&warning_msg);
                details.push(warning_msg);
            }
        }

        // 第四步：修改 workbench.desktop.main.js
        self.log_info("=== 步骤 4: 修改 workbench.desktop.main.js ===");
        match Self::get_workbench_js_path() {
            Ok(workbench_path) => {
                self.log_info(&format!("找到workbench路径: {:?}", workbench_path));
                self.log_info(&format!("workbench文件存在: {}", workbench_path.exists()));

                if workbench_path.exists() {
                    self.log_info("开始修改 workbench.desktop.main.js 文件...");
                    match self.modify_workbench_js(&workbench_path) {
                        Ok(()) => {
                            let success_msg =
                                "Successfully modified workbench.desktop.main.js".to_string();
                            self.log_info(&success_msg);
                            details.push(success_msg);
                        }
                        Err(e) => {
                            let error_msg = format!(
                                "Warning: Failed to modify workbench.desktop.main.js: {}",
                                e
                            );
                            self.log_warning(&error_msg);
                            details.push(error_msg);
                        }
                    }
                } else {
                    let warning_msg =
                        "Warning: Could not find workbench.desktop.main.js file".to_string();
                    self.log_warning(&warning_msg);
                    self.log_warning(&format!(
                        "详细检查: workbench路径={:?}, 存在={}",
                        workbench_path,
                        workbench_path.exists()
                    ));
                    details.push(warning_msg);
                }
            }
            Err(e) => {
                let error_msg =
                    format!("Warning: Could not locate workbench.desktop.main.js: {}", e);
                self.log_error(&error_msg);
                details.push(error_msg);
            }
        }

        let message = if success {
            "Complete Cursor reset successful".to_string()
        } else {
            "Complete Cursor reset completed with some errors".to_string()
        };

        self.log_info("=== Cursor 重置流程完成 ===");
        self.log_info(&format!("最终结果: {}", message));
        self.log_info(&format!("成功状态: {}", success));
        self.log_info(&format!("详细信息条目数: {}", details.len()));

        Ok(ResetResult {
            success,
            message,
            details,
            new_ids: None,
        })
    }

    pub fn get_current_machine_ids(&self) -> Result<Option<MachineIds>> {
        if !self.db_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.db_path).context("Failed to read storage file")?;

        let data: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse storage JSON")?;

        let dev_device_id = data
            .get("telemetry.devDeviceId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mac_machine_id = data
            .get("telemetry.macMachineId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let machine_id = data
            .get("telemetry.machineId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let sqm_id = data
            .get("telemetry.sqmId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let service_machine_id = data
            .get("storage.serviceMachineId")
            .and_then(|v| v.as_str())
            .unwrap_or(&dev_device_id)
            .to_string();

        // Check if any IDs exist
        if dev_device_id.is_empty()
            && mac_machine_id.is_empty()
            && machine_id.is_empty()
            && sqm_id.is_empty()
            && service_machine_id.is_empty()
        {
            return Ok(None);
        }

        Ok(Some(MachineIds {
            dev_device_id,
            mac_machine_id,
            machine_id,
            sqm_id,
            service_machine_id,
        }))
    }

    pub fn get_machine_id_file_content(&self) -> Result<Option<String>> {
        let machine_id_path = Self::get_machine_id_path()?;

        if !machine_id_path.exists() {
            return Ok(None);
        }

        let content =
            fs::read_to_string(&machine_id_path).context("Failed to read machine ID file")?;

        Ok(Some(content.trim().to_string()))
    }

    pub fn get_backup_directory_info(&self) -> Result<(String, Vec<String>)> {
        let db_dir = self
            .db_path
            .parent()
            .context("Could not get parent directory")?;

        let mut all_files = Vec::new();

        if let Ok(entries) = fs::read_dir(db_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy().to_string();
                        // Only include storage.json related files
                        if filename_str.contains("storage.json") {
                            all_files.push(filename_str);
                        }
                    }
                }
            }
        }

        all_files.sort();

        Ok((db_dir.to_string_lossy().to_string(), all_files))
    }
}
