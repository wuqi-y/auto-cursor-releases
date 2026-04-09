use crate::{account_manager::AccountManager, get_app_dir, log_info};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Window};
use tokio::time::{Duration, sleep};

#[derive(Debug, Serialize, Deserialize)]
pub struct CursorBackupInfo {
    pub cursor_settings: CursorPathInfo,
    pub workspace_storage: CursorPathInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CursorPathInfo {
    pub exists: bool,
    pub path: String,
    pub size: Option<u64>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "itemCount")]
    pub item_count: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupListItem {
    pub name: String,
    pub created_at: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub backup_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupProgress {
    pub total: usize,
    pub current: usize,
    pub status: String,
    pub percentage: f64,
}

// 全局取消状态管理
static BACKUP_CANCEL_FLAGS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// 存储当前备份的文件路径，用于取消时清理
static CURRENT_BACKUP_FILES: Lazy<Mutex<HashMap<String, PathBuf>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 获取不同系统的 Cursor 目录路径 (备份功能专用)
pub fn get_cursor_backup_paths() -> Result<(PathBuf, PathBuf), String> {
    let home_dir = dirs::home_dir().ok_or("无法获取用户主目录")?;

    let (settings_path, workspace_path) = if cfg!(target_os = "windows") {
        // Windows: %APPDATA%\Cursor\User\
        let appdata = std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA 环境变量")?;
        let cursor_user = PathBuf::from(appdata).join("Cursor").join("User");
        (
            cursor_user.join("settings.json"),
            cursor_user.join("workspaceStorage"),
        )
    } else if cfg!(target_os = "macos") {
        // macOS: ~/Library/Application Support/Cursor/User/
        let cursor_user = home_dir
            .join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User");
        (
            cursor_user.join("settings.json"),
            cursor_user.join("workspaceStorage"),
        )
    } else {
        // Linux: ~/.config/Cursor/User/
        let cursor_user = home_dir.join(".config").join("Cursor").join("User");
        (
            cursor_user.join("settings.json"),
            cursor_user.join("workspaceStorage"),
        )
    };

    Ok((settings_path, workspace_path))
}

/// 获取应用备份目录路径
pub fn get_backup_dir() -> Result<PathBuf, String> {
    let app_dir = get_app_dir()?;
    let backup_dir = app_dir.join("backups");

    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir).map_err(|e| {
            if is_permission_error(&e) {
                format!(
                    "权限不足，无法创建备份目录: {:?}。请以管理员权限运行应用。错误: {}",
                    backup_dir, e
                )
            } else {
                format!("创建备份目录失败: {}", e)
            }
        })?;
    }

    Ok(backup_dir)
}

/// 计算目录大小和文件数量
pub fn calculate_dir_info(path: &PathBuf) -> Result<(u64, usize), String> {
    if !path.exists() {
        return Ok((0, 0));
    }

    let mut total_size = 0u64;
    let mut count = 0usize;

    fn visit_dir(dir: &PathBuf, size: &mut u64, count: &mut usize) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| format!("读取目录失败: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                *count += 1;
                visit_dir(&path, size, count)?;
            } else if path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    *size += metadata.len();
                }
            }
        }

        Ok(())
    }

    visit_dir(path, &mut total_size, &mut count)?;
    Ok((total_size, count))
}

/// 获取 Cursor 备份信息
#[tauri::command]
pub async fn get_cursor_backup_info() -> Result<CursorBackupInfo, String> {
    log_info!("🔍 获取 Cursor 备份信息...");

    let (settings_path, workspace_path) = get_cursor_backup_paths()?;

    // 检查设置文件
    let settings_info = if settings_path.exists() {
        let metadata =
            fs::metadata(&settings_path).map_err(|e| format!("获取设置文件信息失败: {}", e))?;

        CursorPathInfo {
            exists: true,
            path: settings_path.to_string_lossy().to_string(),
            size: Some(metadata.len()),
            last_modified: metadata
                .modified()
                .ok()
                .map(|time| DateTime::<Utc>::from(time).to_rfc3339()),
            item_count: None,
        }
    } else {
        CursorPathInfo {
            exists: false,
            path: settings_path.to_string_lossy().to_string(),
            size: None,
            last_modified: None,
            item_count: None,
        }
    };

    // 检查工作区存储
    let workspace_info = if workspace_path.exists() && workspace_path.is_dir() {
        let (size, count) = calculate_dir_info(&workspace_path)?;
        let metadata =
            fs::metadata(&workspace_path).map_err(|e| format!("获取工作区信息失败: {}", e))?;

        CursorPathInfo {
            exists: true,
            path: workspace_path.to_string_lossy().to_string(),
            size: Some(size),
            last_modified: metadata
                .modified()
                .ok()
                .map(|time| DateTime::<Utc>::from(time).to_rfc3339()),
            item_count: Some(count),
        }
    } else {
        CursorPathInfo {
            exists: false,
            path: workspace_path.to_string_lossy().to_string(),
            size: None,
            last_modified: None,
            item_count: None,
        }
    };

    let backup_info = CursorBackupInfo {
        cursor_settings: settings_info,
        workspace_storage: workspace_info,
    };

    log_info!("✅ 获取 Cursor 备份信息完成");
    Ok(backup_info)
}

/// 备份 Cursor 数据  
#[tauri::command]
pub async fn backup_cursor_data(backup_type: String, window: Window) -> Result<String, String> {
    log_info!("🔄 开始备份 Cursor 数据，类型: {}", backup_type);

    // 生成备份任务 ID
    let backup_id = uuid::Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // 注册取消标志
    {
        let mut flags = BACKUP_CANCEL_FLAGS.lock().unwrap();
        flags.insert(backup_id.clone(), cancel_flag.clone());
    }

    // 发送备份开始事件，包含备份 ID
    if let Err(e) = window.emit("backup-started", &backup_id) {
        log_info!("⚠️ 发送备份开始事件失败: {}", e);
    }

    // 发送初始进度
    send_progress(&window, 0, 100, "准备备份...").await;

    let (settings_path, workspace_path) = get_cursor_backup_paths()?;
    let backup_dir = get_backup_dir()?;

    // 检查是否取消
    if cancel_flag.load(Ordering::Relaxed) {
        cleanup_backup_task(&backup_id);
        return Err("备份已取消".to_string());
    }

    // 发送进度：目录检查完成
    send_progress(&window, 10, 100, "检查 Cursor 目录...").await;

    // 生成备份文件名
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!("cursor_backup_{}_{}", backup_type, timestamp);
    let backup_file = backup_dir.join(format!("{}.tar.gz", backup_name));

    // 存储当前备份文件路径
    {
        let mut files = CURRENT_BACKUP_FILES.lock().unwrap();
        files.insert(backup_id.clone(), backup_file.clone());
    }

    log_info!("📦 创建备份文件: {:?}", backup_file);

    // 再次检查是否取消
    if cancel_flag.load(Ordering::Relaxed) {
        cleanup_backup_task(&backup_id);
        return Err("备份已取消".to_string());
    }

    // 发送进度：创建备份文件
    send_progress(&window, 20, 100, "创建备份文件...").await;

    // 创建压缩文件
    let tar_gz = fs::File::create(&backup_file)
        .map_err(|e| {
            if is_permission_error(&e) {
                format!("权限不足，无法创建备份文件: {:?}。请以管理员权限运行应用或检查备份目录权限。错误: {}", backup_file, e)
            } else {
                format!("创建备份文件失败: {}", e)
            }
        })?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    match backup_type.as_str() {
        "full" => {
            // 完整备份：设置 + 工作区
            if settings_path.exists() {
                send_progress(&window, 30, 100, "备份设置文件...").await;
                log_info!("📄 添加设置文件: {:?}", settings_path);
                tar.append_path_with_name(&settings_path, "settings.json")
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.to_lowercase().contains("permission") || 
                           error_str.to_lowercase().contains("access") ||
                           error_str.to_lowercase().contains("denied") {
                            format!("权限不足，无法备份设置文件。请以管理员权限运行应用或检查 Cursor 目录权限。错误: {}", e)
                        } else {
                            format!("添加设置文件到备份失败: {}", e)
                        }
                    })?;
                send_progress(&window, 40, 100, "设置文件备份完成").await;
            }
            if workspace_path.exists() {
                send_progress(&window, 50, 100, "开始备份对话记录...").await;
                log_info!("📁 添加工作区目录: {:?}", workspace_path);
                append_workspace_safely(&mut tar, &workspace_path, &window, 50, 90, &cancel_flag)
                    .await
                    .map_err(|e| {
                        cleanup_backup_task(&backup_id);
                        format!("添加工作区到备份失败: {}", e)
                    })?;
            }
        }
        "settings" => {
            // 仅备份设置
            if settings_path.exists() {
                send_progress(&window, 50, 100, "备份设置文件...").await;
                log_info!("📄 添加设置文件: {:?}", settings_path);
                tar.append_path_with_name(&settings_path, "settings.json")
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.to_lowercase().contains("permission") || 
                           error_str.to_lowercase().contains("access") ||
                           error_str.to_lowercase().contains("denied") {
                            format!("权限不足，无法备份设置文件。请以管理员权限运行应用或检查 Cursor 目录权限。错误: {}", e)
                        } else {
                            format!("添加设置文件到备份失败: {}", e)
                        }
                    })?;
                send_progress(&window, 80, 100, "设置文件备份完成").await;
            } else {
                return Err("Cursor 设置文件不存在".to_string());
            }
        }
        "workspace" => {
            // 仅备份工作区
            if workspace_path.exists() {
                send_progress(&window, 30, 100, "开始备份对话记录...").await;
                log_info!("📁 添加工作区目录: {:?}", workspace_path);
                append_workspace_safely(&mut tar, &workspace_path, &window, 30, 80, &cancel_flag)
                    .await
                    .map_err(|e| {
                        cleanup_backup_task(&backup_id);
                        format!("添加工作区到备份失败: {}", e)
                    })?;
            } else {
                return Err("Cursor 工作区存储不存在".to_string());
            }
        }
        _ => {
            return Err(format!("不支持的备份类型: {}", backup_type));
        }
    }

    // 最后检查是否取消
    if cancel_flag.load(Ordering::Relaxed) {
        cleanup_backup_task(&backup_id);
        return Err("备份已取消".to_string());
    }

    send_progress(&window, 95, 100, "正在完成备份...").await;
    tar.finish().map_err(|e| {
        cleanup_backup_task(&backup_id);
        format!("完成备份失败: {}", e)
    })?;

    // 备份成功完成，清理任务状态但保留备份文件
    cleanup_backup_task_success(&backup_id);

    send_progress(&window, 100, 100, "备份完成！").await;
    log_info!("✅ 备份完成: {:?}", backup_file);
    Ok(backup_name)
}

