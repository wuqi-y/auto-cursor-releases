use crate::machine_id::MachineIdRestorer;
use crate::{log_debug, log_error, log_info, log_warn};
use std::fs;
use std::path::PathBuf;

/// 获取 Cursor 版本信息
///
/// 该函数会根据不同操作系统尝试多种方法获取 Cursor 版本：
/// - macOS: 从 Info.plist 或 package.json 读取
/// - Windows: 优先使用自定义路径，然后尝试 where 命令和常见安装位置
/// - Linux: 从常见的安装目录读取 package.json
pub async fn get_cursor_version() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        get_cursor_version_macos()
    }

    #[cfg(target_os = "windows")]
    {
        get_cursor_version_windows()
    }

    #[cfg(target_os = "linux")]
    {
        get_cursor_version_linux()
    }
}

#[cfg(target_os = "macos")]
fn get_cursor_version_macos() -> Result<String, String> {
    log_info!("macOS: 开始获取 Cursor 版本信息");

    // 方法1: 尝试从 Info.plist 读取版本
    let app_path = PathBuf::from("/Applications/Cursor.app/Contents/Info.plist");
    if app_path.exists() {
        if let Ok(content) = fs::read_to_string(&app_path) {
            // 简单的正则匹配版本号
            if let Some(version_start) = content.find("<key>CFBundleShortVersionString</key>") {
                if let Some(version_content) = content[version_start..].find("<string>") {
                    let start_idx = version_start + version_content + 8;
                    if let Some(end_idx) = content[start_idx..].find("</string>") {
                        let version = &content[start_idx..start_idx + end_idx];
                        log_info!("从 Info.plist 获取版本: {}", version);
                        return Ok(version.to_string());
                    }
                }
            }
        }
    }

    // 方法2: 尝试读取 package.json
    let package_path =
        PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/package.json");
    if package_path.exists() {
        if let Ok(content) = fs::read_to_string(&package_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                    log_info!("从 package.json 获取版本: {}", version);
                    return Ok(version.to_string());
                }
            }
        }
    }

    log_error!("macOS: 无法获取 Cursor 版本信息");
    Err("无法获取 Cursor 版本信息".to_string())
}

#[cfg(target_os = "windows")]
fn get_cursor_version_windows() -> Result<String, String> {
    log_info!("Windows: 开始获取 Cursor 版本信息");

    // 优先方法1: 使用 MachineIdRestorer 获取自定义路径
    log_info!("优先尝试使用 MachineIdRestorer 获取自定义 Cursor 路径");
    match MachineIdRestorer::new() {
        Ok(restorer) => {
            // 首先尝试自定义路径
            if let Some(custom_path) = restorer.get_custom_cursor_path() {
                log_info!("找到自定义 Cursor 路径: {}", custom_path);

                // 尝试从自定义路径读取版本
                if let Ok(version) = try_read_version_from_cursor_path(&custom_path) {
                    log_info!("从自定义路径成功获取版本: {}", version);
                    return Ok(version);
                }

                log_warn!("自定义路径存在但无法读取版本，继续尝试其他方法");
            } else {
                log_info!("未配置自定义 Cursor 路径");
            }

            // 如果自定义路径没有找到版本，尝试使用 MachineIdRestorer 的默认方法
            match restorer.get_cursor_version() {
                Ok(version) => {
                    log_info!("通过 MachineIdRestorer 默认方法成功获取版本: {}", version);
                    return Ok(version);
                }
                Err(e) => {
                    log_warn!("MachineIdRestorer 默认方法获取版本失败: {}", e);
                }
            }
        }
        Err(e) => {
            log_warn!("无法初始化 MachineIdRestorer: {}", e);
        }
    }

    // 方法2: 使用 where 命令查找 Cursor.exe
    log_info!("尝试使用 where 命令查找 Cursor.exe");
    if let Ok(output) = std::process::Command::new("where")
        .arg("Cursor.exe")
        .output()
    {
        if output.status.success() {
            if let Ok(path_str) = String::from_utf8(output.stdout) {
                let path_str = path_str.trim();
                log_info!("where 命令找到: {}", path_str);
                if !path_str.is_empty() {
                    let exe_path = PathBuf::from(path_str.lines().next().unwrap_or(""));
                    if exe_path.exists() {
                        log_info!("确认可执行文件存在: {:?}", exe_path);
                        // Cursor.exe 通常在 Cursor 目录下，package.json 在 resources/app/package.json
                        if let Some(cursor_dir) = exe_path.parent() {
                            let package_path = cursor_dir
                                .join("resources")
                                .join("app")
                                .join("package.json");
                            log_info!("推导的 package.json 路径: {:?}", package_path);

                            if let Ok(version) = read_version_from_package_json(&package_path) {
                                log_info!("从 where 命令找到的路径成功获取版本: {}", version);
                                return Ok(version);
                            }
                        }
                    }
                }
            }
        }
    }

    // 方法3: 遍历常见安装位置
    log_info!("尝试遍历常见安装位置");
    let localappdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
    log_info!("LOCALAPPDATA: {}", localappdata);

    let mut cursor_paths = vec![
        PathBuf::from(format!(
            "{}\\Programs\\Cursor\\resources\\app\\package.json",
            localappdata
        )),
        PathBuf::from(format!(
            "{}\\Programs\\cursor\\resources\\app\\package.json",
            localappdata
        )),
        PathBuf::from(format!(
            "{}\\Cursor\\resources\\app\\package.json",
            localappdata
        )),
        PathBuf::from("C:\\Program Files\\Cursor\\resources\\app\\package.json"),
        PathBuf::from("C:\\Program Files (x86)\\Cursor\\resources\\app\\package.json"),
    ];

    // 尝试所有常见驱动器盘符的根目录和常见子目录
    for drive in &["C", "D", "E", "F", "G"] {
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Program Files\\Cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Program Files (x86)\\Cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Software\\Cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Apps\\Cursor\\resources\\app\\package.json",
            drive
        )));
        cursor_paths.push(PathBuf::from(format!(
            "{}:\\Programs\\Cursor\\resources\\app\\package.json",
            drive
        )));
    }

    log_info!("开始搜索 Cursor 版本信息，共 {} 个路径", cursor_paths.len());

    for (index, path) in cursor_paths.iter().enumerate() {
        log_debug!("尝试路径 {}: {:?}", index + 1, path);

        if let Ok(version) = read_version_from_package_json(path) {
            log_info!("从路径 {} 成功获取版本: {}", index + 1, version);
            return Ok(version);
        }
    }

    log_error!("Windows: 所有路径都未找到有效的版本信息");
    Err("无法获取 Cursor 版本信息".to_string())
}