/// 恢复 Cursor 数据
#[tauri::command]
pub async fn restore_cursor_data(backup_name: String) -> Result<String, String> {
    log_info!("🔄 开始恢复 Cursor 数据: {}", backup_name);

    let backup_dir = get_backup_dir()?;
    let backup_file = backup_dir.join(format!("{}.tar.gz", backup_name));

    if !backup_file.exists() {
        return Err("备份文件不存在".to_string());
    }

    // 解析备份类型
    let backup_type = if backup_name.contains("_full_") {
        "full"
    } else if backup_name.contains("_settings_") {
        "settings"
    } else if backup_name.contains("_workspace_") {
        "workspace"
    } else {
        return Err("无法识别备份类型".to_string());
    };

    log_info!("📋 备份类型: {}", backup_type);

    let (settings_path, workspace_path) = get_cursor_backup_paths()?;
    let cursor_user_dir = settings_path.parent().ok_or("无法获取 Cursor 用户目录")?;

    // 检查 Cursor 是否正在运行，如果是则先关闭
    let cursor_was_running = AccountManager::is_cursor_running();
    if cursor_was_running {
        log_info!("🔍 检测到 Cursor 正在运行，需要先关闭以安全恢复数据");

        // 1. 先尝试优雅关闭 Cursor
        log_info!("🔄 尝试优雅关闭 Cursor...");
        match AccountManager::force_close_cursor() {
            Ok(()) => {
                log_info!("✅ 成功优雅关闭 Cursor");
            }
            Err(e) => {
                log_info!("⚠️ 优雅关闭失败: {}，将进行强制关闭", e);
            }
        }

        // 2. 等待优雅关闭完成
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // 3. 强制杀死任何剩余的 Cursor 进程
        log_info!("🔫 强制杀死任何剩余的 Cursor 进程...");
        match AccountManager::force_kill_cursor_processes() {
            Ok(killed_count) => {
                if killed_count > 0 {
                    log_info!("✅ 强制杀死了 {} 个 Cursor 进程", killed_count);
                } else {
                    log_info!("✅ 没有发现需要强制杀死的 Cursor 进程");
                }
            }
            Err(e) => {
                log_info!("⚠️ 强制杀死进程时出错: {}", e);
            }
        }

        // 4. 等待进程完全终止
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    log_info!("📦 开始恢复数据文件...");

    // 创建临时目录用于解压
    let temp_dir = cursor_user_dir.join("restore_temp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|e| format!("删除临时目录失败: {}", e))?;
    }
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

    // 解压备份文件到临时目录
    let tar_gz = fs::File::open(&backup_file).map_err(|e| format!("打开备份文件失败: {}", e))?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);

    log_info!("📂 解压备份文件到临时目录...");
    archive
        .unpack(&temp_dir)
        .map_err(|e| format!("解压备份文件失败: {}", e))?;

    // 根据备份类型备份当前数据（如果存在）
    log_info!("💾 备份当前数据...");

    if (backup_type == "full" || backup_type == "settings") && settings_path.exists() {
        let back_path = settings_path.with_extension("json.back");
        if back_path.exists() {
            fs::remove_file(&back_path).map_err(|e| format!("删除旧的设置备份失败: {}", e))?;
        }
        fs::rename(&settings_path, &back_path).map_err(|e| format!("备份当前设置失败: {}", e))?;
        log_info!("📄 已备份设置文件: {:?} -> {:?}", settings_path, back_path);
    }

    if (backup_type == "full" || backup_type == "workspace") && workspace_path.exists() {
        let back_path = workspace_path.with_extension("back");
        if back_path.exists() {
            fs::remove_dir_all(&back_path).map_err(|e| format!("删除旧的工作区备份失败: {}", e))?;
        }
        fs::rename(&workspace_path, &back_path)
            .map_err(|e| format!("备份当前工作区失败: {}", e))?;
        log_info!(
            "📁 已备份工作区目录: {:?} -> {:?}",
            workspace_path,
            back_path
        );
    }

    // 根据备份类型移动解压后的文件到正确位置
    log_info!("🔄 移动恢复的数据到目标位置...");

    // 移动设置文件（仅在完整备份或设置备份时）
    if backup_type == "full" || backup_type == "settings" {
        let temp_settings = temp_dir.join("settings.json");
        if temp_settings.exists() {
            fs::rename(&temp_settings, &settings_path)
                .map_err(|e| format!("移动设置文件失败: {}", e))?;
            log_info!("📄 已恢复设置文件: {:?}", settings_path);
        } else if backup_type == "settings" {
            return Err("备份文件中未找到设置文件".to_string());
        }
    } else {
        log_info!("⏭️  跳过设置文件恢复（非设置备份）");
    }

    // 移动工作区目录（仅在完整备份或工作区备份时）
    if backup_type == "full" || backup_type == "workspace" {
        let temp_workspace = temp_dir.join("workspaceStorage");
        if temp_workspace.exists() {
            fs::rename(&temp_workspace, &workspace_path)
                .map_err(|e| format!("移动工作区目录失败: {}", e))?;
            log_info!("📁 已恢复工作区目录: {:?}", workspace_path);
        } else if backup_type == "workspace" {
            return Err("备份文件中未找到工作区目录".to_string());
        }
    } else {
        log_info!("⏭️  跳过工作区恢复（非工作区备份）");
    }

    // 清理临时目录
    if temp_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&temp_dir) {
            log_info!("⚠️ 清理临时目录失败: {}", e);
        } else {
            log_info!("🧹 已清理临时目录");
        }
    }

    // 如果 Cursor 之前在运行，现在重启它
    if cursor_was_running {
        log_info!("⏳ 等待数据更新完成...");
        tokio::time::sleep(Duration::from_millis(1000)).await;

        log_info!("🚀 重启 Cursor...");
        match AccountManager::start_cursor() {
            Ok(()) => {
                log_info!("✅ 成功重启 Cursor");
            }
            Err(e) => {
                log_info!("⚠️ 重启 Cursor 失败: {}，请手动启动 Cursor", e);
            }
        }
    } else {
        log_info!("ℹ️ Cursor 之前未在运行，无需重启");
    }

    log_info!("✅ 恢复完成");

    let restore_content = match backup_type {
        "full" => "完整数据（设置和对话记录）",
        "settings" => "设置文件",
        "workspace" => "对话记录",
        _ => "数据",
    };

    let success_msg = if cursor_was_running {
        format!("恢复{}成功，Cursor 已自动重启", restore_content)
    } else {
        format!("恢复{}成功，请启动 Cursor 使配置生效", restore_content)
    };
    Ok(success_msg)
}

/// 获取备份列表
#[tauri::command]
pub async fn get_backup_list() -> Result<Vec<BackupListItem>, String> {
    log_info!("🔍 获取备份列表...");

    let backup_dir = get_backup_dir()?;
    let mut backups = Vec::new();

    if backup_dir.exists() {
        let entries = fs::read_dir(&backup_dir).map_err(|e| format!("读取备份目录失败: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("读取备份目录项失败: {}", e))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("gz") {
                if let Some(filename) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_suffix(".tar"))
                {
                    if filename.starts_with("cursor_backup_") {
                        let metadata = fs::metadata(&path)
                            .map_err(|e| format!("获取备份文件信息失败: {}", e))?;

                        let created = metadata
                            .created()
                            .or_else(|_| metadata.modified())
                            .unwrap_or_else(|_| std::time::SystemTime::now());

                        let backup_type = if filename.contains("_full_") {
                            "full".to_string()
                        } else if filename.contains("_settings_") {
                            "settings".to_string()
                        } else if filename.contains("_workspace_") {
                            "workspace".to_string()
                        } else {
                            "unknown".to_string()
                        };

                        backups.push(BackupListItem {
                            name: filename.to_string(),
                            created_at: DateTime::<Utc>::from(created).to_rfc3339(),
                            size: metadata.len(),
                            backup_type,
                        });
                    }
                }
            }
        }
    }

    // 按创建时间排序（最新的在前）
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    log_info!("✅ 获取备份列表完成，共 {} 个备份", backups.len());
    Ok(backups)
}

/// 取消备份命令
#[tauri::command]
pub async fn cancel_backup(backup_id: String) -> Result<(), String> {
    log_info!("🛑 取消备份请求: {}", backup_id);

    // 设置取消标志
    {
        let flags = BACKUP_CANCEL_FLAGS.lock().unwrap();
        if let Some(cancel_flag) = flags.get(&backup_id) {
            cancel_flag.store(true, Ordering::Relaxed);
            log_info!("✅ 备份取消标志已设置: {}", backup_id);
        } else {
            log_info!("⚠️ 未找到备份任务: {}", backup_id);
            return Err("未找到备份任务".to_string());
        }
    }

    // 等待一小段时间让备份任务有机会响应
    sleep(Duration::from_millis(100)).await;

    Ok(())
}

/// 打开 Cursor 设置目录
#[tauri::command]
pub async fn open_cursor_settings_dir() -> Result<String, String> {
    log_info!("📂 打开 Cursor 设置目录请求");

    let (settings_path, _) = get_cursor_backup_paths()?;
    let settings_dir = settings_path.parent().ok_or("无法获取设置目录")?;

    if !settings_dir.exists() {
        return Err("Cursor 设置目录不存在".to_string());
    }

    let result = open_directory_in_explorer(settings_dir);
    match result {
        Ok(_) => {
            let success_msg = format!("已打开设置目录: {:?}", settings_dir);
            log_info!("✅ {}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let error_msg = format!("打开设置目录失败: {}", e);
            log_info!("❌ {}", error_msg);
            Err(error_msg)
        }
    }
}

/// 打开 Cursor 工作区目录
#[tauri::command]
pub async fn open_cursor_workspace_dir() -> Result<String, String> {
    log_info!("📂 打开 Cursor 工作区目录请求");

    let (_, workspace_path) = get_cursor_backup_paths()?;

    if !workspace_path.exists() {
        return Err("Cursor 工作区目录不存在".to_string());
    }

    let result = open_directory_in_explorer(&workspace_path);
    match result {
        Ok(_) => {
            let success_msg = format!("已打开工作区目录: {:?}", workspace_path);
            log_info!("✅ {}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let error_msg = format!("打开工作区目录失败: {}", e);
            log_info!("❌ {}", error_msg);
            Err(error_msg)
        }
    }
}

/// 打开备份目录
#[tauri::command]
pub async fn open_backup_dir() -> Result<String, String> {
    log_info!("📂 打开备份目录请求");

    let backup_dir = get_backup_dir()?;

    let result = open_directory_in_explorer(&backup_dir);
    match result {
        Ok(_) => {
            let success_msg = format!("已打开备份目录: {:?}", backup_dir);
            log_info!("✅ {}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let error_msg = format!("打开备份目录失败: {}", e);
            log_info!("❌ {}", error_msg);
            Err(error_msg)
        }
    }
}

/// 在文件管理器中打开任意目录
#[tauri::command]
pub async fn open_directory_by_path(path: String) -> Result<String, String> {
    log_info!("📂 打开目录请求: {}", path);

    let dir_path = std::path::Path::new(&path);
    if !dir_path.exists() {
        return Err(format!("目录不存在: {:?}", dir_path));
    }

    let result = open_directory_in_explorer(dir_path);
    match result {
        Ok(_) => Ok(format!("已打开目录: {:?}", dir_path)),
        Err(e) => Err(format!("打开目录失败: {}", e)),
    }
}

/// 在文件管理器中打开目录
fn open_directory_in_explorer(path: &std::path::Path) -> Result<(), String> {
    use std::process::Command;

    let result = if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else {
        // Linux
        Command::new("xdg-open").arg(path).spawn()
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("无法打开目录: {}", e)),
    }
}

/// 删除备份文件
#[tauri::command]
pub async fn delete_cursor_backup(backup_name: String) -> Result<String, String> {
    log_info!("🗑️ 删除备份请求: {}", backup_name);

    let backup_dir = get_backup_dir()?;
    let backup_file = backup_dir.join(format!("{}.tar.gz", backup_name));

    // 检查备份文件是否存在
    if !backup_file.exists() {
        let error_msg = format!("备份文件不存在: {:?}", backup_file);
        log_info!("❌ {}", error_msg);
        return Err(error_msg);
    }

    // 删除备份文件
    match fs::remove_file(&backup_file) {
        Ok(_) => {
            let success_msg = format!("备份文件已删除: {}", backup_name);
            log_info!("✅ {}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            if is_permission_error(&e) {
                let error_msg = format!(
                    "权限不足，无法删除备份文件: {}。请以管理员权限运行应用。错误: {}",
                    backup_name, e
                );
                log_info!("🚫 {}", error_msg);
                Err(error_msg)
            } else {
                let error_msg = format!("删除备份文件失败: {}。错误: {}", backup_name, e);
                log_info!("❌ {}", error_msg);
                Err(error_msg)
            }
        }
    }
}

/// 安全地添加 workspace 目录到 tar 归档
async fn append_workspace_safely<W: std::io::Write>(
    tar: &mut tar::Builder<W>,
    workspace_path: &PathBuf,
    window: &Window,
    start_progress: usize,
    end_progress: usize,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), String> {
    log_info!("🔍 开始扫描工作区目录: {:?}", workspace_path);

    // 首先计算总文件数
    let total_files = count_files_in_dir(workspace_path)?;
    log_info!("📊 工作区总文件数: {}", total_files);

    let mut processed_files = 0usize;

    // 使用迭代方式而非递归来避免 async 递归问题
    // 修复：直接遍历 workspace_path 下的内容，构建正确的归档路径
    let mut dirs_to_process = vec![workspace_path.clone()];

    while let Some(current_dir) = dirs_to_process.pop() {
        // 检查是否取消
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("备份已取消".to_string());
        }
        let entries = match fs::read_dir(&current_dir) {
            Ok(entries) => entries,
            Err(e) => {
                // 检查是否为权限问题
                if is_permission_error(&e) {
                    let error_msg = format!(
                        "权限不足，无法读取目录: {:?}。请以管理员权限运行应用或检查 Cursor 目录权限。",
                        current_dir
                    );
                    log_info!("🚫 {}", error_msg);
                    return Err(error_msg);
                } else {
                    // 其他错误记录警告但继续
                    log_info!("⚠️ 跳过无法读取的目录: {:?} ({})", current_dir, e);
                    continue;
                }
            }
        };

        for entry in entries {
            // 在处理每个文件时检查取消状态
            if cancel_flag.load(Ordering::Relaxed) {
                return Err("备份已取消".to_string());
            }

            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    log_info!("⚠️ 跳过无法读取的条目: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            let relative_path = match path.strip_prefix(workspace_path) {
                Ok(rel) => rel,
                Err(_) => continue,
            };

            // 修复：构建正确的归档路径，格式为 workspaceStorage/相对路径
            let archive_path = format!("workspaceStorage/{}", relative_path.to_string_lossy());

            if path.is_dir() {
                // 添加目录条目
                match tar.append_dir(&archive_path, &path) {
                    Ok(_) => {
                        log_info!("📁 添加目录: {}", archive_path);
                        // 将子目录添加到待处理队列
                        dirs_to_process.push(path);
                    }
                    Err(e) => {
                        log_info!("⚠️ 跳过目录: {} ({})", archive_path, e);
                    }
                }
            } else if path.is_file() {
                // 添加文件
                match fs::File::open(&path) {
                    Ok(mut file) => {
                        let metadata = match file.metadata() {
                            Ok(meta) => meta,
                            Err(e) => {
                                if is_permission_error(&e) {
                                    let error_msg = format!(
                                        "权限不足，无法读取文件: {}。请以管理员权限运行应用。",
                                        archive_path
                                    );
                                    log_info!("🚫 {}", error_msg);
                                    return Err(error_msg);
                                } else {
                                    log_info!(
                                        "⚠️ 跳过文件（无法获取元数据）: {} ({})",
                                        archive_path,
                                        e
                                    );
                                    continue;
                                }
                            }
                        };

                        let mut header = tar::Header::new_gnu();
                        header.set_metadata(&metadata);
                        header.set_size(metadata.len());
                        header.set_cksum();

                        match tar.append_data(&mut header, &archive_path, &mut file) {
                            Ok(_) => {
                                log_info!("📄 添加文件: {} ({}B)", archive_path, metadata.len());
                                processed_files += 1;

                                // 更新进度
                                if total_files > 0 {
                                    let file_progress = (processed_files as f64
                                        / total_files as f64)
                                        * (end_progress - start_progress) as f64;
                                    let current_progress = start_progress as f64 + file_progress;
                                    let status = format!(
                                        "备份文件: {} ({}/{})",
                                        archive_path.split('/').last().unwrap_or(""),
                                        processed_files,
                                        total_files
                                    );
                                    send_progress(window, current_progress as usize, 100, &status)
                                        .await;
                                }
                            }
                            Err(e) => {
                                log_info!("⚠️ 跳过文件: {} ({})", archive_path, e);
                            }
                        }
                    }
                    Err(e) => {
                        if is_permission_error(&e) {
                            let error_msg = format!(
                                "权限不足，无法打开文件: {}。请以管理员权限运行应用。",
                                archive_path
                            );
                            log_info!("🚫 {}", error_msg);
                            return Err(error_msg);
                        } else {
                            log_info!("⚠️ 跳过文件（无法打开）: {} ({})", archive_path, e);
                        }
                    }
                }
            }
        }
    }

    log_info!("✅ 工作区目录添加完成");
    Ok(())
}

/// 检查是否为权限错误
fn is_permission_error(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;

    match error.kind() {
        ErrorKind::PermissionDenied => true,
        _ => {
            // 检查错误消息中是否包含权限相关的关键词
            let error_msg = error.to_string().to_lowercase();
            error_msg.contains("permission")
                || error_msg.contains("access")
                || error_msg.contains("denied")
                || error_msg.contains("权限")
                || error_msg.contains("访问")
        }
    }
}

/// 计算目录中的文件总数
fn count_files_in_dir(dir_path: &PathBuf) -> Result<usize, String> {
    let mut count = 0usize;

    fn count_recursive(dir: &PathBuf, count: &mut usize) -> Result<(), String> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()), // 跳过无法读取的目录
        };

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    *count += 1;
                } else if path.is_dir() {
                    count_recursive(&path, count)?;
                }
            }
        }
        Ok(())
    }

    count_recursive(dir_path, &mut count)?;
    Ok(count)
}

/// 发送进度事件到前端
async fn send_progress(window: &Window, current: usize, total: usize, status: &str) {
    let progress = BackupProgress {
        current,
        total,
        status: status.to_string(),
        percentage: if total > 0 {
            (current as f64 / total as f64) * 100.0
        } else {
            0.0
        },
    };

    if let Err(e) = window.emit("backup-progress", &progress) {
        log_info!("⚠️ 发送进度事件失败: {}", e);
    }
}

/// 清理备份任务状态并删除未完成的备份文件
fn cleanup_backup_task(backup_id: &str) {
    log_info!("🧹 清理备份任务: {}", backup_id);

    // 删除未完成的备份文件
    {
        let mut files = CURRENT_BACKUP_FILES.lock().unwrap();
        if let Some(backup_file) = files.remove(backup_id) {
            if backup_file.exists() {
                if let Err(e) = fs::remove_file(&backup_file) {
                    log_info!("⚠️ 删除未完成的备份文件失败: {:?} ({})", backup_file, e);
                } else {
                    log_info!("🗑️ 已删除未完成的备份文件: {:?}", backup_file);
                }
            }
        }
    }

    // 清理取消标志
    {
        let mut flags = BACKUP_CANCEL_FLAGS.lock().unwrap();
        flags.remove(backup_id);
    }
}

/// 清理备份任务状态但保留备份文件（成功完成时调用）
fn cleanup_backup_task_success(backup_id: &str) {
    log_info!("✅ 备份任务成功完成，清理状态: {}", backup_id);

    // 只清理状态，不删除备份文件
    {
        let mut files = CURRENT_BACKUP_FILES.lock().unwrap();
        files.remove(backup_id);
    }

    {
        let mut flags = BACKUP_CANCEL_FLAGS.lock().unwrap();
        flags.remove(backup_id);
    }
}