#[cfg(target_os = "linux")]
fn get_cursor_version_linux() -> Result<String, String> {
    log_info!("Linux: 开始获取 Cursor 版本信息");

    let home = std::env::var("HOME").unwrap_or_default();
    log_info!("HOME: {}", home);

    let cursor_paths = vec![
        PathBuf::from(format!(
            "{}/.local/share/cursor/resources/app/package.json",
            home
        )),
        PathBuf::from("/opt/Cursor/resources/app/package.json"),
        PathBuf::from("/usr/share/cursor/resources/app/package.json"),
    ];

    log_info!("开始搜索 Cursor 版本信息，共 {} 个路径", cursor_paths.len());

    for (index, path) in cursor_paths.iter().enumerate() {
        log_debug!("尝试路径 {}: {:?}", index + 1, path);

        if let Ok(version) = read_version_from_package_json(path) {
            log_info!("从路径 {} 成功获取版本: {}", index + 1, version);
            return Ok(version);
        }
    }

    log_error!("Linux: 所有路径都未找到有效的版本信息");
    Err("无法获取 Cursor 版本信息".to_string())
}

/// 从 package.json 文件读取版本信息
fn read_version_from_package_json(package_path: &PathBuf) -> Result<String, String> {
    if !package_path.exists() {
        return Err(format!("文件不存在: {:?}", package_path));
    }

    let content = fs::read_to_string(package_path).map_err(|e| format!("读取文件失败: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("JSON 解析失败: {}", e))?;

    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "JSON 中没有找到 version 字段".to_string())?;

    Ok(version.to_string())
}

/// 尝试从 Cursor 路径（应用目录）读取版本
/// 路径可能是类似 "D:/cursor" 这样的应用根目录
#[cfg(target_os = "windows")]
fn try_read_version_from_cursor_path(cursor_path: &str) -> Result<String, String> {
    let base_path = PathBuf::from(cursor_path);

    // 尝试多种可能的 package.json 位置
    let possible_paths = vec![
        base_path.join("package.json"),
        base_path.join("resources").join("app").join("package.json"),
        base_path.join("app").join("package.json"),
    ];

    for path in possible_paths {
        log_debug!("尝试自定义路径下的 package.json: {:?}", path);
        if let Ok(version) = read_version_from_package_json(&path) {
            return Ok(version);
        }
    }

    Err(format!("无法从自定义路径 {} 读取版本", cursor_path))
}