// ==================== 工作区详情相关功能 ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceStorageItem {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "workspaceInfo")]
    pub workspace_info: Option<WorkspaceInfo>,
    #[serde(rename = "conversationCount")]
    pub conversation_count: usize,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub folder: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationData {
    pub id: String,
    pub title: String,
    #[serde(rename = "lastMessage")]
    pub last_message: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub timestamp: String,
    pub sender: String, // "user" 或 "assistant"
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationDetail {
    pub id: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceDetails {
    #[serde(rename = "workspaceInfo")]
    pub workspace_info: WorkspaceInfo,
    pub conversations: Vec<ConversationData>,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
}

/// 获取工作区存储项目列表
#[tauri::command]
pub async fn get_workspace_storage_items() -> Result<Vec<WorkspaceStorageItem>, String> {
    log_info!("🔍 获取工作区存储项目列表...");

    let (_, workspace_path) = get_cursor_backup_paths()?;

    if !workspace_path.exists() {
        log_info!("⚠️ 工作区目录不存在: {:?}", workspace_path);
        return Ok(vec![]);
    }

    let mut items = Vec::new();

    let entries =
        fs::read_dir(&workspace_path).map_err(|e| format!("读取工作区目录失败: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录条目失败: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let workspace_id = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();

            // 读取workspace.json文件
            let workspace_json_path = path.join("workspace.json");
            let workspace_info = if workspace_json_path.exists() {
                match fs::read_to_string(&workspace_json_path) {
                    Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(json) => {
                            if let Some(folder) = json.get("folder").and_then(|f| f.as_str()) {
                                Some(WorkspaceInfo {
                                    folder: folder.to_string(),
                                })
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            } else {
                None
            };

            // 快速统计对话数量（先返回简单统计，避免阻塞）
            let conversation_count = quick_count_conversations(&path).unwrap_or(0);

            // 获取目录基本信息（快速版本，避免递归扫描）
            let (size, last_modified, created_at) = get_dir_info_quick(&path).unwrap_or((
                0,
                "未知时间".to_string(),
                "未知时间".to_string(),
            ));

            let display_name = if let Some(ref info) = workspace_info {
                info.folder
                    .split('/')
                    .last()
                    .unwrap_or(&workspace_id)
                    .to_string()
            } else {
                workspace_id.clone()
            };

            items.push(WorkspaceStorageItem {
                id: workspace_id,
                name: display_name,
                path: path.to_string_lossy().to_string(),
                workspace_info,
                conversation_count,
                last_modified,
                created_at,
                size,
            });
        }
    }

    log_info!("✅ 找到 {} 个工作区", items.len());
    Ok(items)
}

/// 获取工作区详情
#[tauri::command]
pub async fn get_workspace_details(workspace_id: String) -> Result<WorkspaceDetails, String> {
    log_info!("🔍 获取工作区详情: {}", workspace_id);

    let (_, workspace_path) = get_cursor_backup_paths()?;
    let workspace_dir = workspace_path.join(&workspace_id);

    if !workspace_dir.exists() {
        return Err(format!("工作区目录不存在: {}", workspace_id));
    }

    // 读取workspace.json
    let workspace_json_path = workspace_dir.join("workspace.json");
    let workspace_info = if workspace_json_path.exists() {
        match fs::read_to_string(&workspace_json_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    if let Some(folder) = json.get("folder").and_then(|f| f.as_str()) {
                        WorkspaceInfo {
                            folder: folder.to_string(),
                        }
                    } else {
                        WorkspaceInfo {
                            folder: "未知项目".to_string(),
                        }
                    }
                }
                Err(e) => {
                    log_info!("⚠️ 解析workspace.json失败: {}", e);
                    WorkspaceInfo {
                        folder: "未知项目".to_string(),
                    }
                }
            },
            Err(e) => {
                log_info!("⚠️ 读取workspace.json失败: {}", e);
                WorkspaceInfo {
                    folder: "未知项目".to_string(),
                }
            }
        }
    } else {
        WorkspaceInfo {
            folder: "未知项目".to_string(),
        }
    };

    // 读取对话数据
    let conversations = get_conversations_from_sqlite(&workspace_dir)?;

    // 计算总大小
    let (total_size, _, _) = get_dir_info(&workspace_dir)?;

    Ok(WorkspaceDetails {
        workspace_info,
        conversations,
        total_size,
    })
}

/// 快速统计对话数量（简单检查，避免复杂解析）
fn quick_count_conversations(workspace_dir: &PathBuf) -> Result<usize, String> {
    let sqlite_path = workspace_dir.join("state.vscdb");

    if !sqlite_path.exists() {
        return Ok(0);
    }

    // 快速获取真实的对话数量，与详情页保持一致
    match Connection::open(&sqlite_path) {
        Ok(conn) => match get_composer_data(&conn) {
            Ok(conversations) => Ok(conversations.len()),
            Err(_) => Ok(0),
        },
        Err(_) => Ok(0),
    }
}

/// 统计对话数量（完整解析，较慢）- 已废弃，使用quick_count_conversations
#[allow(dead_code)]
fn count_conversations(workspace_dir: &PathBuf) -> Result<usize, String> {
    // 直接调用快速统计函数，保持兼容性
    quick_count_conversations(workspace_dir)
}

/// 从SQLite数据库获取对话数据
fn get_conversations_from_sqlite(workspace_dir: &PathBuf) -> Result<Vec<ConversationData>, String> {
    let sqlite_path = workspace_dir.join("state.vscdb");

    if !sqlite_path.exists() {
        log_info!("⚠️ SQLite文件不存在: {:?}", sqlite_path);
        return Ok(vec![]);
    }

    match Connection::open(&sqlite_path) {
        Ok(conn) => {
            log_info!("🔍 开始从SQLite提取基础对话信息（不加载详细内容）...");

            // 只获取基本的对话信息：数量和标题
            let conversations = get_composer_data(&conn)?;

            log_info!("✅ 总共提取到 {} 个对话", conversations.len());
            Ok(conversations)
        }
        Err(e) => {
            log_info!("⚠️ 连接SQLite失败: {}", e);
            Ok(vec![])
        }
    }
}

/// 从AI服务数据构建完整对话历史
/// 构建完整的对话历史（从AI服务数据）- 已废弃，功能过于复杂
#[allow(dead_code)]
fn build_conversations_from_ai_service(conn: &Connection) -> Result<Vec<ConversationData>, String> {
    log_info!("🔍 开始从AI服务构建完整对话历史...");

    // 1. 获取用户输入 (prompts)
    let user_prompts = get_user_prompts(conn)?;
    log_info!("📋 获取到 {} 个用户输入", user_prompts.len());

    // 2. 获取AI回复 (generations)
    let ai_generations = get_ai_generations(conn)?;
    log_info!("📋 获取到 {} 个AI回复", ai_generations.len());

    // 3. 将所有消息按时间排序
    let mut all_messages = Vec::new();

    // 添加用户消息
    for (text, timestamp) in user_prompts {
        all_messages.push((timestamp, "user".to_string(), text));
    }

    // 添加AI消息
    for (text, timestamp) in ai_generations {
        all_messages.push((timestamp, "assistant".to_string(), text));
    }

    // 按时间排序
    all_messages.sort_by(|a, b| a.0.cmp(&b.0));

    log_info!("📋 总消息数: {}", all_messages.len());

    // 4. 构建对话
    if all_messages.is_empty() {
        return Ok(vec![]);
    }

    // 获取第一条和最后一条消息
    let first_message = &all_messages[0];
    let last_message = &all_messages[all_messages.len() - 1];

    // 生成对话标题（使用第一条用户消息）
    let title = all_messages
        .iter()
        .find(|(_, sender, _)| sender == "user")
        .map(|(_, _, text)| safe_truncate_string(text, 30))
        .unwrap_or_else(|| "新对话".to_string());

    // 构建最后一条消息预览
    let last_message_preview = safe_truncate_string(&last_message.2, 50);

    // 创建对话数据
    let conversation = ConversationData {
        id: "conversation_1".to_string(), // 可以用更复杂的ID生成逻辑
        title,
        last_message: last_message_preview,
        created_at: format_timestamp(first_message.0),
        message_count: all_messages.len(),
    };

    log_info!("✅ 构建了1个完整对话，包含{}条消息", all_messages.len());
    log_info!("📋 对话标题: {}", conversation.title);
    log_info!("📋 最后消息: {}", conversation.last_message);

    Ok(vec![conversation])
}

/// 获取用户输入数据
fn get_user_prompts(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    log_info!("🔍 开始深度分析aiService.prompts完整结构...");

    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = 'aiService.prompts'")
        .map_err(|e| format!("准备查询prompts失败: {}", e))?;

    let mut prompts = Vec::new();

    let rows = stmt
        .query_map([], |row| {
            let value_str: String = row.get(0)?;
            Ok(value_str)
        })
        .map_err(|e| format!("查询prompts执行失败: {}", e))?;

    for row_result in rows {
        if let Ok(value_str) = row_result {
            log_info!("📄 aiService.prompts内容长度: {} 字节", value_str.len());

            match serde_json::from_str::<serde_json::Value>(&value_str) {
                Ok(json_value) => {
                    if let Some(prompts_array) = json_value.as_array() {
                        log_info!("📋 找到 {} 个prompt记录", prompts_array.len());

                        // 深度分析前3个prompt对象的完整结构
                        for (index, prompt) in prompts_array.iter().take(3).enumerate() {
                            log_info!("🔍 Prompt [{}] 完整结构深度分析:", index);

                            // 打印完整的JSON结构
                            let json_str = serde_json::to_string_pretty(prompt).unwrap_or_default();
                            if json_str.len() > 2000 {
                                log_info!(
                                    "📄 完整JSON结构 (前2000字符): {}",
                                    safe_truncate_string(&json_str, 2000)
                                );
                            } else {
                                log_info!("📄 完整JSON结构: {}", json_str);
                            }

                            if let Some(prompt_obj) = prompt.as_object() {
                                // 深度分析每个字段
                                for (key, value) in prompt_obj {
                                    match value {
                                        serde_json::Value::String(s) => {
                                            if s.len() > 100 {
                                                log_info!(
                                                    "  {}: \"{}...\" ({}字符)",
                                                    key,
                                                    safe_truncate_string(s, 50),
                                                    s.chars().count()
                                                );
                                            } else {
                                                log_info!("  {}: \"{}\"", key, s);
                                            }
                                        }
                                        serde_json::Value::Object(obj) => {
                                            log_info!("  {}: [Object 有{}个字段]", key, obj.len());
                                            // 分析嵌套对象
                                            for (sub_key, sub_value) in obj {
                                                match sub_value {
                                                    serde_json::Value::String(s) => {
                                                        if s.len() > 100 {
                                                            log_info!(
                                                                "    {}.{}: \"{}...\" ({}字符)",
                                                                key,
                                                                sub_key,
                                                                safe_truncate_string(s, 50),
                                                                s.chars().count()
                                                            );
                                                        } else {
                                                            log_info!(
                                                                "    {}.{}: \"{}\"",
                                                                key,
                                                                sub_key,
                                                                s
                                                            );
                                                        }
                                                    }
                                                    serde_json::Value::Object(nested_obj) => {
                                                        log_info!(
                                                            "    {}.{}: [Object 有{}个字段]",
                                                            key,
                                                            sub_key,
                                                            nested_obj.len()
                                                        );
                                                        // 分析三层嵌套
                                                        for (nested_key, nested_value) in nested_obj
                                                        {
                                                            match nested_value {
                                                                serde_json::Value::String(s) => {
                                                                    if s.len() > 100 {
                                                                        log_info!(
                                                                            "      {}.{}.{}: \"{}...\" ({}字符)",
                                                                            key,
                                                                            sub_key,
                                                                            nested_key,
                                                                            safe_truncate_string(
                                                                                s, 50
                                                                            ),
                                                                            s.chars().count()
                                                                        );
                                                                    } else {
                                                                        log_info!(
                                                                            "      {}.{}.{}: \"{}\"",
                                                                            key,
                                                                            sub_key,
                                                                            nested_key,
                                                                            s
                                                                        );
                                                                    }
                                                                }
                                                                _ => {
                                                                    log_info!(
                                                                        "      {}.{}.{}: {}",
                                                                        key,
                                                                        sub_key,
                                                                        nested_key,
                                                                        nested_value
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                    serde_json::Value::Array(arr) => {
                                                        log_info!(
                                                            "    {}.{}: [Array 有{}个元素]",
                                                            key,
                                                            sub_key,
                                                            arr.len()
                                                        );
                                                        if arr.len() > 0 {
                                                            log_info!(
                                                                "      第一个元素: {}",
                                                                safe_truncate_string(
                                                                    &arr[0].to_string(),
                                                                    100
                                                                )
                                                            );
                                                        }
                                                    }
                                                    _ => {
                                                        log_info!(
                                                            "    {}.{}: {}",
                                                            key,
                                                            sub_key,
                                                            sub_value
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        serde_json::Value::Array(arr) => {
                                            log_info!("  {}: [Array 有{}个元素]", key, arr.len());
                                            if arr.len() > 0 {
                                                log_info!(
                                                    "    第一个元素: {}",
                                                    safe_truncate_string(&arr[0].to_string(), 100)
                                                );
                                            }
                                        }
                                        _ => {
                                            log_info!("  {}: {}", key, value);
                                        }
                                    }
                                }
                            }
                        }

                        log_info!("📋 (完成前3个prompt详细分析，继续处理所有prompts...)");

                        // 处理所有prompts进行数据提取
                        for (index, prompt) in prompts_array.iter().enumerate() {
                            if let Some(prompt_obj) = prompt.as_object() {
                                let text = prompt_obj
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                // 尝试从prompt对象中获取时间戳
                                let timestamp = prompt_obj
                                    .get("timestamp")
                                    .and_then(|v| v.as_i64())
                                    .or_else(|| {
                                        prompt_obj.get("createdAt").and_then(|v| v.as_i64())
                                    })
                                    .or_else(|| prompt_obj.get("unixMs").and_then(|v| v.as_i64()))
                                    .unwrap_or_else(|| {
                                        // 如果没有找到时间戳，使用基于索引的递增时间戳
                                        // 这样可以保证消息的相对顺序
                                        let base_time = chrono::Utc::now().timestamp_millis()
                                            - (prompts_array.len() as i64 * 60000);
                                        base_time + (index as i64 * 60000) // 每条消息间隔1分钟
                                    });

                                if !text.is_empty() {
                                    prompts.push((text.clone(), timestamp));
                                    log_info!(
                                        "✅ 提取用户输入 [{}]: {} (时间戳: {})",
                                        index,
                                        safe_truncate_string(&text, 50),
                                        timestamp
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log_info!("⚠️ 解析aiService.prompts JSON失败: {}", e);
                }
            }
            break;
        }
    }

    log_info!("✅ 获取到 {} 个用户输入", prompts.len());
    Ok(prompts)
}

/// 获取AI生成数据
fn get_ai_generations(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    log_info!("🔍 开始获取AI生成数据...");

    // 先检查这个key是否存在
    let mut check_stmt = conn
        .prepare("SELECT COUNT(*) FROM ItemTable WHERE key = 'aiService.generations'")
        .map_err(|e| format!("准备检查generations存在性失败: {}", e))?;

    let count: i64 = check_stmt
        .query_row([], |row| row.get(0))
        .map_err(|e| format!("检查generations存在性失败: {}", e))?;

    log_info!("📊 aiService.generations key存在数量: {}", count);

    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = 'aiService.generations'")
        .map_err(|e| format!("准备查询generations失败: {}", e))?;

    let mut generations = Vec::new();

    let rows = stmt
        .query_map([], |row| {
            let value_str: String = row.get(0)?;
            Ok(value_str)
        })
        .map_err(|e| format!("查询generations执行失败: {}", e))?;

    for row_result in rows {
        if let Ok(value_str) = row_result {
            log_info!("📄 aiService.generations内容长度: {} 字节", value_str.len());

            // 调试：显示前200个字符
            if value_str.len() > 200 {
                log_info!(
                    "📄 aiService.generations前200字符: {}",
                    safe_truncate_string(&value_str, 200)
                );
            } else {
                log_info!("📄 aiService.generations完整内容: {}", value_str);
            }

            match serde_json::from_str::<serde_json::Value>(&value_str) {
                Ok(json_value) => {
                    if let Some(generations_array) = json_value.as_array() {
                        log_info!("📋 找到 {} 个generation记录", generations_array.len());

                        for (index, generation) in generations_array.iter().enumerate() {
                            if let Some(gen_obj) = generation.as_object() {
                                // 深度分析generation对象的完整结构
                                log_info!("🔍 Generation [{}] 完整结构深度分析:", index);

                                // 打印完整的JSON结构，但截断过长的字符串
                                let json_str =
                                    serde_json::to_string_pretty(generation).unwrap_or_default();
                                if json_str.len() > 1000 {
                                    log_info!(
                                        "📄 完整JSON结构 (前1000字符): {}",
                                        safe_truncate_string(&json_str, 1000)
                                    );
                                } else {
                                    log_info!("📄 完整JSON结构: {}", json_str);
                                }

                                // 分析顶级字段
                                for (key, value) in gen_obj {
                                    match value {
                                        serde_json::Value::String(s) => {
                                            if s.len() > 100 {
                                                log_info!(
                                                    "  {}: \"{}...\" ({}字符)",
                                                    key,
                                                    safe_truncate_string(s, 30),
                                                    s.chars().count()
                                                );
                                            } else {
                                                log_info!("  {}: \"{}\"", key, s);
                                            }
                                        }
                                        serde_json::Value::Object(obj) => {
                                            log_info!("  {}: [Object 有{}个字段]", key, obj.len());
                                            // 分析嵌套对象的字段
                                            for (sub_key, sub_value) in obj {
                                                match sub_value {
                                                    serde_json::Value::String(s) => {
                                                        if s.len() > 50 {
                                                            log_info!(
                                                                "    {}.{}: \"{}...\" ({}字符)",
                                                                key,
                                                                sub_key,
                                                                safe_truncate_string(s, 30),
                                                                s.chars().count()
                                                            );
                                                        } else {
                                                            log_info!(
                                                                "    {}.{}: \"{}\"",
                                                                key,
                                                                sub_key,
                                                                s
                                                            );
                                                        }
                                                    }
                                                    _ => {
                                                        log_info!(
                                                            "    {}.{}: {}",
                                                            key,
                                                            sub_key,
                                                            sub_value
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        serde_json::Value::Array(arr) => {
                                            log_info!("  {}: [Array 有{}个元素]", key, arr.len());
                                            if arr.len() > 0 {
                                                log_info!(
                                                    "    第一个元素: {}",
                                                    safe_truncate_string(&arr[0].to_string(), 100)
                                                );
                                            }
                                        }
                                        _ => {
                                            log_info!("  {}: {}", key, value);
                                        }
                                    }
                                }

                                // 只分析前2个generation对象避免日志过长
                                if index >= 1 {
                                    log_info!("📋 (只分析前2个generation对象，避免日志过长...)");
                                    break;
                                }

                                // 尝试多个可能的字段名来找到AI的真实回复
                                let text = gen_obj
                                    .get("response") // 可能的AI回复字段1
                                    .and_then(|v| v.as_str())
                                    .or_else(|| gen_obj.get("content").and_then(|v| v.as_str())) // 可能的AI回复字段2
                                    .or_else(|| {
                                        gen_obj.get("generatedText").and_then(|v| v.as_str())
                                    }) // 可能的AI回复字段3
                                    .or_else(|| gen_obj.get("output").and_then(|v| v.as_str())) // 可能的AI回复字段4
                                    .or_else(|| {
                                        gen_obj.get("textDescription").and_then(|v| v.as_str())
                                    }) // 原字段作为备选
                                    .unwrap_or("")
                                    .to_string();

                                let timestamp = gen_obj
                                    .get("unixMs")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or_else(|| {
                                        // 如果没有找到时间戳，使用基于索引的递增时间戳
                                        let base_time = chrono::Utc::now().timestamp_millis()
                                            - (generations_array.len() as i64 * 60000);
                                        base_time + (index as i64 * 60000) + 30000 // AI回复比用户输入晚30秒
                                    });

                                if !text.is_empty() {
                                    generations.push((text.clone(), timestamp));
                                    log_info!(
                                        "✅ 提取AI回复 [{}]: {} (时间戳: {})",
                                        index,
                                        safe_truncate_string(&text, 50),
                                        timestamp
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log_info!("⚠️ 解析aiService.generations JSON失败: {}", e);
                }
            }
            break;
        }
    }

    log_info!("⚠️ aiService.prompts和aiService.generations都只存储用户输入！");
    log_info!("🔍 开始全面搜索数据库中所有可能包含AI回复的字段...");

    // 搜索所有key并分析其内容大小
    let mut all_keys_stmt = conn
        .prepare("SELECT key, LENGTH(value) as size FROM ItemTable ORDER BY size DESC")
        .map_err(|e| format!("准备查询所有keys失败: {}", e))?;

    let key_rows = all_keys_stmt
        .query_map([], |row| {
            let key: String = row.get(0)?;
            let size: i64 = row.get(1)?;
            Ok((key, size))
        })
        .map_err(|e| format!("查询所有keys失败: {}", e))?;

    log_info!("🔍 数据库中所有字段按大小排序 (前20个):");
    let mut count = 0;
    for key_result in key_rows {
        if let Ok((key, size)) = key_result {
            log_info!("  📋 {}: {} 字节", key, size);
            count += 1;
            if count >= 20 {
                break;
            }
        }
    }

    log_info!("🔍 寻找可能包含AI回复的特殊字段...");

    // 搜索可能包含对话/回复的所有字段
    let search_patterns = vec![
        "%chat%",
        "%conversation%",
        "%message%",
        "%reply%",
        "%response%",
        "%assistant%",
        "%completion%",
        "%output%",
        "%result%",
        "%answer%",
        "%ai%",
        "%generation%",
        "%composer%",
        "%dialog%",
        "%thread%",
    ];

    for pattern in search_patterns {
        let mut pattern_stmt = conn.prepare(&format!("SELECT key, LENGTH(value) FROM ItemTable WHERE key LIKE '{}' ORDER BY LENGTH(value) DESC", pattern))
            .map_err(|e| format!("准备查询pattern {}失败: {}", pattern, e))?;

        let pattern_rows = pattern_stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let size: i64 = row.get(1)?;
                Ok((key, size))
            })
            .map_err(|e| format!("查询pattern {}失败: {}", pattern, e))?;

        let mut found_any = false;
        for key_result in pattern_rows {
            if let Ok((key, size)) = key_result {
                if size > 1000 {
                    // 只关注大于1KB的字段
                    if !found_any {
                        log_info!("  🎯 模式 {} 匹配到:", pattern);
                        found_any = true;
                    }
                    log_info!("    📄 {}: {} 字节", key, size);

                    // 尝试读取并分析这个字段的内容
                    let mut content_stmt = conn
                        .prepare("SELECT value FROM ItemTable WHERE key = ?")
                        .map_err(|e| format!("准备查询内容失败: {}", e))?;

                    if let Ok(content) =
                        content_stmt.query_row([&key], |row| -> Result<String, _> { row.get(0) })
                    {
                        log_info!(
                            "      📄 内容预览 (前200字符): {}",
                            safe_truncate_string(&content, 200)
                        );

                        // 尝试解析为JSON看看结构
                        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content)
                        {
                            match &json_value {
                                serde_json::Value::Array(arr) => {
                                    log_info!("      📊 JSON数组，包含 {} 个元素", arr.len());
                                    if arr.len() > 0 {
                                        let first_element = &arr[0];
                                        if let Some(obj) = first_element.as_object() {
                                            log_info!(
                                                "      🔍 第一个元素的字段: {}",
                                                obj.keys()
                                                    .map(|k| k.as_str())
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            );
                                        }
                                    }
                                }
                                serde_json::Value::Object(obj) => {
                                    log_info!(
                                        "      📊 JSON对象，包含字段: {}",
                                        obj.keys()
                                            .map(|k| k.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    );
                                }
                                _ => {
                                    log_info!(
                                        "      📊 JSON类型: {}",
                                        match json_value {
                                            serde_json::Value::String(_) => "String",
                                            serde_json::Value::Number(_) => "Number",
                                            serde_json::Value::Bool(_) => "Bool",
                                            serde_json::Value::Null => "Null",
                                            _ => "Unknown",
                                        }
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    log_info!("🔍 深入分析composer相关字段，寻找AI回复...");

    // 分析composer.composerData
    let mut composer_stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = 'composer.composerData'")
        .map_err(|e| format!("准备查询composer.composerData失败: {}", e))?;

    if let Ok(composer_content) =
        composer_stmt.query_row([], |row| -> Result<String, _> { row.get(0) })
    {
        log_info!("🔍 composer.composerData 内容分析:");
        log_info!("  📄 内容长度: {} 字节", composer_content.len());
        log_info!(
            "  📄 内容预览: {}",
            safe_truncate_string(&composer_content, 300)
        );

        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&composer_content) {
            log_info!("  📊 JSON结构分析:");
            match &json_value {
                serde_json::Value::Object(obj) => {
                    for (key, value) in obj {
                        match value {
                            serde_json::Value::String(s) => {
                                if s.len() > 100 {
                                    log_info!(
                                        "    {}: \"{}...\" ({}字符)",
                                        key,
                                        safe_truncate_string(s, 50),
                                        s.chars().count()
                                    );
                                } else {
                                    log_info!("    {}: \"{}\"", key, s);
                                }
                            }
                            serde_json::Value::Array(arr) => {
                                log_info!("    {}: [Array 有{}个元素]", key, arr.len());
                                if arr.len() > 0 {
                                    log_info!(
                                        "      第一个元素: {}",
                                        safe_truncate_string(&arr[0].to_string(), 100)
                                    );
                                }
                            }
                            serde_json::Value::Object(nested_obj) => {
                                log_info!("    {}: [Object 有{}个字段]", key, nested_obj.len());
                                log_info!(
                                    "      字段: {}",
                                    nested_obj
                                        .keys()
                                        .map(|k| k.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                            }
                            _ => {
                                log_info!("    {}: {}", key, value);
                            }
                        }
                    }
                }
                _ => {
                    log_info!("  📊 非对象类型: {}", json_value);
                }
            }
        }
    } else {
        log_info!("⚠️ 未找到composer.composerData字段");
    }

    // 分析workbench.backgroundComposer.workspacePersistentData
    let mut bg_composer_stmt = conn.prepare("SELECT value FROM ItemTable WHERE key = 'workbench.backgroundComposer.workspacePersistentData'")
        .map_err(|e| format!("准备查询backgroundComposer失败: {}", e))?;

    if let Ok(bg_composer_content) =
        bg_composer_stmt.query_row([], |row| -> Result<String, _> { row.get(0) })
    {
        log_info!("🔍 workbench.backgroundComposer.workspacePersistentData 内容分析:");
        log_info!("  📄 内容长度: {} 字节", bg_composer_content.len());
        log_info!(
            "  📄 内容预览: {}",
            safe_truncate_string(&bg_composer_content, 300)
        );

        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&bg_composer_content) {
            log_info!("  📊 JSON结构分析:");
            match &json_value {
                serde_json::Value::Object(obj) => {
                    for (key, value) in obj {
                        match value {
                            serde_json::Value::String(s) => {
                                if s.len() > 100 {
                                    log_info!(
                                        "    {}: \"{}...\" ({}字符)",
                                        key,
                                        safe_truncate_string(s, 50),
                                        s.chars().count()
                                    );
                                } else {
                                    log_info!("    {}: \"{}\"", key, s);
                                }
                            }
                            serde_json::Value::Array(arr) => {
                                log_info!("    {}: [Array 有{}个元素]", key, arr.len());
                                if arr.len() > 0 && arr.len() <= 5 {
                                    for (i, item) in arr.iter().enumerate() {
                                        log_info!(
                                            "      [{}]: {}",
                                            i,
                                            safe_truncate_string(&item.to_string(), 100)
                                        );
                                    }
                                } else if arr.len() > 5 {
                                    log_info!("      前3个元素:");
                                    for (i, item) in arr.iter().take(3).enumerate() {
                                        log_info!(
                                            "      [{}]: {}",
                                            i,
                                            safe_truncate_string(&item.to_string(), 100)
                                        );
                                    }
                                }
                            }
                            serde_json::Value::Object(nested_obj) => {
                                log_info!("    {}: [Object 有{}个字段]", key, nested_obj.len());
                                log_info!(
                                    "      字段: {}",
                                    nested_obj
                                        .keys()
                                        .map(|k| k.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                            }
                            _ => {
                                log_info!("    {}: {}", key, value);
                            }
                        }
                    }
                }
                _ => {
                    log_info!("  📊 非对象类型: {}", json_value);
                }
            }
        }
    } else {
        log_info!("⚠️ 未找到workbench.backgroundComposer.workspacePersistentData字段");
    }

    log_info!("💡 结论: 如果这些字段也没有AI回复，那么AI回复可能:");
    log_info!("  1. 存储在workspace目录的其他文件中");
    log_info!("  2. 通过API实时获取，不进行持久化存储");
    log_info!("  3. 存储在Cursor的云端或其他位置");

    log_info!("✅ 获取到 {} 个AI回复", generations.len());
    Ok(generations)
}

/// 从composer.composerData获取基础对话信息
fn get_composer_data(conn: &Connection) -> Result<Vec<ConversationData>, String> {
    log_info!("🔍 开始从composer.composerData获取对话信息...");

    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = 'composer.composerData'")
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let mut conversations = Vec::new();

    let rows = stmt
        .query_map([], |row| {
            let value_str: String = row.get(0)?;
            Ok(value_str)
        })
        .map_err(|e| format!("查询执行失败: {}", e))?;

    for row_result in rows {
        if let Ok(value_str) = row_result {
            log_info!("📄 composer.composerData内容长度: {} 字节", value_str.len());

            match serde_json::from_str::<serde_json::Value>(&value_str) {
                Ok(json_value) => {
                    if let Some(obj) = json_value.as_object() {
                        if let Some(all_composers) =
                            obj.get("allComposers").and_then(|v| v.as_array())
                        {
                            log_info!("📋 找到 {} 个composer", all_composers.len());

                            for (index, composer) in all_composers.iter().enumerate() {
                                if let Some(composer_obj) = composer.as_object() {
                                    log_info!("🔍 分析第{}个composer对象:", index + 1);

                                    // 打印所有字段以便调试
                                    for (key, value) in composer_obj {
                                        match value {
                                            serde_json::Value::String(s) => {
                                                log_info!(
                                                    "  {} = \"{}\"",
                                                    key,
                                                    safe_truncate_string(s, 50)
                                                );
                                            }
                                            serde_json::Value::Number(n) => {
                                                log_info!("  {} = {}", key, n);
                                            }
                                            serde_json::Value::Bool(b) => {
                                                log_info!("  {} = {}", key, b);
                                            }
                                            serde_json::Value::Array(arr) => {
                                                log_info!(
                                                    "  {} = [Array 有{}个元素]",
                                                    key,
                                                    arr.len()
                                                );
                                            }
                                            serde_json::Value::Object(obj) => {
                                                log_info!(
                                                    "  {} = [Object 有{}个字段]",
                                                    key,
                                                    obj.len()
                                                );
                                            }
                                            _ => {
                                                log_info!("  {} = {}", key, value);
                                            }
                                        }
                                    }

                                    let composer_id = composer_obj
                                        .get("composerId")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();

                                    let created_at = composer_obj
                                        .get("createdAt")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0);

                                    // 尝试提取真正的标题
                                    let title = if let Some(title_field) =
                                        composer_obj.get("title").and_then(|v| v.as_str())
                                    {
                                        title_field.to_string()
                                    } else if let Some(name_field) =
                                        composer_obj.get("name").and_then(|v| v.as_str())
                                    {
                                        name_field.to_string()
                                    } else if let Some(label_field) =
                                        composer_obj.get("label").and_then(|v| v.as_str())
                                    {
                                        label_field.to_string()
                                    } else if let Some(description_field) =
                                        composer_obj.get("description").and_then(|v| v.as_str())
                                    {
                                        description_field.to_string()
                                    } else {
                                        format!("对话 {}", safe_truncate_string(&composer_id, 8))
                                    };

                                    // 创建ConversationData
                                    conversations.push(ConversationData {
                                        id: composer_id.clone(),
                                        title: title.clone(),
                                        last_message: "等待加载内容...".to_string(),
                                        created_at: format_timestamp(created_at),
                                        message_count: 0, // 稍后从aiService中更新
                                    });

                                    log_info!(
                                        "✅ 提取composer: id={}, title=\"{}\"",
                                        composer_id,
                                        title
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log_info!("⚠️ 解析composer.composerData JSON失败: {}", e);
                }
            }
            break; // 只有一个composer.composerData记录
        }
    }

    // 如果composer.composerData中没有找到对话，尝试其他数据源
    if conversations.is_empty() {
        log_info!("🔍 composer.composerData中未找到对话，尝试其他数据源...");

        // 检查 workbench.auxiliarybar.viewContainersWorkspaceState
        let mut aux_stmt = conn.prepare("SELECT value FROM ItemTable WHERE key = 'workbench.auxiliarybar.viewContainersWorkspaceState'")
            .map_err(|e| format!("准备查询auxiliarybar失败: {}", e))?;

        if let Ok(aux_json_str) = aux_stmt.query_row([], |row| -> Result<String, _> { row.get(0) })
        {
            log_info!("✅ 找到auxiliarybar数据，长度: {}", aux_json_str.len());

            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&aux_json_str) {
                if let Some(array) = json_value.as_array() {
                    log_info!("📊 auxiliarybar包含 {} 个元素", array.len());

                    let mut aichat_count = 0;
                    for item in array {
                        if let Some(obj) = item.as_object() {
                            if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                                // 查找 aichat 相关的对话
                                if id.contains("aichat") {
                                    aichat_count += 1;
                                    let visible = obj
                                        .get("visible")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false);
                                    log_info!(
                                        "🎯 找到aichat对话 {}: {} (visible: {})",
                                        aichat_count,
                                        id,
                                        visible
                                    );

                                    // 提取对话UUID
                                    if let Some(uuid_start) = id.rfind('.') {
                                        let uuid = &id[uuid_start + 1..];
                                        if uuid.len() == 36 {
                                            let conversation = ConversationData {
                                                id: uuid.to_string(),
                                                title: format!("对话 {}", aichat_count),
                                                last_message: "等待加载内容...".to_string(),
                                                created_at: "2025-10-25 20:00:00 UTC".to_string(),
                                                message_count: 0,
                                            };
                                            conversations.push(conversation);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    log_info!("🎯 从auxiliarybar找到 {} 个aichat对话", aichat_count);
                }
            }
        }
    }

    log_info!("✅ 总共提取到 {} 个对话", conversations.len());
    Ok(conversations)
}

/// 从aiService获取对话内容
fn get_ai_service_conversations(conn: &Connection) -> Result<Vec<(String, String, i64)>, String> {
    log_info!("🔍 开始从aiService获取对话内容...");

    let mut ai_data = Vec::new();

    // 获取aiService.generations
    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = 'aiService.generations'")
        .map_err(|e| format!("准备查询generations失败: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            let value_str: String = row.get(0)?;
            Ok(value_str)
        })
        .map_err(|e| format!("查询generations执行失败: {}", e))?;

    for row_result in rows {
        if let Ok(value_str) = row_result {
            log_info!("📄 aiService.generations内容长度: {} 字节", value_str.len());

            match serde_json::from_str::<serde_json::Value>(&value_str) {
                Ok(json_value) => {
                    if let Some(generations) = json_value.as_array() {
                        log_info!("📋 找到 {} 个generation记录", generations.len());

                        for generation in generations {
                            if let Some(gen_obj) = generation.as_object() {
                                let uuid = gen_obj
                                    .get("generationUUID")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let text = gen_obj
                                    .get("textDescription")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let timestamp =
                                    gen_obj.get("unixMs").and_then(|v| v.as_i64()).unwrap_or(0);

                                if !uuid.is_empty() && !text.is_empty() {
                                    ai_data.push((uuid, text, timestamp));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log_info!("⚠️ 解析aiService.generations JSON失败: {}", e);
                }
            }
            break;
        }
    }

    log_info!("✅ 从aiService提取到 {} 个对话记录", ai_data.len());
    Ok(ai_data)
}

/// 合并AI服务数据到对话中
fn merge_ai_service_data(
    conversations: &mut Vec<ConversationData>,
    ai_data: Vec<(String, String, i64)>,
) {
    log_info!("🔄 开始合并AI服务数据到对话中...");

    // 为每个对话统计消息和找到最新内容
    let mut conversation_stats: std::collections::HashMap<String, (usize, String, i64)> =
        std::collections::HashMap::new();

    for (uuid, text, timestamp) in ai_data {
        // 尝试从各种可能的关联中找到对话ID
        // 这里可能需要更复杂的匹配逻辑，目前先简单处理

        // 如果我们有足够的信息，可以尝试匹配到现有conversations
        // 暂时为每个unique的text创建统计
        let conversation_key = if conversations.is_empty() {
            uuid.clone()
        } else {
            // 尝试匹配到现有对话ID
            conversations.first().unwrap().id.clone()
        };

        let entry = conversation_stats
            .entry(conversation_key)
            .or_insert((0, String::new(), 0));
        entry.0 += 1; // 增加消息计数

        // 更新最新消息
        if timestamp > entry.2 {
            entry.1 = text;
            entry.2 = timestamp;
        }
    }

    // 应用统计到conversations
    if let Some(conv) = conversations.first_mut() {
        if let Some((count, last_message, _)) = conversation_stats.get(&conv.id) {
            conv.message_count = *count;
            conv.last_message = last_message.clone();
        }
    }

    log_info!("✅ AI服务数据合并完成");
}

/// 安全截取字符串（按字符数，不是字节数）
fn safe_truncate_string(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// 格式化时间戳
fn format_timestamp(timestamp_ms: i64) -> String {
    if timestamp_ms == 0 {
        return "未知时间".to_string();
    }

    // 将毫秒时间戳转换为秒
    let timestamp_secs = timestamp_ms / 1000;

    match DateTime::from_timestamp(timestamp_secs, 0) {
        Some(datetime) => {
            let utc_datetime: DateTime<Utc> = datetime.into();
            utc_datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
        None => {
            format!("时间戳: {}", timestamp_ms)
        }
    }
}

/// 调试SQLite数据库内容
fn debug_sqlite_content(sqlite_path: &PathBuf) -> Result<(), String> {
    log_info!("🔍 开始调试SQLite数据库: {:?}", sqlite_path);

    match Connection::open(sqlite_path) {
        Ok(conn) => {
            // 获取所有表名
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table'")
                .map_err(|e| format!("准备查询失败: {}", e))?;

            let table_names: Result<Vec<String>, _> = stmt
                .query_map([], |row| Ok(row.get::<_, String>(0)?))
                .unwrap()
                .collect();

            match table_names {
                Ok(tables) => {
                    log_info!("📋 找到 {} 个表:", tables.len());
                    for table in &tables {
                        log_info!("  - {}", table);

                        // 打印每个表的结构
                        if let Ok(mut pragma_stmt) =
                            conn.prepare(&format!("PRAGMA table_info({})", table))
                        {
                            log_info!("    结构:");
                            let _: Result<Vec<_>, _> = pragma_stmt
                                .query_map([], |row| {
                                    let col_name: String = row.get(1)?;
                                    let col_type: String = row.get(2)?;
                                    log_info!("      {} ({})", col_name, col_type);
                                    Ok(())
                                })
                                .unwrap()
                                .collect();
                        }

                        // 打印每个表的前几条数据
                        if let Ok(mut count_stmt) =
                            conn.prepare(&format!("SELECT COUNT(*) FROM {}", table))
                        {
                            if let Ok(count) = count_stmt.query_row([], |row| row.get::<_, i64>(0))
                            {
                                log_info!("    数据行数: {}", count);

                                if count > 0 && count <= 10 {
                                    // 如果数据不多，打印所有数据
                                    if let Ok(mut data_stmt) =
                                        conn.prepare(&format!("SELECT * FROM {} LIMIT 5", table))
                                    {
                                        log_info!("    前5条数据:");
                                        let _: Result<Vec<_>, _> = data_stmt
                                            .query_map([], |row| {
                                                let mut row_data = Vec::new();
                                                let column_count = row.as_ref().column_count();
                                                for i in 0..column_count {
                                                    let value: Result<String, _> = row.get(i);
                                                    match value {
                                                        Ok(v) => row_data.push(v),
                                                        Err(_) => {
                                                            // 尝试其他类型
                                                            let int_val: Result<i64, _> =
                                                                row.get(i);
                                                            match int_val {
                                                                Ok(v) => {
                                                                    row_data.push(v.to_string())
                                                                }
                                                                Err(_) => row_data
                                                                    .push("NULL".to_string()),
                                                            }
                                                        }
                                                    }
                                                }
                                                log_info!("      [{}]", row_data.join(", "));
                                                Ok(())
                                            })
                                            .unwrap()
                                            .collect();
                                    }
                                }
                            }
                        }
                        log_info!(""); // 空行分隔
                    }
                }
                Err(e) => {
                    log_info!("❌ 获取表名失败: {}", e);
                }
            }

            // 执行全面搜索
            log_info!("");
            log_info!("🔍 执行全面对话数据搜索...");
            let _ = search_all_conversation_keys(&conn);

            Ok(())
        }
        Err(e) => Err(format!("连接SQLite失败: {}", e)),
    }
}

/// 提取对话ID列表
fn extract_conversation_ids(conn: &Connection) -> SqliteResult<Vec<String>> {
    let mut conversation_ids = Vec::new();

    // 查询包含对话面板配置的key和composer相关数据
    let queries = vec![
        "SELECT key, value FROM ItemTable WHERE key LIKE '%composerChatViewPane%' OR key LIKE '%aichat.view.%'",
        "SELECT key, value FROM ItemTable WHERE key LIKE '%composer%'",
        "SELECT key, value FROM ItemTable WHERE key = 'composer.composerData'",
    ];

    for query in queries {
        log_info!("🔍 执行查询: {}", query);

        if let Ok(mut stmt) = conn.prepare(query) {
            let mut row_count = 0;

            // 首先检查有多少行数据
            let rows = match stmt.query_map([], |row| {
                let key: String = row.get("key")?;
                let value_blob: Vec<u8> = row.get("value")?;
                Ok((key, value_blob))
            }) {
                Ok(rows) => rows.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
                Err(e) => {
                    log_info!("  ❌ 查询执行失败: {}", e);
                    continue;
                }
            };

            log_info!("  📊 查询返回 {} 行数据", rows.len());

            // 处理每一行数据
            for (key, value_blob) in rows {
                row_count += 1;
                log_info!(
                    "  📋 处理行 {}: key={}, blob_size={}",
                    row_count,
                    key,
                    value_blob.len()
                );

                // 将BLOB转换为字符串
                match String::from_utf8(value_blob.clone()) {
                    Ok(value_str) => {
                        log_info!("🔍 分析key: {}", key);
                        log_info!("📄 完整Value内容: {}", value_str);

                        // 从key中直接提取UUID（如果key本身就包含对话ID）
                        if key.contains("aichat.view.") {
                            if let Some(uuid_start) = key.rfind("aichat.view.") {
                                let uuid_part = &key[uuid_start + 12..]; // "aichat.view.".len() = 12
                                if uuid_part.len() >= 36 && uuid_part.chars().nth(8) == Some('-') {
                                    let uuid = &uuid_part[..36]; // UUID标准长度
                                    log_info!("  📋 从key提取UUID: {}", uuid);
                                    conversation_ids.push(uuid.to_string());
                                }
                            }
                        }

                        // 从JSON值中提取对话相关数据
                        match serde_json::from_str::<serde_json::Value>(&value_str) {
                            Ok(json_value) => {
                                log_info!("📋 成功解析JSON，开始分析结构...");
                                analyze_json_for_conversations(
                                    &json_value,
                                    &mut conversation_ids,
                                    &key,
                                );
                            }
                            Err(e) => {
                                log_info!(
                                    "⚠️ JSON解析失败 ({}): {}",
                                    e,
                                    if value_str.len() > 100 {
                                        safe_truncate_string(&value_str, 100)
                                    } else {
                                        value_str.clone()
                                    }
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log_info!(
                            "⚠️ BLOB转字符串失败 ({}): key={}, 尝试显示原始字节前50个",
                            e,
                            key
                        );
                        let preview: Vec<String> = value_blob
                            .iter()
                            .take(50)
                            .map(|b| format!("{:02x}", b))
                            .collect();
                        log_info!("   原始字节: {}", preview.join(" "));
                    }
                }
            }

            if row_count == 0 {
                log_info!("  ❌ 查询无结果");
            } else {
                log_info!("  ✅ 查询完成，处理了 {} 行数据", row_count);
            }
        } else {
            log_info!("  ❌ 查询准备失败");
        }
    }

    // 去重
    conversation_ids.sort();
    conversation_ids.dedup();

    log_info!("📋 提取到的唯一对话ID: {:?}", conversation_ids);
    Ok(conversation_ids)
}

/// 分析JSON数据以查找对话相关内容
fn analyze_json_for_conversations(
    json_value: &serde_json::Value,
    conversation_ids: &mut Vec<String>,
    key: &str,
) {
    log_info!("🔍 分析JSON数据，key: {}", key);

    match json_value {
        serde_json::Value::Object(obj) => {
            log_info!("📋 JSON对象包含 {} 个字段", obj.len());

            for (json_key, json_val) in obj {
                log_info!(
                    "  🔑 字段: {} = {}",
                    json_key,
                    if json_val.to_string().len() > 200 {
                        safe_truncate_string(&json_val.to_string(), 200)
                    } else {
                        json_val.to_string()
                    }
                );

                // 检查字段名是否包含对话相关关键词
                if json_key.to_lowercase().contains("conversation")
                    || json_key.to_lowercase().contains("chat")
                    || json_key.to_lowercase().contains("thread")
                    || json_key.to_lowercase().contains("message")
                {
                    log_info!("    🎯 找到对话相关字段: {}", json_key);
                }

                // 提取UUID格式的ID
                if json_key.contains("aichat.view.") {
                    if let Some(uuid_start) = json_key.rfind("aichat.view.") {
                        let uuid_part = &json_key[uuid_start + 12..];
                        if uuid_part.len() >= 36 && uuid_part.chars().nth(8) == Some('-') {
                            let uuid = &uuid_part[..36];
                            log_info!("  📋 从JSON字段提取UUID: {}", uuid);
                            conversation_ids.push(uuid.to_string());
                        }
                    }
                }

                // 递归分析嵌套对象
                if let serde_json::Value::Object(_) = json_val {
                    analyze_json_for_conversations(
                        json_val,
                        conversation_ids,
                        &format!("{}.{}", key, json_key),
                    );
                }

                // 分析数组
                if let serde_json::Value::Array(arr) = json_val {
                    log_info!("    📋 数组包含 {} 个元素", arr.len());
                    for (i, item) in arr.iter().enumerate() {
                        if i < 3 {
                            // 只显示前3个元素
                            log_info!(
                                "      [{}]: {}",
                                i,
                                if item.to_string().len() > 100 {
                                    safe_truncate_string(&item.to_string(), 100)
                                } else {
                                    item.to_string()
                                }
                            );
                        }
                        analyze_json_for_conversations(
                            item,
                            conversation_ids,
                            &format!("{}[{}]", key, i),
                        );
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            log_info!("📋 JSON数组包含 {} 个元素", arr.len());
            for (i, item) in arr.iter().enumerate() {
                analyze_json_for_conversations(item, conversation_ids, &format!("{}[{}]", key, i));
            }
        }
        _ => {
            log_info!(
                "📋 JSON值类型: {}",
                match json_value {
                    serde_json::Value::String(_) => "String",
                    serde_json::Value::Number(_) => "Number",
                    serde_json::Value::Bool(_) => "Bool",
                    serde_json::Value::Null => "Null",
                    _ => "Unknown",
                }
            );
        }
    }
}

/// 获取单个对话的详细数据
fn get_conversation_data(
    conn: &Connection,
    conversation_id: &str,
) -> SqliteResult<ConversationData> {
    log_info!("🔍 查询对话数据: {}", conversation_id);

    // 尝试多种可能的key格式查找对话数据
    let possible_keys = vec![
        format!("aichat.conversation.{}", conversation_id),
        format!("workbench.panel.aichat.conversation.{}", conversation_id),
        format!("conversation.{}", conversation_id),
        conversation_id.to_string(),
        format!("chat.{}", conversation_id),
    ];

    for key_pattern in &possible_keys {
        let query = "SELECT key, value FROM ItemTable WHERE key = ? OR key LIKE ?";

        if let Ok(mut stmt) = conn.prepare(query) {
            let like_pattern = format!("%{}%", key_pattern);
            let result: Result<Vec<_>, _> = stmt
                .query_map([key_pattern, &like_pattern], |row| {
                    let key: String = row.get("key")?;
                    let value_blob: Vec<u8> = row.get("value")?;

                    if let Ok(value_str) = String::from_utf8(value_blob) {
                        log_info!("  找到匹配的key: {} | Value长度: {}", key, value_str.len());

                        if value_str.len() > 500 {
                            log_info!(
                                "  Value前500字符: {}",
                                safe_truncate_string(&value_str, 500)
                            );
                        } else {
                            log_info!("  Value内容: {}", value_str);
                        }

                        // 尝试解析对话数据
                        if let Ok(json_value) =
                            serde_json::from_str::<serde_json::Value>(&value_str)
                        {
                            log_info!("  成功解析JSON，结构: {:#}", json_value);
                        }
                    }

                    Ok(())
                })
                .unwrap()
                .collect();

            if result.is_ok() {
                log_info!("  ✅ 查询模式 {} 完成", key_pattern);
            }
        }
    }

    // 如果没找到具体的对话数据，创建一个占位对话
    Ok(ConversationData {
        id: conversation_id.to_string(),
        title: format!("对话 {}", safe_truncate_string(conversation_id, 8)),
        last_message: "暂未找到对话内容".to_string(),
        created_at: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        message_count: 0,
    })
}

/// 全面搜索可能包含对话数据的所有keys（用于调试）
fn search_all_conversation_keys(conn: &Connection) -> SqliteResult<()> {
    log_info!("🔍 全面搜索所有可能的对话相关keys...");

    // 搜索包含对话ID的所有keys
    let conversation_ids = [
        "083449b1-6df2-46d1-8020-8ef413010ada",
        "9eedb95d-08dc-47b6-9657-97d52178c448",
        "a16f04bb-99d8-4947-909d-7546dbeec176",
    ];

    for conv_id in &conversation_ids {
        log_info!("🔍 搜索对话ID: {}", conv_id);

        // 搜索包含这个ID的所有key
        let query = "SELECT key, length(value) as value_length FROM ItemTable WHERE key LIKE ?";
        if let Ok(mut stmt) = conn.prepare(query) {
            let pattern = format!("%{}%", conv_id);
            let _: Result<Vec<_>, _> = stmt
                .query_map([&pattern], |row| {
                    let key: String = row.get("key")?;
                    let value_length: i64 = row.get("value_length")?;
                    log_info!("  📋 找到key: {} (值长度: {})", key, value_length);
                    Ok(())
                })
                .unwrap()
                .collect();
        }
    }

    // 搜索可能包含对话内容的其他模式
    let search_patterns = vec![
        "%message%",
        "%conversation%",
        "%chat%",
        "%aichat%",
        "%composer%",
        "%dialog%",
        "%thread%",
    ];

    for pattern in &search_patterns {
        log_info!("🔍 搜索模式: {}", pattern);
        let query = "SELECT key, length(value) as value_length FROM ItemTable WHERE key LIKE ? AND length(value) > 100";

        if let Ok(mut stmt) = conn.prepare(query) {
            let mut count = 0;
            let _: Result<Vec<_>, _> = stmt
                .query_map([pattern], |row| {
                    let key: String = row.get("key")?;
                    let value_length: i64 = row.get("value_length")?;

                    if count < 5 {
                        // 只显示前5个
                        log_info!(
                            "  📋 {}模式匹配: {} (值长度: {})",
                            pattern,
                            key,
                            value_length
                        );
                        count += 1;
                    }

                    Ok(())
                })
                .unwrap()
                .collect();

            if count == 0 {
                log_info!("  ❌ {}模式无匹配", pattern);
            } else {
                log_info!("  ✅ {}模式找到匹配项", pattern);
            }
        }
    }

    Ok(())
}

/// 调试特定工作区的SQLite数据库内容（用于开发调试）
#[tauri::command]
pub async fn debug_workspace_sqlite(workspace_id: String) -> Result<String, String> {
    log_info!("🔍 调试工作区SQLite: {}", workspace_id);

    let (_, workspace_path) = get_cursor_backup_paths()?;
    let workspace_dir = workspace_path.join(&workspace_id);
    let sqlite_path = workspace_dir.join("state.vscdb");

    if !sqlite_path.exists() {
        return Ok(format!("SQLite文件不存在: {:?}", sqlite_path));
    }

    // 使用同一个数据库连接进行所有操作
    match Connection::open(&sqlite_path) {
        Ok(conn) => {
            // 先执行基础调试
            log_info!("🔍 开始完整的数据库分析...");

            // 1. 获取表信息
            match conn.prepare("SELECT name FROM sqlite_master WHERE type='table'") {
                Ok(mut stmt) => {
                    match stmt.query_map([], |row| {
                        let table_name: String = row.get(0)?;
                        Ok(table_name)
                    }) {
                        Ok(rows) => {
                            let table_names: Vec<String> =
                                rows.collect::<Result<Vec<_>, _>>().unwrap_or_default();
                            log_info!("📋 找到 {} 个表: {:?}", table_names.len(), table_names);
                        }
                        Err(e) => log_info!("❌ 获取表名失败: {}", e),
                    }
                }
                Err(e) => log_info!("❌ 准备表查询失败: {}", e),
            }

            // 2. 获取ItemTable行数
            match conn.prepare("SELECT COUNT(*) FROM ItemTable") {
                Ok(mut stmt) => {
                    match stmt.query_row([], |row| {
                        let count: i64 = row.get(0)?;
                        Ok(count)
                    }) {
                        Ok(count) => {
                            log_info!("📊 ItemTable实际行数: {}", count);
                        }
                        Err(e) => log_info!("❌ 获取行数失败: {}", e),
                    }
                }
                Err(e) => log_info!("❌ 准备行数查询失败: {}", e),
            }

            // 3. 直接查询所有数据，使用更详细的错误处理
            log_info!("🔍 开始查询ItemTable所有数据...");
            let query = "SELECT key, value FROM ItemTable ORDER BY key";
            log_info!("🔍 执行SQL: {}", query);

            match conn.prepare(query) {
                Ok(mut stmt) => {
                    log_info!("✅ SQL准备成功");
                    match stmt.query_map([], |row| {
                        let key: String = row.get("key").map_err(|e| {
                            log_info!("❌ 获取key失败: {}", e);
                            e
                        })?;

                        // 尝试读取value - 先作为TEXT，如果失败再作为BLOB
                        let value_str = if let Ok(text_value) = row.get::<_, String>("value") {
                            log_info!(
                                "✅ 成功获取行(TEXT): key={}, size={}",
                                key,
                                text_value.len()
                            );
                            text_value
                        } else if let Ok(blob_value) = row.get::<_, Vec<u8>>("value") {
                            match String::from_utf8(blob_value) {
                                Ok(converted_str) => {
                                    log_info!(
                                        "✅ 成功获取行(BLOB->TEXT): key={}, size={}",
                                        key,
                                        converted_str.len()
                                    );
                                    converted_str
                                }
                                Err(e) => {
                                    log_info!("❌ BLOB转字符串失败: {}", e);
                                    return Err(rusqlite::Error::InvalidColumnType(
                                        1,
                                        "value".to_string(),
                                        rusqlite::types::Type::Blob,
                                    ));
                                }
                            }
                        } else {
                            log_info!("❌ 无法读取value列");
                            return Err(rusqlite::Error::InvalidColumnType(
                                1,
                                "value".to_string(),
                                rusqlite::types::Type::Text,
                            ));
                        };

                        Ok((key, value_str))
                    }) {
                        Ok(rows_iterator) => {
                            log_info!("✅ 查询执行成功，开始收集结果...");

                            let mut collected_rows = Vec::new();
                            let mut row_count = 0;

                            for row_result in rows_iterator {
                                match row_result {
                                    Ok(row) => {
                                        row_count += 1;
                                        log_info!("📥 收集第{}行", row_count);
                                        collected_rows.push(row);
                                    }
                                    Err(e) => {
                                        log_info!("❌ 处理第{}行时出错: {}", row_count + 1, e);
                                    }
                                }
                            }

                            log_info!("📊 成功收集 {} 行数据", collected_rows.len());

                            for (i, (key, value_str)) in collected_rows.iter().enumerate() {
                                log_info!("🔸 第{}行: key = {}", i + 1, key);
                                log_info!("   📏 内容大小: {} 字节", value_str.len());

                                // 显示内容
                                if value_str.len() <= 200 {
                                    log_info!("   📄 完整内容: {}", value_str);
                                } else {
                                    log_info!(
                                        "   📄 内容前200字符: {}",
                                        safe_truncate_string(&value_str, 200)
                                    );
                                    log_info!("   📄 内容后200字符: {}", {
                                        let chars: Vec<char> = value_str.chars().collect();
                                        let start_pos = chars.len().saturating_sub(200);
                                        chars[start_pos..].iter().collect::<String>()
                                    });
                                }

                                // 检查是否为JSON
                                if value_str.trim_start().starts_with('{')
                                    || value_str.trim_start().starts_with('[')
                                {
                                    match serde_json::from_str::<serde_json::Value>(&value_str) {
                                        Ok(json_value) => {
                                            log_info!("   ✅ 这是有效的JSON数据");
                                            if let Some(obj) = json_value.as_object() {
                                                log_info!(
                                                    "   📋 JSON对象包含字段: {:?}",
                                                    obj.keys().collect::<Vec<_>>()
                                                );

                                                // 检查是否包含对话相关字段
                                                for field_name in obj.keys() {
                                                    if field_name
                                                        .to_lowercase()
                                                        .contains("conversation")
                                                        || field_name
                                                            .to_lowercase()
                                                            .contains("chat")
                                                        || field_name
                                                            .to_lowercase()
                                                            .contains("thread")
                                                        || field_name
                                                            .to_lowercase()
                                                            .contains("message")
                                                    {
                                                        log_info!(
                                                            "   🎯 找到对话相关字段: {}",
                                                            field_name
                                                        );
                                                        log_info!("       值: {}", obj[field_name]);
                                                    }
                                                }
                                            } else if let Some(arr) = json_value.as_array() {
                                                log_info!(
                                                    "   📋 JSON数组包含 {} 个元素",
                                                    arr.len()
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            log_info!("   ⚠️ JSON解析失败: {}", e);
                                        }
                                    }
                                } else {
                                    log_info!("   📋 这是普通字符串数据");
                                }

                                log_info!(""); // 空行分隔
                            }
                        }
                        Err(e) => {
                            log_info!("❌ 查询执行失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log_info!("❌ SQL准备失败: {}", e);
                }
            }
        }
        Err(e) => {
            log_info!("❌ 无法打开数据库: {}", e);
        }
    }

    Ok("SQLite调试信息和详细分析已输出到日志".to_string())
}

/// 快速获取目录基本信息（不递归扫描，避免性能问题）
fn get_dir_info_quick(dir_path: &PathBuf) -> Result<(u64, String, String), String> {
    let metadata = fs::metadata(dir_path).map_err(|e| format!("获取目录元数据失败: {}", e))?;

    // 简单估算：只计算几个主要文件的大小
    let mut estimated_size = 0u64;

    // 检查主要文件
    let important_files = ["state.vscdb", "workspace.json"];
    for file_name in &important_files {
        let file_path = dir_path.join(file_name);
        if let Ok(file_meta) = fs::metadata(&file_path) {
            estimated_size += file_meta.len();
        }
    }

    // 获取修改时间
    let last_modified_str = metadata
        .modified()
        .ok()
        .map(|time| {
            DateTime::<Utc>::from(time)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string()
        })
        .unwrap_or_else(|| "未知时间".to_string());

    // 获取创建时间
    let created_at_str = metadata
        .created()
        .ok()
        .map(|time| {
            DateTime::<Utc>::from(time)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string()
        })
        .unwrap_or_else(|| "未知时间".to_string());

    Ok((estimated_size, last_modified_str, created_at_str))
}

/// 获取目录信息（大小、最后修改时间和创建时间）- 完整版本，较慢
fn get_dir_info(dir_path: &PathBuf) -> Result<(u64, String, String), String> {
    let mut total_size = 0u64;
    let mut latest_modified = std::time::SystemTime::UNIX_EPOCH;
    let mut earliest_created = std::time::SystemTime::now();

    // 获取目录本身的创建时间
    if let Ok(metadata) = fs::metadata(dir_path) {
        if let Ok(created) = metadata.created() {
            earliest_created = created;
        }
    }

    fn scan_dir(
        dir: &PathBuf,
        total_size: &mut u64,
        latest_modified: &mut std::time::SystemTime,
        earliest_created: &mut std::time::SystemTime,
    ) -> Result<(), std::io::Error> {
        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if let Ok(modified) = metadata.modified() {
                if modified > *latest_modified {
                    *latest_modified = modified;
                }
            }

            if let Ok(created) = metadata.created() {
                if created < *earliest_created {
                    *earliest_created = created;
                }
            }

            if path.is_file() {
                *total_size += metadata.len();
            } else if path.is_dir() {
                scan_dir(&path, total_size, latest_modified, earliest_created)?;
            }
        }
        Ok(())
    }

    scan_dir(
        dir_path,
        &mut total_size,
        &mut latest_modified,
        &mut earliest_created,
    )
    .map_err(|e| format!("扫描目录失败: {}", e))?;

    let last_modified_str = if latest_modified != std::time::SystemTime::UNIX_EPOCH {
        format_system_time(latest_modified)
    } else {
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    };

    let created_at_str = format_system_time(earliest_created);

    Ok((total_size, last_modified_str, created_at_str))
}

/// 格式化系统时间为字符串
fn format_system_time(system_time: std::time::SystemTime) -> String {
    match system_time.duration_since(std::time::SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let datetime = DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0)
                .unwrap_or_else(|| Utc::now());
            datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
        Err(_) => Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    }
}

/// 获取对话详情
#[tauri::command]
pub async fn get_conversation_detail(
    workspace_id: String,
    conversation_id: String,
) -> Result<ConversationDetail, String> {
    log_info!("🔍 获取对话详情: {} - {}", workspace_id, conversation_id);

    let (_, workspace_path) = get_cursor_backup_paths()?;
    let workspace_dir = workspace_path.join(&workspace_id);

    if !workspace_dir.exists() {
        return Err(format!("工作区目录不存在: {}", workspace_id));
    }

    // 获取完整的聊天消息
    let messages = get_full_chat_messages(&workspace_dir)?;

    if messages.is_empty() {
        return Err("未找到聊天消息".to_string());
    }

    // 获取第一条和最后一条消息
    let first_message = &messages[0];
    let title = if let Some(user_msg) = messages.iter().find(|msg| msg.sender == "user") {
        safe_truncate_string(&user_msg.content, 30)
    } else {
        "新对话".to_string()
    };

    let conversation_detail = ConversationDetail {
        id: conversation_id.clone(),
        title,
        created_at: first_message.timestamp.clone(),
        message_count: messages.len(),
        messages,
    };

    log_info!(
        "✅ 获取对话详情完成，包含 {} 条消息",
        conversation_detail.message_count
    );

    Ok(conversation_detail)
}

/// 获取完整的聊天消息列表
fn get_full_chat_messages(workspace_dir: &PathBuf) -> Result<Vec<ChatMessage>, String> {
    let sqlite_path = workspace_dir.join("state.vscdb");

    if !sqlite_path.exists() {
        log_info!("⚠️ SQLite文件不存在: {:?}", sqlite_path);
        return Ok(vec![]);
    }

    match Connection::open(&sqlite_path) {
        Ok(conn) => {
            log_info!("🔍 开始从SQLite获取完整聊天消息...");

            // 1. 获取用户输入 (prompts)
            let user_prompts = get_user_prompts(&conn)?;
            log_info!("📋 获取到 {} 个用户输入", user_prompts.len());

            // 2. 获取AI回复 (generations)
            log_info!("🚀 即将调用 get_ai_generations...");
            let ai_generations = match get_ai_generations(&conn) {
                Ok(generations) => {
                    log_info!("📋 成功获取到 {} 个AI回复", generations.len());
                    generations
                }
                Err(e) => {
                    log_info!("❌ 获取AI回复失败: {}", e);
                    return Err(format!("获取AI回复失败: {}", e));
                }
            };

            // 3. 将所有消息按时间排序并转换为ChatMessage格式
            let mut temp_messages: Vec<(i64, String, String)> = Vec::new();

            // 添加用户消息 (保留数字时间戳用于排序)
            for (text, timestamp) in user_prompts {
                temp_messages.push((timestamp, "user".to_string(), text));
            }

            // 添加AI消息 (保留数字时间戳用于排序)
            for (text, timestamp) in ai_generations {
                temp_messages.push((timestamp, "assistant".to_string(), text));
            }

            // 按数字时间戳排序
            temp_messages.sort_by(|a, b| a.0.cmp(&b.0));

            // 添加排序调试信息
            log_info!("📋 消息排序后的前5条:");
            for (i, (timestamp, sender, content)) in temp_messages.iter().take(5).enumerate() {
                log_info!(
                    "  {}. [{}] {}: {} (ts: {})",
                    i + 1,
                    format_timestamp(*timestamp),
                    sender,
                    safe_truncate_string(content, 30),
                    timestamp
                );
            }

            // 转换为最终的ChatMessage格式
            let all_messages: Vec<ChatMessage> = temp_messages
                .into_iter()
                .map(|(timestamp, sender, content)| ChatMessage {
                    timestamp: format_timestamp(timestamp),
                    sender,
                    content,
                })
                .collect();

            log_info!("✅ 获取完整聊天消息完成，总共 {} 条", all_messages.len());
            Ok(all_messages)
        }
        Err(e) => {
            log_info!("⚠️ 连接SQLite失败: {}", e);
            Ok(vec![])
        }
    }
}
