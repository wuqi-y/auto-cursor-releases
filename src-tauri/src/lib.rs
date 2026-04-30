mod account_manager;
mod auth_checker;
mod cursor_info;
mod logger;
mod machine_id;
mod other;
mod weblog;

use account_manager::{AccountListResult, AccountManager, LogoutResult, SwitchAccountResult};
use auth_checker::{AuthCheckResult, AuthChecker, TokenInfo};
use base64::{Engine as _, engine::general_purpose};
use chrono;
use encoding_rs::GBK;
use machine_id::{BackupInfo, MachineIdRestorer, MachineIds, ResetResult, RestoreResult};
use rand::{Rng, distributions::Alphanumeric};
use regex::Regex;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(target_os = "windows"))]
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, Window};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// 全局应用句柄，用于在后台任务中发射事件
pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

// 全局注册进程管理器，用于存储和终止正在运行的注册进程
// key: task_id (生成自email), value: 进程ID (PID)
use once_cell::sync::Lazy;

static REGISTRATION_PROCESSES: Lazy<Arc<Mutex<HashMap<String, u32>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
static ANSI_ESCAPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\x1B\[[0-9;?]*[ -/]*[@-~]").unwrap());

// ==================== Cursor 备份相关功能 ====================
// 备份功能已迁移到 cursor_backup.rs 模块

mod cursor_backup;
pub use cursor_backup::{
    backup_cursor_data, cancel_backup, debug_workspace_sqlite, delete_cursor_backup,
    get_backup_list, get_conversation_detail, get_cursor_backup_info, get_workspace_details,
    get_workspace_storage_items, open_backup_dir, open_cursor_settings_dir,
    open_cursor_workspace_dir, open_directory_by_path, restore_cursor_data,
};

mod next_work_web;
pub use next_work_web::NextWorkWebServer;

mod vless_proxy;
pub use vless_proxy::{cancel_vless_xray_download, start_vless_proxy, stop_vless_proxy};

mod tray;

// 日志宏现在在logger.rs中定义

// 获取应用目录的辅助函数
pub fn get_app_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;
    let app_dir = exe_path
        .parent()
        .ok_or("Failed to get parent directory")?
        .to_path_buf();
    Ok(app_dir)
}

fn get_shared_config_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not find user home directory".to_string())?;
    let dir = home.join(".auto-cursor-vip");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create shared config dir: {}", e))?;
    Ok(dir)
}

fn get_primary_config_path(file_name: &str) -> Result<PathBuf, String> {
    Ok(get_shared_config_dir()?.join(file_name))
}

fn get_legacy_config_path(file_name: &str) -> Result<PathBuf, String> {
    Ok(get_app_dir()?.join(file_name))
}

fn read_config_with_legacy_fallback(file_name: &str) -> Result<String, String> {
    let primary_path = get_primary_config_path(file_name)?;
    if primary_path.exists() {
        return fs::read_to_string(&primary_path)
            .map_err(|e| format!("Failed to read config {:?}: {}", primary_path, e));
    }

    let legacy_path = get_legacy_config_path(file_name)?;
    if legacy_path.exists() {
        return fs::read_to_string(&legacy_path)
            .map_err(|e| format!("Failed to read legacy config {:?}: {}", legacy_path, e));
    }

    Ok(String::new())
}

fn merge_haozhuma_config_into_runtime_config(
    config_obj: &mut serde_json::Value,
    frontend_config: Option<&serde_json::Value>,
) {
    let saved_config = read_haozhuma_config_sync()
        .ok()
        .filter(|content| !content.trim().is_empty())
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok());

    let frontend_haozhuma_config = frontend_config.and_then(|value| value.get("haozhuma"));

    let merged_haozhuma = match (saved_config, frontend_haozhuma_config) {
        (Some(mut saved), Some(frontend)) => {
            merge_json_value(&mut saved, frontend);
            saved
        }
        (Some(saved), None) => saved,
        (None, Some(frontend)) => frontend.clone(),
        (None, None) => return,
    };

    config_obj["haozhuma"] = merged_haozhuma;
}

fn merge_json_value(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                let target_value = target_map
                    .entry(key.clone())
                    .or_insert(serde_json::Value::Null);
                merge_json_value(target_value, patch_value);
            }
        }
        (target_slot, patch_value) => {
            *target_slot = patch_value.clone();
        }
    }
}

// 创建隐藏窗口的Command（Windows平台适配）
fn create_hidden_command(executable_path: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new(executable_path);
        // Windows平台：隐藏命令行窗口
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(executable_path)
    }
}

#[cfg(target_os = "windows")]
fn terminate_process_tree(pid: u32) -> bool {
    match Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_process_descendants(pid: u32, visited: &mut HashSet<u32>, descendants: &mut Vec<u32>) {
    let output = match Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
    {
        Ok(output) => output,
        Err(_) => return,
    };

    if !output.status.success() {
        return;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let child_pid = match trimmed.parse::<u32>() {
            Ok(child_pid) => child_pid,
            Err(_) => continue,
        };

        if visited.insert(child_pid) {
            collect_process_descendants(child_pid, visited, descendants);
            descendants.push(child_pid);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn send_unix_signal(pid: u32, signal: &str) -> bool {
    match Command::new("kill")
        .args([signal, &pid.to_string()])
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn terminate_process_tree(pid: u32) -> bool {
    let mut visited = HashSet::new();
    let mut targets = Vec::new();
    collect_process_descendants(pid, &mut visited, &mut targets);
    targets.push(pid);
    targets.sort_unstable();
    targets.dedup();
    targets.reverse();

    let mut terminated = false;

    for target in &targets {
        if send_unix_signal(*target, "-TERM") {
            terminated = true;
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(350));

    for target in &targets {
        if send_unix_signal(*target, "-KILL") {
            terminated = true;
        }
    }

    terminated
}

// 生成唯一的任务ID（用于并行注册时隔离验证码文件）
fn generate_task_id(email: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let mut hasher = DefaultHasher::new();
    email.hash(&mut hasher);
    timestamp.hash(&mut hasher);

    format!("{:x}", hasher.finish())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexCdpOverrides {
    url: Option<String>,
    steps: Option<Vec<CodexCdpStep>>,
    wait_after_open: Option<f64>,
    wait_after_action: Option<f64>,
    element_timeout: Option<f64>,
    post_oauth_step1_py: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexCdpStep {
    #[serde(rename = "type")]
    step_type: String,
    selector: String,
    value: Option<String>,
    wait_for_load: Option<bool>,
}

#[derive(Debug, Clone)]
enum CodexCdpCliStep {
    Click {
        selector: String,
        wait_for_load: bool,
    },
    Input { selector: String, value: String },
}

fn default_codex_cdp_cli_steps(email: &str, access_password: &str) -> Vec<CodexCdpCliStep> {
    vec![
        CodexCdpCliStep::Click {
            selector: "css:button[class*='btn-secondary']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Input {
            selector: "css:input#email".to_string(),
            value: email.to_string(),
        },
        CodexCdpCliStep::Click {
            selector: "css:button[type='submit']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Input {
            selector: "css:input[type='password']".to_string(),
            value: access_password.to_string(),
        },
        CodexCdpCliStep::Click {
            selector: "css:button[type='submit']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Input {
            selector: "@name=otp".to_string(),
            value: "__AUTO__".to_string(),
        },
        CodexCdpCliStep::Input {
            selector: "@name=name".to_string(),
            value: "__RANDOM_EN_NAME__".to_string(),
        },
        CodexCdpCliStep::Click {
            selector: "css:button[type='submit']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Click {
            selector: "css:button[type='submit']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Input {
            selector: "@name=age".to_string(),
            value: "25".to_string(),
        },
        CodexCdpCliStep::Click {
            selector: "@name=allCheckboxes".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Click {
            selector: "css:button[type='submit']".to_string(),
            wait_for_load: false,
        },
        CodexCdpCliStep::Click {
            selector:
                "xpath://button[@type='submit' and (contains(normalize-space(.), 'Yes') or contains(normalize-space(.), '确定'))]"
                    .to_string(),
            wait_for_load: false,
        },
    ]
}

fn resolve_codex_cdp_step_value(value: &str, email: &str, access_password: &str) -> String {
    match value.trim() {
        "__REGISTER_EMAIL__" => email.to_string(),
        "__ACCESS_PASSWORD__" => access_password.to_string(),
        _ => value.to_string(),
    }
}

fn parse_codex_cdp_cli_steps(
    steps: &[CodexCdpStep],
    email: &str,
    access_password: &str,
) -> Result<Vec<CodexCdpCliStep>, String> {
    let mut parsed_steps = Vec::with_capacity(steps.len());

    for (index, step) in steps.iter().enumerate() {
        let selector = step.selector.trim();
        if selector.is_empty() {
            return Err(format!("Codex CDP steps[{}].selector 不能为空", index));
        }

        match step.step_type.trim() {
            "click" => parsed_steps.push(CodexCdpCliStep::Click {
                selector: selector.to_string(),
                wait_for_load: step.wait_for_load.unwrap_or(false),
            }),
            "input" => {
                let value = step
                    .value
                    .clone()
                    .ok_or_else(|| format!("Codex CDP steps[{}].value 不能为空", index))?;
                parsed_steps.push(CodexCdpCliStep::Input {
                    selector: selector.to_string(),
                    value: resolve_codex_cdp_step_value(&value, email, access_password),
                });
            }
            other => {
                return Err(format!(
                    "Codex CDP steps[{}].type 不支持: {}，仅支持 click / input",
                    index, other
                ));
            }
        }
    }

    Ok(parsed_steps)
}

// 递归复制目录的辅助函数
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap();
        let dst_path = dst.join(name);
        if path.is_dir() {
            copy_dir_all(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

// 复制 pyBuild 文件夹到应用目录
pub fn copy_pybuild_to_app_dir(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let app_dir = get_app_dir()?;
    let src_dir = app_dir.join("pyBuild");

    // 创建目标目录
    fs::create_dir_all(&src_dir).map_err(|e| format!("Failed to create directory: {}", e))?;

    // 复制资源文件到工作目录
    let resource_dir = app_handle.path().resource_dir().unwrap().join("pyBuild");
    if resource_dir.exists() {
        log_info!("Found resource directory at: {:?}", resource_dir);

        // 如果目标目录已存在，先删除它以实现覆盖
        if src_dir.exists() {
            fs::remove_dir_all(&src_dir)
                .map_err(|e| format!("Failed to remove existing directory: {}", e))?;
        }

        // 递归复制目录
        if let Err(e) = copy_dir_all(&resource_dir, &src_dir) {
            log_error!("Failed to copy resource directory: {}", e);
            return Err(format!("Failed to copy pyBuild directory: {}", e));
        }

        log_info!("Successfully copied pyBuild to: {:?}", src_dir);
        Ok(())
    } else {
        log_info!("Resource directory not found at: {:?}", resource_dir);
        Err("Resource directory not found".to_string())
    }
}

// 获取Python可执行文件路径的辅助函数
fn get_python_executable_path_by_name(executable_name: &str) -> Result<PathBuf, String> {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let exe_name = if cfg!(target_os = "windows") {
        format!("{}.exe", executable_name)
    } else {
        executable_name.to_string()
    };

    if cfg!(debug_assertions) {
        Ok(get_app_dir()?
            .join("pyBuild")
            .join(platform)
            .join(&exe_name))
    } else {
        let current_exe =
            std::env::current_exe().map_err(|e| format!("无法获取当前执行文件路径: {}", e))?;
        let exe_dir = current_exe.parent().ok_or("无法获取执行文件目录")?;
        Ok(exe_dir.join("pyBuild").join(platform).join(&exe_name))
    }
}

fn get_python_executable_path() -> Result<PathBuf, String> {
    get_python_executable_path_by_name("cursor_register")
}

fn decode_python_output(buffer: &[u8]) -> String {
    let decoded = match std::str::from_utf8(buffer) {
        Ok(text) => text.trim_end_matches(['\r', '\n']).to_string(),
        Err(_) => {
            let (decoded, _, _) = GBK.decode(buffer);
            decoded.trim_end_matches(['\r', '\n']).to_string()
        }
    };
    sanitize_python_output_line(&decoded)
}

fn sanitize_python_output_line(line: &str) -> String {
    let mut cleaned = ANSI_ESCAPE_RE.replace_all(line, "").to_string();
    cleaned = cleaned.replace('\u{FFFD}', "");
    cleaned = cleaned.trim_end_matches(['\r', '\n']).to_string();

    // 控制台编码降级时，emoji 常被替换成前缀 "? "，这里做展示层修正。
    if let Some(rest) = cleaned.strip_prefix("? ") {
        let rest = rest.trim_start();
        let strip_prefix =
            rest.starts_with('[') || rest.chars().next().map(|c| !c.is_ascii()).unwrap_or(false);
        if strip_prefix {
            cleaned = rest.to_string();
        }
    }

    cleaned
}

// 邮箱配置结构体
#[derive(Debug, Serialize, Deserialize, Clone)]
struct EmailConfig {
    worker_domain: String,
    email_domain: String,
    admin_password: String,
    access_password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HaozhumaPhoneFilters {
    isp: String,
    province: String,
    ascription: String,
    paragraph: String,
    exclude: String,
    uid: String,
    author: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HaozhumaRetryConfig {
    max_phone_retry: u64,
    poll_interval_seconds: u64,
    send_check_timeout_seconds: u64,
    sms_poll_timeout_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HaozhumaConfig {
    enabled: bool,
    api_domain: String,
    username: String,
    password: String,
    project_id: String,
    default_country_code: String,
    phone_filters: HaozhumaPhoneFilters,
    retry: HaozhumaRetryConfig,
}

// Cloudflare临时邮箱相关结构体
#[derive(Debug, Serialize, Deserialize)]
struct CloudflareEmailResponse {
    jwt: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CloudflareMailsResponse {
    results: Option<Vec<CloudflareMail>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CloudflareMail {
    raw: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CodexTokenFileInfo {
    file_name: String,
    file_path: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    content: String,
}

// Tempmail API 响应结构体
#[derive(Debug, Deserialize)]
struct TempmailResponse {
    result: bool,
    count: u32,
    mail_list: Vec<TempmailMail>,
}

#[derive(Debug, Deserialize)]
struct TempmailMail {
    mail_id: u64,
    from_mail: String,
    from_name: String,
    subject: String,
    time: String,
}

#[derive(Debug, Deserialize)]
struct TempmailDetailResponse {
    result: bool,
    mail_id: u64,
    from_mail: String,
    from_name: String,
    subject: String,
    html: String,
    text: String,
}

// 生成随机邮箱名称
fn generate_random_email_name() -> String {
    let letters1: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect::<String>()
        .to_lowercase();

    let numbers: String = (0..rand::thread_rng().gen_range(1..=3))
        .map(|_| rand::thread_rng().gen_range(0..10).to_string())
        .collect();

    let letters2: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(rand::thread_rng().gen_range(1..=3))
        .map(char::from)
        .collect::<String>()
        .to_lowercase();

    format!("{}{}{}", letters1, numbers, letters2)
}

// 创建临时邮箱
async fn create_cloudflare_temp_email() -> Result<(String, String), String> {
    let client = reqwest::Client::new();
    let random_name = generate_random_email_name();

    // 获取邮箱配置
    let email_config = get_email_config().await?;

    let url = format!("https://{}/admin/new_address", email_config.worker_domain);
    let payload = serde_json::json!({
        "enablePrefix": true,
        "name": random_name,
        "domain": email_config.email_domain,
    });

    log_debug!("创建邮箱请求详情:");
    log_debug!("  URL: {}", url);
    log_debug!(
        "  Headers: x-admin-auth=[hidden], x-custom-auth=[hidden], Content-Type=application/json"
    );
    log_debug!(
        "  Payload: {}",
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );

    let response = client
        .post(&url)
        .header("X-Admin-Auth", &email_config.admin_password)
        .header("x-Custom-Auth", &email_config.access_password)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("创建邮箱请求失败: {}", e))?;

    let status = response.status();
    let headers = response.headers().clone();

    log_debug!("响应详情:");
    log_debug!("  状态码: {}", status);
    log_debug!("  响应头: {:?}", headers);

    // 获取响应文本用于调试
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取响应文本失败: {}", e))?;

    log_info!("  响应体: {}", response_text);

    if status.is_success() {
        let data: CloudflareEmailResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("解析响应JSON失败: {} | 响应内容: {}", e, response_text))?;

        log_debug!("🔍 [DEBUG] 解析后的数据: {:?}", data);

        match (data.jwt, data.address) {
            (Some(jwt), Some(address)) => {
                log_info!("✅ 创建临时邮箱成功: {}", address);
                Ok((jwt, address))
            }
            _ => Err(format!(
                "响应中缺少JWT或邮箱地址 | 完整响应: {}",
                response_text
            )),
        }
    } else {
        Err(format!(
            "创建邮箱失败，状态码: {} | 响应内容: {}",
            status, response_text
        ))
    }
}

// 获取验证码
async fn get_verification_code_from_cloudflare(jwt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    // 获取邮箱配置
    let email_config = get_email_config().await?;

    // 最多尝试30次，每次等待10秒
    for attempt in 1..=30 {
        log_debug!("🔍 第{}次尝试获取验证码...", attempt);

        let url = format!("https://{}/api/mails", email_config.worker_domain);
        log_debug!("🔍 [DEBUG] 获取邮件请求详情:");
        log_info!("  URL: {}", url);
        log_info!("  Headers:");
        log_info!("    Authorization: Bearer {}", jwt);
        log_info!("    Content-Type: application/json");
        log_info!("  Query: limit=10&offset=0");

        let response = client
            .get(&url)
            .header("Authorization", &format!("Bearer {}", jwt))
            .header("Content-Type", "application/json")
            .query(&[("limit", "10"), ("offset", "0")])
            .send()
            .await
            .map_err(|e| format!("获取邮件请求失败: {}", e))?;

        let status = response.status();
        log_debug!("🔍 [DEBUG] 获取邮件响应状态码: {}", status);

        if response.status().is_success() {
            let response_text = response
                .text()
                .await
                .map_err(|e| format!("读取邮件响应文本失败: {}", e))?;

            // log_debug!("🔍 [DEBUG] 邮件响应体: {}", response_text);

            let data: CloudflareMailsResponse =
                serde_json::from_str(&response_text).map_err(|e| {
                    format!("解析邮件响应JSON失败: {} | 响应内容: {}", e, response_text)
                })?;

            // log_debug!("🔍 [DEBUG] 解析后的邮件数据: {:?}", data);

            if let Some(results) = data.results {
                log_debug!("🔍 [DEBUG] 邮件数量: {}", results.len());
                if !results.is_empty() {
                    if let Some(raw_content) = &results[0].raw {
                        if let Some(verification_code) =
                            extract_verification_code_from_mail_raw_body(raw_content)
                        {
                            log_info!("✅ 成功提取验证码: {}", verification_code);
                            return Ok(verification_code);
                        }

                        log_debug!("🔍 [DEBUG] 未找到匹配的验证码模式");
                    } else {
                        log_debug!("🔍 [DEBUG] 第一封邮件没有raw内容");
                    }
                } else {
                    log_debug!("🔍 [DEBUG] 邮件列表为空");
                }
            } else {
                log_debug!("🔍 [DEBUG] 响应中没有results字段");
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法读取错误响应".to_string());
            log_info!(
                "🔍 [DEBUG] 获取邮件失败，状态码: {} | 错误内容: {}",
                status,
                error_text
            );
        }

        // 等待10秒后重试
        log_info!("⏳ 等待10秒后重试...");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }

    Err("获取验证码超时".to_string())
}

// Tempmail API 函数
async fn get_verification_code_from_tempmail(
    tempmail_email: &str,
    pin: &str,
    register_email: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();

    // 最多10轮查找，每轮查10个邮件，间隔12秒（总共约2分钟）
    for round in 1..=10 {
        log_info!("🔍 第{}轮从tempmail获取验证码...", round);

        // 获取邮件列表
        let encoded_email = urlencoding::encode(tempmail_email);
        let url = format!(
            "https://tempmail.plus/api/mails?email={}&limit=10&epin={}",
            encoded_email, pin
        );

        log_debug!("🔍 [DEBUG] Tempmail请求详情:");
        log_info!("  URL: {}", url);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("获取tempmail邮件列表失败: {}", e))?;

        let status = response.status();
        log_debug!("🔍 [DEBUG] Tempmail响应状态码: {}", status);

        if response.status().is_success() {
            let response_text = response
                .text()
                .await
                .map_err(|e| format!("读取tempmail响应文本失败: {}", e))?;

            log_debug!("🔍 [DEBUG] Tempmail响应体: {}", response_text);

            let data: TempmailResponse = serde_json::from_str(&response_text).map_err(|e| {
                format!(
                    "解析tempmail响应JSON失败: {} | 响应内容: {}",
                    e, response_text
                )
            })?;

            if data.result && !data.mail_list.is_empty() {
                log_info!(
                    "🔍 第{}轮找到 {} 封邮件，开始检查验证码...",
                    round,
                    data.mail_list.len()
                );

                // 查找验证码（最多查看前10封邮件）
                for (index, mail) in data.mail_list.iter().take(10).enumerate() {
                    log_debug!(
                        "🔍 [DEBUG] 检查第{}封邮件: from={}, subject={}",
                        index + 1,
                        mail.from_mail,
                        mail.subject
                    );

                    // 获取邮件详情
                    let detail_url = format!(
                        "https://tempmail.plus/api/mails/{}?email={}&epin={}",
                        mail.mail_id, encoded_email, pin
                    );

                    log_debug!("🔍 [DEBUG] 获取邮件详情: {}", detail_url);

                    let detail_response = client
                        .get(&detail_url)
                        .send()
                        .await
                        .map_err(|e| format!("获取tempmail邮件详情失败: {}", e))?;

                    if detail_response.status().is_success() {
                        let detail_text = detail_response
                            .text()
                            .await
                            .map_err(|e| format!("读取tempmail详情响应失败: {}", e))?;

                        log_debug!("🔍 [DEBUG] 邮件详情响应: {}", detail_text);

                        let detail_data: TempmailDetailResponse =
                            serde_json::from_str(&detail_text).map_err(|e| {
                                format!(
                                    "解析tempmail详情JSON失败: {} | 响应内容: {}",
                                    e, detail_text
                                )
                            })?;

                        if detail_data.result {
                            // 检查HTML内容是否包含注册邮箱
                            if detail_data.html.contains(register_email) {
                                log_debug!(
                                    "🔍 [DEBUG] 第{}轮第{}封邮件包含注册邮箱 {}，开始提取验证码",
                                    round,
                                    index + 1,
                                    register_email
                                );

                                // 如果HTML中没有找到，尝试从text中提取
                                if let Ok(verification_code) =
                                    extract_verification_code_from_text(&detail_data.text)
                                {
                                    log_info!(
                                        "✅ 第{}轮第{}封邮件成功从文本提取验证码: {}",
                                        round,
                                        index + 1,
                                        verification_code
                                    );
                                    return Ok(verification_code);
                                }

                                // 从HTML内容中提取验证码
                                if let Some(verification_code) =
                                    extract_verification_code_from_content(&detail_data.html)
                                {
                                    log_info!(
                                        "✅ 第{}轮第{}封邮件成功提取验证码: {}",
                                        round,
                                        index + 1,
                                        verification_code
                                    );
                                    return Ok(verification_code);
                                }

                                log_debug!(
                                    "🔍 [DEBUG] 第{}轮第{}封邮件包含注册邮箱但未找到验证码",
                                    round,
                                    index + 1
                                );
                            } else {
                                log_debug!(
                                    "🔍 [DEBUG] 第{}轮第{}封邮件不包含注册邮箱 {}，跳过",
                                    round,
                                    index + 1,
                                    register_email
                                );
                            }
                        }
                    }
                }

                log_info!("🔍 第{}轮未找到验证码，本轮检查完毕", round);
            } else {
                log_info!("🔍 第{}轮暂无邮件或请求失败", round);
            }
        } else {
            log_debug!(
                "🔍 [DEBUG] 第{}轮Tempmail API请求失败，状态码: {}",
                round,
                status
            );
        }

        // 如果不是最后一轮，等待12秒后重试
        if round < 10 {
            log_info!("⏳ 等待12秒后进行第{}轮查找...", round + 1);
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;
        }
    }

    log_info!("❌ 10轮查找均未找到验证码（总时长约2分钟），需要用户手动输入");
    Err("从tempmail获取验证码失败，已尝试10轮查找（约2分钟），请手动输入验证码".to_string())
}

// 从文本内容中提取验证码
fn extract_verification_code_from_text(text: &str) -> Result<String, String> {
    // 尝试多种正则表达式模式
    let patterns = vec![
        // 匹配\n \n604152\n\n中的数字
        r"\n(\d{6})\n",
        r"\b(\d{6})\b", // 最后尝试任意6位数字
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(text) {
                if let Some(code) = captures.get(1) {
                    let verification_code = code.as_str().to_string();
                    log_debug!("✅ 从文本中提取到验证码: {}", verification_code);
                    return Ok(verification_code);
                }
            }
        }
    }

    Err("未能从文本中提取验证码".to_string())
}

// 从Outlook邮箱获取验证码
async fn get_verification_code_from_outlook(email: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let encoded_email = urlencoding::encode(email);

    // 最多尝试30次，每次等待10秒
    for attempt in 1..=30 {
        log_debug!("🔍 第{}次尝试从Outlook获取验证码...", attempt);

        // 获取收件箱邮件
        let inbox_url = format!(
            "http://query.paopaodw.com/api/GetLastEmails?email={}&boxType=1",
            encoded_email
        );
        log_debug!("🔍 [DEBUG] 获取收件箱邮件: {}", inbox_url);

        let inbox_response = client
            .get(&inbox_url)
            .send()
            .await
            .map_err(|e| format!("获取收件箱邮件失败: {}", e))?;

        if inbox_response.status().is_success() {
            let inbox_text = inbox_response
                .text()
                .await
                .map_err(|e| format!("读取收件箱响应失败: {}", e))?;

            log_debug!("🔍 [DEBUG] 收件箱响应: {}", inbox_text);

            if let Ok(inbox_data) = serde_json::from_str::<serde_json::Value>(&inbox_text) {
                if let Some(data) = inbox_data.get("data").and_then(|d| d.as_array()) {
                    for email_item in data {
                        if let Some(body) = email_item.get("Body").and_then(|b| b.as_str()) {
                            if let Some(code) = extract_verification_code_from_content(body) {
                                log_info!("✅ 从收件箱找到验证码: {}", code);
                                return Ok(code);
                            }
                        }
                    }
                }
            }
        }

        // 获取垃圾箱邮件
        let spam_url = format!(
            "http://query.paopaodw.com/api/GetLastEmails?email={}&boxType=2",
            encoded_email
        );
        log_debug!("🔍 [DEBUG] 获取垃圾箱邮件: {}", spam_url);

        let spam_response = client
            .get(&spam_url)
            .send()
            .await
            .map_err(|e| format!("获取垃圾箱邮件失败: {}", e))?;

        if spam_response.status().is_success() {
            let spam_text = spam_response
                .text()
                .await
                .map_err(|e| format!("读取垃圾箱响应失败: {}", e))?;

            log_debug!("🔍 [DEBUG] 垃圾箱响应: {}", spam_text);

            if let Ok(spam_data) = serde_json::from_str::<serde_json::Value>(&spam_text) {
                if let Some(data) = spam_data.get("data").and_then(|d| d.as_array()) {
                    for email_item in data {
                        if let Some(body) = email_item.get("Body").and_then(|b| b.as_str()) {
                            if let Some(code) = extract_verification_code_from_content(body) {
                                log_info!("✅ 从垃圾箱找到验证码: {}", code);
                                return Ok(code);
                            }
                        }
                    }
                }
            }
        }

        if attempt < 30 {
            log_info!("⏰ 第{}次尝试未找到验证码，等待10秒后重试...", attempt);
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    }

    Err("获取验证码超时，请检查邮箱或稍后重试".to_string())
}

// 提取验证码的通用函数（复用现有逻辑）
fn extract_verification_code_from_content(content: &str) -> Option<String> {
    use regex::Regex;

    // 最高优先：换行包围的独立 6 位数字（如 \n177872\r），避免先于域名等误匹配
    for pat in [
        r"\n(\d{6})\r",
        r"\r\n(\d{6})\r\n",
        r"\n(\d{6})\n",
        r"\r(\d{6})\r",
    ] {
        let re = Regex::new(pat).unwrap();
        if let Some(captures) = re.captures(content) {
            if let Some(code) = captures.get(1) {
                return Some(code.as_str().to_string());
            }
        }
    }

    // 使用现有的验证码提取逻辑
    let re1 = Regex::new(r"code is (\d{6})").unwrap();
    if let Some(captures) = re1.captures(content) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }

    // 第二种方式
    let re2 = Regex::new(r"验证码为：(\d{6})").unwrap();
    if let Some(captures) = re2.captures(content) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }

    // 第三种方式
    let re3 = Regex::new(r"verification code is: (\d{6})").unwrap();
    if let Some(captures) = re3.captures(content) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }

    // 第四种方式 - 更通用的6位数字匹配，先清洗明显干扰项再匹配
    // 1) 移除颜色代码（如 #414141）
    let color_code_regex = Regex::new(r"#([0-9a-fA-F]{6})\b").unwrap();
    let content_without_colors = color_code_regex.replace_all(content, "");

    // 2) 移除前面是 + 号的 6 位数字（如 +123456）
    let plus_regex = Regex::new(r"\+\d{6}\b").unwrap();
    let content_without_plus = plus_regex.replace_all(&content_without_colors, "");

    // 3) 排除「六位数字+点+域名」形态（如 801304.xyz），避免当作验证码
    let six_then_domain = Regex::new(r"\b\d{6}\.[a-zA-Z][a-zA-Z0-9.-]*").unwrap();
    let content_without_six_domain = six_then_domain.replace_all(&content_without_plus, "");

    // 2. 查找 6 位数字
    let re4 = Regex::new(r"\b(\d{6})\b").unwrap();
    if let Some(captures) = re4.captures(&content_without_six_domain) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }

    None
}

/// 从邮件原始正文提取验证码：先走通用 HTML/文本规则，再走与 Cloudflare `raw` 一致的清洗与匹配
fn extract_verification_code_from_mail_raw_body(raw: &str) -> Option<String> {
    if let Some(code) = extract_verification_code_from_content(raw) {
        return Some(code);
    }
    use regex::Regex;
    let re1 = Regex::new(r"code is (\d{6})").unwrap();
    if let Some(captures) = re1.captures(raw) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }
    let re2 = Regex::new(r"code is:\s*(\d{6})").unwrap();
    if let Some(captures) = re2.captures(raw) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }
    let color_code_regex = Regex::new(r"#([0-9a-fA-F]{6})\b").unwrap();
    let content_without_colors = color_code_regex.replace_all(raw, "");
    let plus_regex = Regex::new(r"\+\d{6}").unwrap();
    let content_without_plus = plus_regex.replace_all(&content_without_colors, "");
    let at_regex = Regex::new(r"@\d{6}").unwrap();
    let content_without_at = at_regex.replace_all(&content_without_plus, "");
    let equal_regex = Regex::new(r"=\d{6}").unwrap();
    let content_without_equal = equal_regex.replace_all(&content_without_at, "");
    // 同步通用规则：排除「六位数字+点+域名」片段，减少误识别（如 801304.xyz）
    let six_then_domain = Regex::new(r"\b\d{6}\.[a-zA-Z][a-zA-Z0-9.-]*").unwrap();
    let content_cleaned = six_then_domain.replace_all(&content_without_equal, "");
    let re3 = Regex::new(r"\b(\d{6})\b").unwrap();
    if let Some(captures) = re3.captures(&content_cleaned) {
        if let Some(code) = captures.get(1) {
            return Some(code.as_str().to_string());
        }
    }
    None
}

fn json_navigate_one_segment<'a>(
    v: &'a serde_json::Value,
    segment: &str,
) -> Result<&'a serde_json::Value, String> {
    let segment = segment.trim();
    if segment.is_empty() {
        return Err("JSON路径存在空段".to_string());
    }
    if let Some(open_bracket) = segment.find('[') {
        let key = segment[..open_bracket].trim();
        let close = segment
            .rfind(']')
            .ok_or_else(|| format!("路径段括号不匹配: {}", segment))?;
        let idx_str = segment[open_bracket + 1..close].trim();
        let idx: usize = idx_str
            .parse()
            .map_err(|_| format!("无效的数组下标: {}", idx_str))?;
        let parent = if key.is_empty() {
            v
        } else {
            v.get(key)
                .ok_or_else(|| format!("JSON路径不存在: {}", key))?
        };
        let arr = parent
            .as_array()
            .ok_or_else(|| format!("路径 {} 不是数组", if key.is_empty() { "根" } else { key }))?;
        arr.get(idx).ok_or_else(|| format!("数组下标 {} 越界", idx))
    } else if segment.chars().all(|c| c.is_ascii_digit()) {
        let idx: usize = segment
            .parse()
            .map_err(|_| format!("无效数字路径段: {}", segment))?;
        let arr = v
            .as_array()
            .ok_or_else(|| format!("当前节点不是数组，无法使用下标 {}", idx))?;
        arr.get(idx).ok_or_else(|| format!("数组下标 {} 越界", idx))
    } else {
        v.get(segment)
            .ok_or_else(|| format!("JSON路径不存在: {}", segment))
    }
}

fn json_value_at_path<'a>(
    root: &'a serde_json::Value,
    path: &str,
) -> Result<&'a serde_json::Value, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("响应JSON路径为空".to_string());
    }
    let mut cur = root;
    for segment in path.split('.').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        cur = json_navigate_one_segment(cur, segment)?;
    }
    Ok(cur)
}

fn json_value_to_mail_raw(v: &serde_json::Value) -> Result<String, String> {
    match v {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Null => Err("JSON路径指向 null".to_string()),
        _ => Err(format!("JSON路径须指向字符串（邮件原文），当前为: {:?}", v)),
    }
}

fn parse_self_hosted_headers(
    headers_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let headers_val: serde_json::Value = serde_json::from_str(headers_json.trim())
        .map_err(|e| format!("请求 Headers 须为合法 JSON 对象: {}", e))?;
    headers_val
        .as_object()
        .cloned()
        .ok_or_else(|| "请求 Headers 须为 JSON 对象（键值对）".to_string())
}

async fn execute_self_hosted_mail_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    headers_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<reqwest::Response, String> {
    let method = reqwest::Method::from_bytes(method.trim().to_uppercase().as_bytes())
        .map_err(|e| format!("请求 Method 无效: {}", e))?;

    let mut req = client.request(method, url.trim());
    for (k, val) in headers_obj {
        let header_value = match val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => continue,
            other => other.to_string().trim_matches('"').to_string(),
        };
        req = req.header(k, header_value);
    }

    req.send()
        .await
        .map_err(|e| format!("自建邮箱 API 请求失败: {}", e))
}

async fn maybe_clear_self_hosted_mailbox(
    client: &reqwest::Client,
    clear_enabled: bool,
    clear_url: Option<&str>,
    clear_headers_json: Option<&str>,
    clear_method: Option<&str>,
) -> Result<(), String> {
    if !clear_enabled {
        return Ok(());
    }

    let clear_url = clear_url.unwrap_or("").trim();
    if clear_url.is_empty() {
        return Err("已开启清空邮箱，但清空 URL 为空".to_string());
    }

    let clear_headers = parse_self_hosted_headers(clear_headers_json.unwrap_or("{}"))?;
    let clear_method = clear_method.unwrap_or("GET").trim();

    log_info!("🧹 [自建邮箱API] 获取验证码前先清空邮箱...");
    let response =
        execute_self_hosted_mail_request(client, clear_method, clear_url, &clear_headers).await?;
    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("读取清空邮箱响应失败: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "清空邮箱请求失败: HTTP {} {}",
            status,
            response_text.chars().take(200).collect::<String>()
        ));
    }

    log_info!("✅ [自建邮箱API] 清空邮箱请求成功");
    Ok(())
}

async fn get_verification_code_from_self_hosted_mail_api(
    url: &str,
    headers_json: &str,
    response_path: &str,
    register_email: Option<&str>,
    clear_enabled: bool,
    clear_url: Option<&str>,
    clear_headers_json: Option<&str>,
    clear_method: Option<&str>,
) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("自建邮箱 API URL 为空".to_string());
    }
    let path = response_path.trim().trim_matches('.');
    let path = if path.is_empty() {
        "results[0].raw"
    } else {
        path
    };

    let headers_obj = parse_self_hosted_headers(headers_json)?;
    let client = reqwest::Client::new();
    let register_email = register_email.unwrap_or("").trim().to_lowercase();

    maybe_clear_self_hosted_mailbox(
        &client,
        clear_enabled,
        clear_url,
        clear_headers_json,
        clear_method,
    )
    .await?;

    // 首次请求前等待，避免 API 仍返回上一封邮件（新验证码邮件尚未出现在列表首位）
    log_info!("⏳ [自建邮箱API] 首次拉取前等待 30 秒，避免误用旧邮件…");
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

    for attempt in 1..=30 {
        log_debug!("🔍 [自建邮箱API] 第{}次拉取邮件...", attempt);

        let response = execute_self_hosted_mail_request(&client, "GET", url, &headers_obj).await?;
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| format!("读取 API 响应失败: {}", e))?;

        if !status.is_success() {
            log_debug!(
                "🔍 [自建邮箱API] HTTP {}: {}",
                status,
                response_text.chars().take(200).collect::<String>()
            );
        } else if let Ok(root) = serde_json::from_str::<serde_json::Value>(&response_text) {
            match json_value_at_path(&root, path) {
                Ok(leaf) => match json_value_to_mail_raw(leaf) {
                    Ok(raw) => {
                        let mut raw_candidate = raw;
                        if !register_email.is_empty() {
                            let local = register_email
                                .split('@')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            let is_email_matched = |text: &str| {
                                let text_lower = text.to_lowercase();
                                text_lower.contains(&register_email)
                                    || (local.len() >= 3 && text_lower.contains(&local))
                            };

                            let email_matched = is_email_matched(&raw_candidate);
                            if !email_matched {
                                log_debug!(
                                    "🔍 [自建邮箱API] 命中邮件但邮箱不匹配，立即二次拉取（expect={}）",
                                    register_email
                                );
                                // 不匹配时立即再拉一次，避免并行场景下取到别人的同批邮件。
                                let retry_response = execute_self_hosted_mail_request(
                                    &client,
                                    "GET",
                                    url,
                                    &headers_obj,
                                )
                                .await?;
                                let retry_status = retry_response.status();
                                let retry_text = retry_response
                                    .text()
                                    .await
                                    .map_err(|e| format!("读取二次拉取响应失败: {}", e))?;
                                if retry_status.is_success() {
                                    if let Ok(retry_root) =
                                        serde_json::from_str::<serde_json::Value>(&retry_text)
                                    {
                                        if let Ok(retry_leaf) =
                                            json_value_at_path(&retry_root, path)
                                        {
                                            if let Ok(retry_raw) =
                                                json_value_to_mail_raw(retry_leaf)
                                            {
                                                if is_email_matched(&retry_raw) {
                                                    raw_candidate = retry_raw;
                                                } else {
                                                    log_debug!(
                                                        "🔍 [自建邮箱API] 二次拉取仍未匹配到目标邮箱，继续轮询（expect={}）",
                                                        register_email
                                                    );
                                                    continue;
                                                }
                                            } else {
                                                continue;
                                            }
                                        } else {
                                            continue;
                                        }
                                    } else {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            }
                        }
                        if let Some(code) = extract_verification_code_from_content(&raw_candidate) {
                            log_info!("✅ [自建邮箱API] 提取验证码成功: {}", code);
                            return Ok(code);
                        }
                        log_debug!(
                            "🔍 [自建邮箱API] raw 中未匹配到验证码（仅 code is / 验证码为 / verification code / 清洗后6位）"
                        );
                    }
                    Err(e) => log_debug!("🔍 [自建邮箱API] 路径值无效: {}", e),
                },
                Err(e) => log_debug!("🔍 [自建邮箱API] JSON路径解析失败: {}", e),
            }
        } else {
            log_debug!("🔍 [自建邮箱API] 响应不是合法 JSON");
        }

        if attempt < 30 {
            log_info!("⏳ [自建邮箱API] 等待10秒后重试...");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    }

    Err("自建邮箱 API 获取验证码超时".to_string())
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_available_backups() -> Result<Vec<BackupInfo>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .find_backups()
        .map_err(|e| format!("Failed to find backups: {}", e))
}

#[tauri::command]
async fn extract_backup_ids(backup_path: String) -> Result<MachineIds, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .extract_ids_from_backup(&backup_path)
        .map_err(|e| format!("Failed to extract IDs from backup: {}", e))
}

#[tauri::command]
async fn flash_window(window: Window) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{FLASHW_ALL, FLASHW_TIMERNOFG, FlashWindow};

        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                // 闪烁窗口直到用户关注
                FlashWindow(hwnd.0 as _, (FLASHW_ALL | FLASHW_TIMERNOFG) as i32);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::{NSApp, NSRequestUserAttentionType};
        use cocoa::base::nil;
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        unsafe {
            let app: *mut Object = NSApp();
            if app != nil {
                log_info!("🍎 [macOS] Requesting user attention - Dock icon should bounce");

                // 使用 objc 的 msg_send! 宏来正确调用 requestUserAttention 方法
                let _: i32 = msg_send![app, requestUserAttention: NSRequestUserAttentionType::NSCriticalRequest];

                log_info!(
                    "🍎 [macOS] User attention request sent successfully - Dock should be bouncing!"
                );
            } else {
                log_error!("🍎 [macOS] Failed to get NSApp instance");
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux上尝试请求用户注意
        // 由于Tauri在Linux上的限制，我们使用一个简单的实现
        log_info!("Flash window requested on Linux (limited support)");
    }

    Ok(())
}

#[tauri::command]
async fn delete_backup(backup_path: String) -> Result<serde_json::Value, String> {
    use std::fs;

    match fs::remove_file(&backup_path) {
        Ok(_) => {
            log_info!("✅ 成功删除备份文件: {}", backup_path);
            Ok(serde_json::json!({
                "success": true,
                "message": "备份文件删除成功"
            }))
        }
        Err(e) => {
            log_error!("❌ 删除备份文件失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("删除失败: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn restore_machine_ids(backup_path: String) -> Result<RestoreResult, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    let mut details = Vec::new();
    let mut success = true;

    // Extract IDs from backup
    let ids = match restorer.extract_ids_from_backup(&backup_path) {
        Ok(ids) => {
            details.push("Successfully extracted IDs from backup".to_string());
            ids
        }
        Err(e) => {
            return Ok(RestoreResult {
                success: false,
                message: format!("Failed to extract IDs from backup: {}", e),
                details,
            });
        }
    };

    // Create backup of current state
    match restorer.create_backup() {
        Ok(backup_path) => {
            details.push(format!("Created backup at: {}", backup_path));
        }
        Err(e) => {
            details.push(format!("Warning: Failed to create backup: {}", e));
        }
    }

    // Update storage file
    if let Err(e) = restorer.update_storage_file(&ids) {
        success = false;
        details.push(format!("Failed to update storage file: {}", e));
    } else {
        details.push("Successfully updated storage.json".to_string());
    }

    // Update SQLite database (simplified version)
    match restorer.update_sqlite_db(&ids) {
        Ok(sqlite_results) => {
            details.extend(sqlite_results);
        }
        Err(e) => {
            details.push(format!("Warning: Failed to update SQLite database: {}", e));
        }
    }

    // Update machine ID file
    if let Err(e) = restorer.update_machine_id_file(&ids.dev_device_id) {
        details.push(format!("Warning: Failed to update machine ID file: {}", e));
    } else {
        details.push("Successfully updated machine ID file".to_string());
    }

    // Update system IDs
    match restorer.update_system_ids(&ids) {
        Ok(system_results) => {
            details.extend(system_results);
        }
        Err(e) => {
            details.push(format!("Warning: Failed to update system IDs: {}", e));
        }
    }

    let message = if success {
        "Machine IDs restored successfully".to_string()
    } else {
        "Machine ID restoration completed with some errors".to_string()
    };

    Ok(RestoreResult {
        success,
        message,
        details,
    })
}

#[tauri::command]
async fn get_cursor_paths() -> Result<(String, String), String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    Ok((
        restorer.db_path.to_string_lossy().to_string(),
        restorer.sqlite_path.to_string_lossy().to_string(),
    ))
}

#[tauri::command]
async fn check_cursor_installation() -> Result<bool, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    Ok(restorer.db_path.exists() || restorer.sqlite_path.exists())
}

#[tauri::command]
async fn get_cursor_version() -> Result<String, String> {
    cursor_info::get_cursor_version().await
}

#[tauri::command]
async fn reset_machine_ids() -> Result<ResetResult, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .reset_machine_ids()
        .map_err(|e| format!("Failed to reset machine IDs: {}", e))
}

#[tauri::command]
async fn complete_cursor_reset() -> Result<ResetResult, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .complete_cursor_reset()
        .map_err(|e| format!("Failed to complete Cursor reset: {}", e))
}

#[tauri::command]
async fn get_log_file_path() -> Result<String, String> {
    if let Some(log_path) = logger::Logger::get_log_path() {
        Ok(log_path.to_string_lossy().to_string())
    } else {
        Err("Logger not initialized".to_string())
    }
}

#[tauri::command]
async fn get_log_config() -> Result<serde_json::Value, String> {
    let (max_size_mb, log_file_name) = logger::get_log_config();
    Ok(serde_json::json!({
        "max_size_mb": max_size_mb,
        "log_file_name": log_file_name,
        "log_file_path": logger::Logger::get_log_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Not initialized".to_string())
    }))
}

// ==================== Web日志相关命令 ====================

#[tauri::command]
async fn write_weblog(
    level: String,
    message: String,
    url: Option<String>,
    user_agent: Option<String>,
    stack: Option<String>,
) -> Result<String, String> {
    let entry = weblog::WebLogEntry {
        level,
        message,
        url,
        user_agent,
        stack,
        timestamp: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
    };

    weblog::WebLogger::write_weblog(&entry);
    Ok("Web log written successfully".to_string())
}

#[tauri::command]
async fn get_weblog_file_path() -> Result<String, String> {
    if let Some(log_path) = weblog::WebLogger::get_weblog_path() {
        Ok(log_path.to_string_lossy().to_string())
    } else {
        Err("Web logger not initialized".to_string())
    }
}

#[tauri::command]
async fn get_weblog_config() -> Result<serde_json::Value, String> {
    let (max_size_mb, log_file_name) = weblog::get_weblog_config();
    Ok(serde_json::json!({
        "max_size_mb": max_size_mb,
        "log_file_name": log_file_name,
        "log_file_path": weblog::WebLogger::get_weblog_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Not initialized".to_string())
    }))
}

#[tauri::command]
async fn get_recent_weblogs(limit: usize) -> Result<Vec<String>, String> {
    weblog::WebLogger::get_recent_weblogs(limit)
        .map_err(|e| format!("Failed to get recent web logs: {}", e))
}

#[tauri::command]
async fn test_logging() -> Result<String, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .test_logging()
        .map_err(|e| format!("Failed to test logging: {}", e))
}

#[tauri::command]
async fn debug_windows_cursor_paths() -> Result<Vec<String>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .debug_windows_cursor_paths()
        .map_err(|e| format!("Failed to debug Windows cursor paths: {}", e))
}

#[tauri::command]
async fn set_custom_cursor_path(path: String) -> Result<String, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .set_custom_cursor_path(&path)
        .map_err(|e| format!("Failed to set custom cursor path: {}", e))
}

#[tauri::command]
async fn get_custom_cursor_path() -> Result<Option<String>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    Ok(restorer.get_custom_cursor_path())
}

#[tauri::command]
async fn clear_custom_cursor_path() -> Result<String, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .clear_custom_cursor_path()
        .map_err(|e| format!("Failed to clear custom cursor path: {}", e))
}

// 浏览器路径管理相关命令
#[tauri::command]
async fn select_browser_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .set_title("选择浏览器可执行文件")
        .add_filter("可执行文件", &["exe"])
        .blocking_pick_file();

    Ok(file_path.map(|path| path.to_string()))
}

#[tauri::command]
async fn set_custom_browser_path(path: String) -> Result<String, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .set_custom_browser_path(&path)
        .map_err(|e| format!("Failed to set custom browser path: {}", e))
}

#[tauri::command]
async fn get_custom_browser_path() -> Result<Option<String>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    Ok(restorer.get_custom_browser_path())
}

#[tauri::command]
async fn clear_custom_browser_path() -> Result<String, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .clear_custom_browser_path()
        .map_err(|e| format!("Failed to clear custom browser path: {}", e))
}

#[tauri::command]
async fn open_log_file() -> Result<String, String> {
    // 使用新的日志系统获取日志文件路径
    let log_path = if let Some(path) = logger::Logger::get_log_path() {
        path
    } else {
        return Err("日志系统未初始化".to_string());
    };

    // 检查日志文件是否存在
    if !log_path.exists() {
        return Err("日志文件不存在，请先运行应用以生成日志".to_string());
    }

    let log_path_str = log_path.to_string_lossy().to_string();

    // 根据操作系统打开文件
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", &log_path_str])
            .spawn()
            .map_err(|e| format!("Failed to open log file: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(&log_path_str)
            .spawn()
            .map_err(|e| format!("Failed to open log file: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(&log_path_str)
            .spawn()
            .map_err(|e| format!("Failed to open log file: {}", e))?;
    }

    Ok(format!("已打开日志文件: {}", log_path_str))
}

#[tauri::command]
async fn open_log_directory() -> Result<String, String> {
    // 使用新的日志系统获取日志文件路径
    let log_path = if let Some(path) = logger::Logger::get_log_path() {
        path
    } else {
        return Err("日志系统未初始化".to_string());
    };

    let log_dir = log_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let log_dir_str = log_dir.to_string_lossy().to_string();

    // 根据操作系统打开目录
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg(&log_dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open log directory: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(&log_dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open log directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xdg-open")
            .arg(&log_dir_str)
            .spawn()
            .map_err(|e| format!("Failed to open log directory: {}", e))?;
    }

    Ok(format!("已打开日志目录: {}", log_dir_str))
}

#[tauri::command]
async fn get_current_machine_ids() -> Result<Option<MachineIds>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .get_current_machine_ids()
        .map_err(|e| format!("Failed to get current machine IDs: {}", e))
}

#[tauri::command]
async fn get_machine_id_file_content() -> Result<Option<String>, String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .get_machine_id_file_content()
        .map_err(|e| format!("Failed to get machine ID file content: {}", e))
}

#[tauri::command]
async fn get_backup_directory_info() -> Result<(String, Vec<String>), String> {
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("Failed to initialize restorer: {}", e))?;

    restorer
        .get_backup_directory_info()
        .map_err(|e| format!("Failed to get backup directory info: {}", e))
}

#[tauri::command]
async fn check_user_authorization(token: String) -> Result<AuthCheckResult, String> {
    AuthChecker::check_user_authorized(&token)
        .await
        .map_err(|e| format!("Failed to check user authorization: {}", e))
}

#[tauri::command]
async fn get_user_info(token: String) -> Result<AuthCheckResult, String> {
    AuthChecker::get_user_info(&token)
        .await
        .map_err(|e| format!("Failed to get user info: {}", e))
}

#[tauri::command]
async fn get_subscription_info_only(token: String) -> Result<AuthCheckResult, String> {
    AuthChecker::get_subscription_info_only(&token)
        .await
        .map_err(|e| format!("Failed to get subscription info: {}", e))
}

#[tauri::command]
async fn get_current_period_usage(token: String) -> Result<serde_json::Value, String> {
    other::get_current_period_usage_impl(&token)
        .await
        .map_err(|e| format!("Failed to get current period usage: {}", e))
}

#[tauri::command]
async fn get_token_auto() -> Result<TokenInfo, String> {
    Ok(AuthChecker::get_token_auto())
}

#[tauri::command]
async fn list_codex_token_files() -> Result<Vec<CodexTokenFileInfo>, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not find user home directory".to_string())?;
    let token_dir = home.join(".auto-cursor-vip").join("codex_tokens");

    if !token_dir.exists() {
        return Ok(Vec::new());
    }

    let mut items: Vec<CodexTokenFileInfo> = Vec::new();
    let read_dir =
        fs::read_dir(&token_dir).map_err(|e| format!("Failed to read codex token dir: {}", e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Failed to read codex token entry: {}", e))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if !file_name.starts_with("token_") || !file_name.ends_with(".json") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read token file {}: {}", file_name, e))?;
        let metadata = fs::metadata(&path)
            .map_err(|e| format!("Failed to read metadata for {}: {}", file_name, e))?;
        let created_at_unix = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let updated_at_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        items.push(CodexTokenFileInfo {
            file_name: file_name.to_string(),
            file_path: path.to_string_lossy().to_string(),
            created_at_unix,
            updated_at_unix,
            content,
        });
    }

    items.sort_by(|a, b| b.updated_at_unix.cmp(&a.updated_at_unix));
    Ok(items)
}

#[tauri::command]
async fn debug_cursor_paths() -> Result<Vec<String>, String> {
    AuthChecker::debug_cursor_paths().map_err(|e| format!("Failed to debug cursor paths: {}", e))
}

// Account Management Commands
#[tauri::command]
async fn get_account_list() -> Result<AccountListResult, String> {
    Ok(AccountManager::get_account_list())
}

#[tauri::command]
async fn add_account(
    email: String,
    token: String,
    refresh_token: Option<String>,
    workos_cursor_session_token: Option<String>,
) -> Result<serde_json::Value, String> {
    match AccountManager::add_account(
        email.clone(),
        token,
        refresh_token,
        workos_cursor_session_token,
    ) {
        Ok(()) => Ok(serde_json::json!({
            "success": true,
            "message": format!("Account {} added successfully", email)
        })),
        Err(e) => {
            let error_msg = e.to_string();
            // 如果是账号已存在的错误，返回 success: true
            if error_msg.contains("Account with this email already exists") {
                Ok(serde_json::json!({
                    "success": true,
                    "message": format!("Failed to add account: {}", error_msg)
                }))
            } else {
                Ok(serde_json::json!({
                    "success": false,
                    "message": format!("Failed to add account: {}", error_msg)
                }))
            }
        }
    }
}

#[tauri::command]
async fn switch_account(
    email: String,
    auto_restart: Option<bool>,
    reset_machine_id: Option<bool>,
) -> Result<SwitchAccountResult, String> {
    log_info!(
        "🔄 Starting account switch to: {}, reset_machine_id: {:?}",
        email,
        reset_machine_id
    );

    // 检查是否启用了无感换号
    log_info!("🔍 [DEBUG] 开始检查无感换号状态...");
    let seamless_enabled = match MachineIdRestorer::check_seamless_switch_status() {
        Ok(enabled) => {
            log_info!("🔍 [DEBUG] 无感换号状态检查完成: {}", enabled);
            enabled
        }
        Err(e) => {
            log_warn!("⚠️ [DEBUG] 检查无感换号状态失败: {}, 使用传统切换", e);
            false
        }
    };

    if seamless_enabled {
        log_info!("✨ [DEBUG] 使用无感换号模式切换账户");

        // 获取账户列表找到目标账户的token
        let accounts = match AccountManager::load_accounts() {
            Ok(accounts) => accounts,
            Err(e) => {
                return Ok(SwitchAccountResult {
                    success: false,
                    message: format!("Failed to load accounts: {}", e),
                    details: vec![e.to_string()],
                });
            }
        };

        let target_account = match accounts.iter().find(|acc| acc.email == email) {
            Some(account) => account,
            None => {
                return Ok(SwitchAccountResult {
                    success: false,
                    message: "Account not found".to_string(),
                    details: vec![format!("No account found with email: {}", email)],
                });
            }
        };

        // 先调用传统切换方式（auto_restart强制为false，不重启Cursor）
        log_info!("🔄 [DEBUG] 无感换号模式：先执行传统切换（不重启）");
        let switch_result = AccountManager::switch_account(email.clone(), false);

        if !switch_result.success {
            log_error!("❌ [DEBUG] 传统切换失败: {}", switch_result.message);
            return Ok(switch_result);
        }

        log_info!("✅ [DEBUG] 传统切换成功，继续更新Web配置");

        // 更新Web配置文件，触发无感换号 (手动切换模式)
        // 默认重置机器ID以确保兼容性
        let should_reset = reset_machine_id.unwrap_or(true);
        if let Err(e) = NextWorkWebServer::update_seamless_switch_token(
            target_account.token.clone(),
            email.clone(),
            false,
            should_reset,
        )
        .await
        {
            log_error!("❌ [DEBUG] 更新无感换号配置失败: {}", e);
            return Ok(SwitchAccountResult {
                success: false,
                message: format!("无感换号更新失败: {}", e),
                details: vec![e],
            });
        }

        log_info!("✅ [DEBUG] 无感换号配置已更新，Cursor将自动切换到新账户");

        Ok(SwitchAccountResult {
            success: true,
            message: format!("无感换号已触发，切换到账户: {}", email),
            details: vec![
                "使用无感换号模式".to_string(),
                "已执行传统切换（Storage文件已更新）".to_string(),
                "Web配置已更新".to_string(),
                "Cursor将自动检测并切换账户".to_string(),
                "无需重启Cursor".to_string(),
            ],
        })
    } else {
        log_info!("🔄 [DEBUG] 使用传统切换模式");
        Ok(AccountManager::switch_account(
            email,
            auto_restart.unwrap_or(true),
        ))
    }
}

#[tauri::command]
async fn switch_account_with_token(
    email: String,
    token: String,
    auth_type: Option<String>,
) -> Result<SwitchAccountResult, String> {
    Ok(AccountManager::switch_account_with_token(
        email, token, auth_type,
    ))
}

#[tauri::command]
async fn edit_account(
    email: String,
    new_token: Option<String>,
    new_refresh_token: Option<String>,
    new_workos_cursor_session_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "🔍 [DEBUG] edit_account called with email: {}, new_token: {:?}, new_refresh_token: {:?}, new_workos_cursor_session_token: {:?}",
        email,
        new_token
            .as_ref()
            .map(|t| format!("{}...", &t[..t.len().min(10)])),
        new_refresh_token
            .as_ref()
            .map(|t| format!("{}...", &t[..t.len().min(10)])),
        new_workos_cursor_session_token
            .as_ref()
            .map(|t| format!("{}...", &t[..t.len().min(10)]))
    );

    match AccountManager::edit_account(
        email.clone(),
        new_token,
        new_refresh_token,
        new_workos_cursor_session_token,
    ) {
        Ok(()) => {
            log_info!("✅ [DEBUG] Account {} updated successfully", email);
            Ok(serde_json::json!({
                "success": true,
                "message": format!("Account {} updated successfully", email)
            }))
        }
        Err(e) => {
            log_error!("❌ [DEBUG] Failed to update account {}: {}", email, e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("Failed to update account: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn update_account_custom_tags(
    email: String,
    custom_tags: Vec<account_manager::CustomTag>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "🔍 [DEBUG] update_account_custom_tags called for email: {}",
        email
    );

    match AccountManager::update_custom_tags(email.clone(), custom_tags) {
        Ok(()) => {
            log_info!("✅ [DEBUG] Custom tags updated successfully for {}", email);
            Ok(serde_json::json!({
                "success": true,
                "message": format!("Custom tags updated successfully for {}", email)
            }))
        }
        Err(e) => {
            log_error!(
                "❌ [DEBUG] Failed to update custom tags for {}: {}",
                email,
                e
            );
            Ok(serde_json::json!({
                "success": false,
                "message": format!("Failed to update custom tags: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn remove_account(email: String) -> Result<serde_json::Value, String> {
    match AccountManager::remove_account(email.clone()) {
        Ok(()) => Ok(serde_json::json!({
            "success": true,
            "message": format!("Account {} removed successfully", email)
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("Failed to remove account: {}", e)
        })),
    }
}

#[tauri::command]
async fn logout_current_account() -> Result<LogoutResult, String> {
    Ok(AccountManager::logout_current_account())
}

#[tauri::command]
async fn export_accounts(export_path: String) -> Result<serde_json::Value, String> {
    match AccountManager::export_accounts(export_path) {
        Ok(exported_path) => Ok(serde_json::json!({
            "success": true,
            "message": format!("账户导出成功: {}", exported_path),
            "exported_path": exported_path
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("导出失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn import_accounts(import_file_path: String) -> Result<serde_json::Value, String> {
    match AccountManager::import_accounts(import_file_path) {
        Ok(message) => Ok(serde_json::json!({
            "success": true,
            "message": message
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("导入失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn open_cancel_subscription_page(
    app: tauri::AppHandle,
    workos_cursor_session_token: String,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 Opening cancel subscription page with WorkOS token...");

    let url = "https://cursor.com/dashboard?tab=billing";

    // 先尝试关闭已存在的窗口
    if let Some(existing_window) = app.get_webview_window("cancel_subscription") {
        log_info!("🔄 Closing existing cancel subscription window...");
        if let Err(e) = existing_window.close() {
            log_error!("❌ Failed to close existing window: {}", e);
        } else {
            log_info!("✅ Existing window closed successfully");
        }
        // 等待一小段时间确保窗口完全关闭
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 创建新的 WebView 窗口（默认隐藏）
    let app_handle = app.clone();
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app,
        "cancel_subscription",
        tauri::WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Cursor - 取消订阅")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .initialization_script(&format!(
        r#"
        // 在页面加载前设置 Cookie
        document.cookie = 'WorkosCursorSessionToken={}; domain=.cursor.com; path=/; secure; samesite=none';
        console.log('Cookie injected via initialization script');
        
        // 可选：检查 Cookie 是否设置成功
        console.log('Current cookies:', document.cookie);
        "#,
        workos_cursor_session_token
    ))
    .on_page_load(move |_window, _payload| {
        // 在页面加载完成时注入 Cookie
        let cus_script = r#"
            function findAndClickCancelButton () {
            console.log('Current page URL:', window.location.href);

            const manBtn = document.querySelector('.dashboard-outline-button') || document.querySelector('.dashboard-outline-button-medium')
            if (manBtn) {
                console.log('找到了');
                manBtn.click();
                setTimeout(() => {
                manBtn.click();
                setTimeout(() => {
                    manBtn.click();
                }, 1000)
                }, 1000)
                setTimeout(() => {
                window.__TAURI_INTERNALS__.invoke('show_cancel_subscription_window');
                }, 1500)
            } else {
                if (location.href.includes('dashboard')) {
                window.__TAURI_INTERNALS__.invoke('cancel_subscription_failed');
                console.log('没找到按钮');
                }
            }
            }
            if (document.readyState === 'complete') {
            console.log('页面已经加载完成');
            setTimeout(() => {
                findAndClickCancelButton()
            }, 2500)
            } else {
            // 监听页面加载完成事件
            window.addEventListener('load', function () {
                console.log('window load 事件触发');
                setTimeout(() => {
                findAndClickCancelButton()
                }, 2500)
            });
            }
            "#;
        
        if let Err(e) = _window.eval(cus_script) {
            log_error!("❌ Failed to inject page load: {}", e);
        } else {
            log_info!("✅ Page load injected successfully on page load");
        }
    })
    .visible(true) // 默认隐藏窗口
    .build();

    match webview_window {
        Ok(window) => {
            // 添加窗口关闭事件监听器
            let app_handle_clone = app_handle.clone();
            window.on_window_event(move |event| {
                match event {
                    tauri::WindowEvent::CloseRequested { .. } => {
                        log_info!("🔄 Cancel subscription window close requested by user");
                        // 用户手动关闭窗口时，调用失败处理
                        let app_handle_clone = app_handle_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = cancel_subscription_failed(app_handle_clone).await {
                                log_error!("❌ Failed to handle window close: {}", e);
                            }
                        });
                    }
                    tauri::WindowEvent::Destroyed => {
                        log_info!("🔄 Cancel subscription window destroyed");
                    }
                    _ => {}
                }
            });

            log_info!("✅ Successfully opened WebView window");
            Ok(serde_json::json!({
                "success": true,
                "message": "已打开取消订阅页面，正在自动登录..."
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to create WebView window: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("无法打开内置浏览器: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn show_cancel_subscription_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("cancel_subscription") {
        // 延迟1500ms再显示窗口
        tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

        window
            .show()
            .map_err(|e| format!("Failed to show window: {}", e))?;
        log_info!("✅ Cancel subscription window shown");

        // 发送事件通知前端操作成功
        if let Err(e) = app.emit("cancel-subscription-success", ()) {
            log_error!("❌ Failed to emit success event: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
async fn cancel_subscription_failed(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("cancel_subscription") {
        window
            .close()
            .map_err(|e| format!("Failed to close window: {}", e))?;
        log_error!("❌ Cancel subscription failed, window closed");

        // 发送事件通知前端操作失败
        if let Err(e) = app.emit("cancel-subscription-failed", ()) {
            log_error!("❌ Failed to emit failed event: {}", e);
        }
    }
    Ok(())
}

// 纯函数：获取绑卡链接
async fn get_bind_card_url_internal(
    workos_cursor_session_token: String,
    subscription_tier: Option<String>,
    allow_automatic_payment: Option<bool>,
    allow_trial: Option<bool>,
) -> Result<String, String> {
    use reqwest::header::{COOKIE, HeaderMap, HeaderValue};

    log_info!("🔄 Fetching bind card URL from Cursor API...");

    // 构建请求头
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&format!(
            "WorkosCursorSessionToken={}",
            workos_cursor_session_token
        ))
        .map_err(|e| format!("Failed to create cookie header: {}", e))?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    // 构建请求体
    let tier = subscription_tier.as_deref().unwrap_or("pro");
    let allow_trial_val = allow_trial.unwrap_or(true);
    let allow_automatic_payment_val = allow_automatic_payment.unwrap_or(true);

    let body = serde_json::json!({
        "tier": tier,
        "allowTrial": allow_trial_val,
        "allowAutomaticPayment": allow_automatic_payment_val
    });

    // 创建 HTTP 客户端
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // 发送 POST 请求
    log_info!("📤 Sending POST request to https://cursor.com/api/checkout");
    let response = client
        .post("https://cursor.com/api/checkout")
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    // 检查响应状态
    let status = response.status();
    log_info!("📥 Received response with status: {}", status);

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        log_error!("❌ API request failed: {} - {}", status, error_text);
        return Err(format!("API request failed: {} - {}", status, error_text));
    }

    // 获取响应文本（直接就是URL，可能带引号）
    let mut url = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // 去除可能存在的引号
    url = url.trim().trim_matches('"').to_string();

    log_info!("✅ Successfully got bind card URL: {}", url);

    // 检查是否返回的是 dashboard 页面（说明已经绑卡）
    if url.contains("cursor.com/dashboard") {
        log_error!("❌ 返回的是 dashboard 页面，该账户可能已经绑卡");
        return Err(
            "该账户可能已经绑定过银行卡，无法再次绑卡。如需更换银行卡，请先取消订阅后再试。"
                .to_string(),
        );
    }

    // 检查是否是 Stripe checkout URL
    if !url.contains("checkout.stripe.com") {
        log_error!("❌ 返回的不是有效的 Stripe checkout URL: {}", url);
        return Err(format!("返回的不是有效的绑卡链接: {}", url));
    }

    Ok(url)
}

// 供Python脚本调用的获取绑卡URL函数
#[tauri::command]
async fn get_bind_card_url_for_python(
    workos_cursor_session_token: String,
    subscription_tier: Option<String>,
    allow_automatic_payment: Option<bool>,
    allow_trial: Option<bool>,
) -> Result<String, String> {
    get_bind_card_url_internal(
        workos_cursor_session_token,
        subscription_tier,
        allow_automatic_payment,
        allow_trial,
    )
    .await
}

// 纯函数：确认宽限期免责声明
async fn acknowledge_grace_period_internal(
    workos_cursor_session_token: String,
) -> Result<String, String> {
    use reqwest::header::{HeaderMap, HeaderValue};

    log_info!(
        "🔄 Acknowledging grace period disclaimer...{}",
        workos_cursor_session_token
    );

    // 构建请求头
    let mut headers = HeaderMap::new();
    headers.insert("Origin", HeaderValue::from_static("https://cursor.com"));
    // 使用传入的 WorkosCursorSessionToken
    let cookie_value = format!("WorkosCursorSessionToken={}", workos_cursor_session_token);
    log_info!(
        "🔍 [DEBUG] Using WorkosCursorSessionToken: {}...",
        &workos_cursor_session_token[..workos_cursor_session_token.len().min(50)]
    );
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie_value).map_err(|e| format!("Invalid cookie value: {}", e))?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    // 创建 HTTP 客户端
    let client = reqwest::Client::new();

    // 发送 POST 请求
    log_info!(
        "📤 Sending POST request to https://cursor.com/api/dashboard/web-acknowledge-grace-period-disclaimer"
    );
    let body = serde_json::json!({});
    match client
        .post("https://cursor.com/api/dashboard/web-acknowledge-grace-period-disclaimer")
        .headers(headers)
        .json(&body)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let headers_map: std::collections::HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            log_debug!("📥 API 响应状态: {}", status);
            log_debug!("📥 响应头: {:?}", headers_map);

            match response.text().await {
                Ok(body) => {
                    log_debug!("📥 响应体: {}", body);

                    if status.is_success() {
                        log_info!("✅ Successfully acknowledged grace period disclaimer");
                        Ok(body)
                    } else {
                        let error_msg = format!("❌ API request failed: {} - {}", status, body);
                        log_error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
                Err(e) => {
                    let error_msg = format!("❌ Failed to read response: {}", e);
                    log_error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("❌ Network request failed: {}", e);
            log_error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
async fn acknowledge_grace_period(
    _app: tauri::AppHandle,
    workos_cursor_session_token: String,
) -> Result<serde_json::Value, String> {
    match acknowledge_grace_period_internal(workos_cursor_session_token).await {
        Ok(response) => Ok(serde_json::json!({
            "success": true,
            "message": "Grace period disclaimer acknowledged successfully",
            "response": response
        })),
        Err(error) => {
            log_error!("❌ Failed to acknowledge grace period: {}", error);
            Err(error)
        }
    }
}

#[tauri::command]
async fn get_bind_card_url(
    _app: tauri::AppHandle,
    workos_cursor_session_token: String,
    subscription_tier: Option<String>,
    allow_automatic_payment: Option<bool>,
    allow_trial: Option<bool>,
) -> Result<serde_json::Value, String> {
    match get_bind_card_url_internal(
        workos_cursor_session_token,
        subscription_tier,
        allow_automatic_payment,
        allow_trial,
    )
    .await
    {
        Ok(url) => {
            // 复制到剪贴板
            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                let _ = Command::new("pbcopy")
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .and_then(|mut child| {
                        use std::io::Write;
                        if let Some(stdin) = child.stdin.as_mut() {
                            stdin.write_all(url.as_bytes())?;
                        }
                        child.wait()
                    });
            }

            #[cfg(target_os = "windows")]
            {
                use std::process::Command;
                let _ = Command::new("cmd")
                    .args(&["/C", &format!("echo {} | clip", url)])
                    .output();
            }

            #[cfg(target_os = "linux")]
            {
                use std::process::Command;
                let _ = Command::new("xclip")
                    .args(&["-selection", "clipboard"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .and_then(|mut child| {
                        use std::io::Write;
                        if let Some(stdin) = child.stdin.as_mut() {
                            stdin.write_all(url.as_bytes())?;
                        }
                        child.wait()
                    });
            }

            Ok(serde_json::json!({
                "success": true,
                "url": url,
                "message": "绑卡链接已复制到剪贴板"
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("获取绑卡链接失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn open_manual_bind_card_page(
    app: tauri::AppHandle,
    workos_cursor_session_token: String,
    subscription_tier: Option<String>,
    allow_automatic_payment: Option<bool>,
    allow_trial: Option<bool>,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 Opening manual bind card page with WorkOS token...");

    // 获取绑卡链接
    let url = match get_bind_card_url_internal(
        workos_cursor_session_token,
        subscription_tier,
        allow_automatic_payment,
        allow_trial,
    )
    .await
    {
        Ok(url) => url,
        Err(e) => {
            log_error!("❌ Failed to get bind card URL: {}", e);
            return Ok(serde_json::json!({
                "success": false,
                "message": format!("获取绑卡链接失败: {}", e)
            }));
        }
    };

    // 先尝试关闭已存在的窗口
    if let Some(existing_window) = app.get_webview_window("manual_bind_card") {
        log_info!("🔄 Closing existing manual bind card window...");
        if let Err(e) = existing_window.close() {
            log_error!("❌ Failed to close existing window: {}", e);
        } else {
            log_info!("✅ Existing window closed successfully");
        }
        // 等待一小段时间确保窗口完全关闭
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 解析 URL
    let parsed_url = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            log_error!("❌ Failed to parse URL: {}", e);
            return Ok(serde_json::json!({
                "success": false,
                "message": format!("无效的URL格式: {}", e)
            }));
        }
    };

    // 创建新的 WebView 窗口
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app,
        "manual_bind_card",
        tauri::WebviewUrl::External(parsed_url),
    )
    .title("Cursor - 手动绑卡")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .visible(true)
    .build();

    match webview_window {
        Ok(_window) => {
            log_info!("✅ Successfully opened bind card window");
            Ok(serde_json::json!({
                "success": true,
                "message": "已打开手动绑卡页面"
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to create WebView window: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("无法打开内置浏览器: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn show_manual_bind_card_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("manual_bind_card") {
        // 延迟1000ms再显示窗口
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        window
            .show()
            .map_err(|e| format!("Failed to show window: {}", e))?;
        log_info!("✅ Manual bind card window shown");

        // 发送事件通知前端操作成功
        if let Err(e) = app.emit("manual-bind-card-success", ()) {
            log_error!("❌ Failed to emit success event: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
async fn manual_bind_card_failed(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("manual_bind_card") {
        window
            .close()
            .map_err(|e| format!("Failed to close window: {}", e))?;
        log_error!("❌ Manual bind card failed, window closed");

        // 发送事件通知前端操作失败
        if let Err(e) = app.emit("manual-bind-card-failed", ()) {
            log_error!("❌ Failed to emit failed event: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
async fn delete_cursor_account(
    workos_cursor_session_token: String,
) -> Result<serde_json::Value, String> {
    use reqwest::header::{HeaderMap, HeaderValue};

    log_info!("🔄 开始调用 Cursor 删除账户 API...");

    // 构建请求头
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7"),
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Content-Length", HeaderValue::from_static("2"));
    headers.insert("Origin", HeaderValue::from_static("https://cursor.com"));
    headers.insert(
        "Referer",
        HeaderValue::from_static("https://cursor.com/cn/dashboard?tab=settings"),
    );
    headers.insert(
        "Sec-CH-UA",
        HeaderValue::from_static(
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\"",
        ),
    );
    headers.insert("Sec-CH-UA-Arch", HeaderValue::from_static("\"x86\""));
    headers.insert("Sec-CH-UA-Bitness", HeaderValue::from_static("\"64\""));
    headers.insert("Sec-CH-UA-Mobile", HeaderValue::from_static("?0"));
    headers.insert("Sec-CH-UA-Platform", HeaderValue::from_static("\"macOS\""));
    headers.insert(
        "Sec-CH-UA-Platform-Version",
        HeaderValue::from_static("\"15.3.1\""),
    );
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36"));

    // 使用传入的 WorkosCursorSessionToken
    let cookie_value = format!("WorkosCursorSessionToken={}", workos_cursor_session_token);
    log_info!(
        "🔍 [DEBUG] Using WorkosCursorSessionToken: {}...",
        &workos_cursor_session_token[..workos_cursor_session_token.len().min(50)]
    );
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie_value).map_err(|e| format!("Invalid cookie value: {}", e))?,
    );

    // 创建 HTTP 客户端
    let client = reqwest::Client::new();

    // 发送请求
    match client
        .post("https://cursor.com/api/dashboard/delete-account")
        .headers(headers)
        .body("{}")
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let headers_map: std::collections::HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            log_debug!("📥 API 响应状态: {}", status);
            log_debug!("📥 响应头: {:?}", headers_map);

            match response.text().await {
                Ok(body) => {
                    log_debug!("📥 响应体: {}", body);

                    Ok(serde_json::json!({
                        "success": status.is_success(),
                        "status": status.as_u16(),
                        "message": if status.is_success() {
                            format!("✅ 删除账户请求成功！状态码: {}, 响应: {}", status, body)
                        } else {
                            format!("❌ 删除账户失败！状态码: {}, 响应: {}", status, body)
                        },
                        "response_body": body,
                        "response_headers": headers_map
                    }))
                }
                Err(e) => {
                    log_error!("❌ 读取响应体失败: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "status": status.as_u16(),
                        "message": format!("❌ 读取响应失败: {}", e),
                        "response_headers": headers_map
                    }))
                }
            }
        }
        Err(e) => {
            log_error!("❌ 网络请求失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("❌ 网络请求失败: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn trigger_authorization_login(
    uuid: String,
    challenge: String,
    workos_cursor_session_token: String,
) -> Result<serde_json::Value, String> {
    use reqwest::header::{HeaderMap, HeaderValue};

    log_info!("🔄 开始调用 Cursor 授权登录 API...");
    log_debug!("🔍 [DEBUG] UUID: {}", uuid);
    log_debug!("🔍 [DEBUG] Challenge: {}", challenge);

    // 构建请求头
    let mut headers = HeaderMap::new();
    // 使用传入的 WorkosCursorSessionToken
    let cookie_value = format!("WorkosCursorSessionToken={}", workos_cursor_session_token);
    log_info!(
        "🔍 [DEBUG] Using WorkosCursorSessionToken: {}...",
        &workos_cursor_session_token[..workos_cursor_session_token.len().min(50)]
    );
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie_value).map_err(|e| format!("Invalid cookie value: {}", e))?,
    );

    // 创建 HTTP 客户端
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "challenge": challenge,
        "uuid": uuid,
    });

    // 发送请求
    match client
        .post("https://cursor.com/api/auth/loginDeepCallbackControl")
        .headers(headers)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let headers_map: std::collections::HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            log_debug!("📥 API 响应状态: {}", status);
            log_debug!("📥 响应头: {:?}", headers_map);

            match response.text().await {
                Ok(body) => {
                    log_debug!("📥 响应体: {}", body);

                    Ok(serde_json::json!({
                        "success": status.is_success(),
                        "status": status.as_u16(),
                        "message": if status.is_success() {
                            format!("✅ 授权登录请求成功！状态码: {}, 响应: {}", status, body)
                        } else {
                            format!("❌ 授权登录失败！状态码: {}, 响应: {}", status, body)
                        },
                        "response_body": body,
                        "response_headers": headers_map
                    }))
                }
                Err(e) => {
                    log_error!("❌ 读取响应体失败: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "status": status.as_u16(),
                        "message": format!("❌ 读取授权登录响应失败: {}", e),
                        "response_headers": headers_map
                    }))
                }
            }
        }
        Err(e) => {
            log_error!("❌ 网络请求授权登录失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("❌ 网络请求授权登录失败: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn trigger_authorization_login_poll(
    uuid: String,
    verifier: String,
) -> Result<serde_json::Value, String> {
    use reqwest::header::{HeaderMap, HeaderValue};

    log_info!("🔄 开始调用 Cursor 授权登录 Poll API...");
    log_debug!("🔍 [DEBUG] UUID: {}", uuid);
    log_debug!("🔍 [DEBUG] verifier: {}", verifier);

    // 构建请求头
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en,zh-CN;q=0.9,zh;q=0.8,eu;q=0.7"),
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Content-Length", HeaderValue::from_static("2"));
    headers.insert("Origin", HeaderValue::from_static("https://cursor.com"));
    headers.insert(
        "Sec-CH-UA",
        HeaderValue::from_static(
            "\"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"139\", \"Chromium\";v=\"139\"",
        ),
    );
    headers.insert("Sec-CH-UA-Arch", HeaderValue::from_static("\"x86\""));
    headers.insert("Sec-CH-UA-Bitness", HeaderValue::from_static("\"64\""));
    headers.insert("Sec-CH-UA-Mobile", HeaderValue::from_static("?0"));
    headers.insert("Sec-CH-UA-Platform", HeaderValue::from_static("\"macOS\""));
    headers.insert(
        "Sec-CH-UA-Platform-Version",
        HeaderValue::from_static("\"15.3.1\""),
    );
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36"));

    // 创建 HTTP 客户端
    let client = reqwest::Client::new();

    // 发送请求
    match client
        .get(&format!(
            "https://api2.cursor.sh/auth/poll?uuid={}&verifier={}",
            uuid, verifier
        ))
        .headers(headers)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let headers_map: std::collections::HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            log_debug!("📥 API 响应状态: {}", status);
            log_debug!("📥 响应头: {:?}", headers_map);

            match response.text().await {
                Ok(body) => {
                    log_debug!("📥 响应体: {}", body);

                    Ok(serde_json::json!({
                        "success": status.is_success(),
                        "status": status.as_u16(),
                        "message": if status.is_success() {
                            format!("✅ 授权登录Poll请求成功！状态码: {}, 响应: {}", status, body)
                        } else {
                            format!("❌ 授权登录Poll失败！状态码: {}, 响应: {}", status, body)
                        },
                        "response_body": body,
                        "response_headers": headers_map
                    }))
                }
                Err(e) => {
                    log_error!("❌ 读取响应体失败: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "status": status.as_u16(),
                        "message": format!("❌ 读取授权登录Poll响应失败: {}", e),
                        "response_headers": headers_map
                    }))
                }
            }
        }
        Err(e) => {
            log_error!("❌ 网络请求授权登录Poll失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("❌ 网络请求授权登录Poll失败: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn get_usage_for_period(
    token: String,
    start_date: u64,
    end_date: u64,
    team_id: i32,
) -> Result<serde_json::Value, String> {
    log_info!(
        "🔍 获取用量数据请求: token长度={}, start_date={}, end_date={}, team_id={}",
        token.len(),
        start_date,
        end_date,
        team_id
    );

    match AuthChecker::get_usage_for_period(&token, start_date, end_date, team_id).await {
        Ok(Some(usage_data)) => {
            log_info!("✅ 成功获取用量数据");
            Ok(serde_json::json!({
                "success": true,
                "message": "Successfully retrieved usage data",
                "data": usage_data
            }))
        }
        Ok(None) => {
            log_warn!("⚠️ 未找到用量数据");
            Ok(serde_json::json!({
                "success": false,
                "message": "No usage data found"
            }))
        }
        Err(e) => {
            log_error!("❌ 获取用量数据失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("Failed to get usage data: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn refresh_eligible_accounts_cache() -> Result<serde_json::Value, String> {
    match NextWorkWebServer::refresh_eligible_accounts_cache().await {
        Ok(count) => Ok(serde_json::json!({
            "success": true,
            "count": count,
            "message": format!("缓存刷新成功，找到 {} 个符合条件的账户", count)
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("缓存刷新失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn get_user_analytics(
    token: String,
    team_id: i32,
    user_id: i32,
    start_date: String,
    end_date: String,
) -> Result<serde_json::Value, String> {
    log_info!(
        "🔍 获取用户分析数据 - team_id: {}, user_id: {}, 时间范围: {} 到 {}",
        team_id,
        user_id,
        start_date,
        end_date
    );

    match AuthChecker::get_user_analytics(&token, team_id, user_id, &start_date, &end_date).await {
        Ok(Some(analytics_data)) => {
            log_info!("✅ 成功获取用户分析数据");
            Ok(serde_json::json!({
                "success": true,
                "message": "Successfully retrieved user analytics data",
                "data": analytics_data
            }))
        }
        Ok(None) => {
            log_warn!("⚠️ 未找到用户分析数据");
            Ok(serde_json::json!({
                "success": false,
                "message": "No user analytics data found"
            }))
        }
        Err(e) => {
            log_error!("❌ 获取用户分析数据失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("Failed to get user analytics data: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn get_usage_events(
    token: String,
    team_id: i32,
    start_date: String,
    end_date: String,
    page: i32,
    page_size: i32,
) -> Result<serde_json::Value, String> {
    log_info!(
        "🔍 获取使用事件数据 - team_id: {}, 时间范围: {} 到 {}, 页码: {}, 页大小: {}",
        team_id,
        start_date,
        end_date,
        page,
        page_size
    );

    match AuthChecker::get_usage_events(&token, team_id, &start_date, &end_date, page, page_size)
        .await
    {
        Ok(Some(events_data)) => {
            log_info!("✅ 成功获取使用事件数据");
            Ok(serde_json::json!({
                "success": true,
                "message": "Successfully retrieved usage events data",
                "data": events_data
            }))
        }
        Ok(None) => {
            log_warn!("⚠️ 未找到使用事件数据");
            Ok(serde_json::json!({
                "success": false,
                "message": "No usage events data found"
            }))
        }
        Err(e) => {
            log_error!("❌ 获取使用事件数据失败: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("Failed to get usage events data: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn register_cursor_account(
    first_name: String,
    last_name: String,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 开始注册 Cursor 账户...");
    log_info!("👤 姓名: {} {}", first_name, last_name);

    // 获取可执行文件路径
    let executable_path = get_python_executable_path()?;

    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    log_info!("🐍 调用Python可执行文件: {:?}", executable_path);

    // 生成随机邮箱
    let random_email = format!(
        "{}{}{}@gmail.com",
        first_name.to_lowercase(),
        last_name.to_lowercase(),
        rand::random::<u32>() % 1000
    );

    // 获取应用目录
    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();

    // 使用 Base64 编码应用目录路径，避免特殊字符问题
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 执行Python可执行文件
    let output = create_hidden_command(&executable_path.to_string_lossy())
        .arg(&random_email)
        .arg(&first_name)
        .arg(&last_name)
        .arg("true") // 默认使用无痕模式
        .arg(&app_dir_base64) // 使用 Base64 编码的应用目录参数
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动Python脚本: {}", e))?
        .wait_with_output()
        .map_err(|e| format!("等待Python脚本执行失败: {}", e))?;

    // 处理输出
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_error!("❌ Python脚本执行失败: {}", stderr);
        return Err(format!("注册失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log_info!("📝 Python脚本输出: {}", stdout);

    // 解析JSON响应
    let result: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("解析注册结果失败: {}", e))?;

    if result["success"].as_bool().unwrap_or(false) {
        // 注册成功，保存账户信息
        if let Some(email) = result["email"].as_str() {
            match AccountManager::add_account(
                email.to_string(),
                "python_registered_token".to_string(), // 临时token
                None,
                None,
            ) {
                Ok(_) => log_info!("💾 账户信息已保存"),
                Err(e) => log_warn!("⚠️ 保存账户信息失败: {}", e),
            }
        }

        log_info!("✅ 注册成功!");
        Ok(result)
    } else {
        let error_msg = result["error"].as_str().unwrap_or("未知错误");
        log_error!("❌ 泣册失败: {}", error_msg);
        Err(error_msg.to_string())
    }
}

#[tauri::command]
async fn create_temp_email() -> Result<serde_json::Value, String> {
    log_info!("📧 测试Python可执行文件...");

    // 获取可执行文件路径
    let executable_path = get_python_executable_path()?;

    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    // 获取应用目录
    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();

    // 使用 Base64 编码应用目录路径，避免特殊字符问题
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 执行Python可执行文件测试（传递一个测试邮箱）
    let output = create_hidden_command(&executable_path.to_string_lossy())
        .arg("test@example.com")
        .arg("Test")
        .arg("User")
        .arg("true") // 默认使用无痕模式
        .arg(&app_dir_base64) // 使用 Base64 编码的应用目录参数
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动Python脚本: {}", e))?
        .wait_with_output()
        .map_err(|e| format!("等待Python脚本执行失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建邮箱失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("解析邮箱结果失败: {}", e))?;

    Ok(result)
}

/// 批量注册账户（并行执行，每个账户一个独立窗口）
#[tauri::command]
async fn batch_register_with_email_parallel(
    app: tauri::AppHandle,
    emails: Vec<String>,
    first_names: Vec<String>,
    last_names: Vec<String>,
    email_type: Option<String>,
    _outlook_mode: Option<String>,  // 保留用于未来扩展
    tempmail_email: Option<String>, // Tempmail邮箱地址
    tempmail_pin: Option<String>,   // Tempmail PIN码
    self_hosted_mail_url: Option<String>,
    self_hosted_mail_headers_json: Option<String>,
    self_hosted_mail_response_path: Option<String>,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_indices: Option<Vec<u32>>, // 选中的银行卡索引列表
    config: Option<serde_json::Value>,       // 新增：配置JSON，包含订阅配置等
    max_concurrent: Option<usize>,           // 最大并发数，默认为3
) -> Result<serde_json::Value, String> {
    let email_type_str = email_type.as_deref().unwrap_or("custom");
    log_info!(
        "🔄 批量注册 {} 个 Cursor 账户（并行模式，邮箱类型：{}）...",
        emails.len(),
        email_type_str
    );

    if emails.len() != first_names.len() || emails.len() != last_names.len() {
        return Err("邮箱、姓名数量不一致".to_string());
    }

    // 读取银行卡配置
    let bank_card_config = read_bank_card_config().await?;
    let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
        .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

    let all_cards =
        if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
            cards_array.clone()
        } else {
            // 如果是旧格式（单张卡），转换为数组
            vec![bank_card_data]
        };

    // 如果提供了选中的银行卡索引，则只使用选中的卡片
    let cards = if let Some(indices) = &selected_card_indices {
        let mut selected_cards = Vec::new();
        for &index in indices.iter() {
            if (index as usize) < all_cards.len() {
                selected_cards.push(all_cards[index as usize].clone());
            } else {
                return Err(format!(
                    "银行卡索引 {} 超出范围（总共 {} 张卡）",
                    index,
                    all_cards.len()
                ));
            }
        }
        selected_cards
    } else {
        // 如果没有提供索引，使用所有卡片（保持向后兼容）
        all_cards
    };

    if enable_bank_card_binding.unwrap_or(true) && cards.len() < emails.len() {
        return Err(format!(
            "选中的银行卡数量({})少于注册账户数量({})，请选择足够的银行卡",
            cards.len(),
            emails.len()
        ));
    }

    log_info!("📋 准备使用 {} 张银行卡进行并行批量注册", cards.len());

    // 保存总数用于后续使用
    let total_count = emails.len();

    // 获取最大并发数，默认为3
    let max_concurrent_tasks = max_concurrent.unwrap_or(3);

    // 创建信号量，限制同时运行的任务数量（避免资源竞争和窗口数量限制）
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent_tasks));
    log_info!(
        "🚦 使用信号量控制并发，最多同时运行 {} 个注册任务",
        max_concurrent_tasks
    );

    // 创建所有注册任务
    let mut tasks = Vec::new();

    for i in 0..total_count {
        let app_clone = app.clone();
        let email = emails[i].clone();
        let first_name = first_names[i].clone();
        let last_name = last_names[i].clone();
        let email_type_str = email_type_str.to_string();
        let use_incognito_clone = use_incognito;
        let enable_bank_card_binding_clone = enable_bank_card_binding;
        let skip_phone_verification_clone = skip_phone_verification;
        let tempmail_email_clone = tempmail_email.clone();
        let tempmail_pin_clone = tempmail_pin.clone();
        let self_hosted_url_clone = self_hosted_mail_url.clone();
        let self_hosted_headers_clone = self_hosted_mail_headers_json.clone();
        let self_hosted_path_clone = self_hosted_mail_response_path.clone();
        let self_hosted_clear_enabled_clone = self_hosted_mail_clear_enabled;
        let self_hosted_clear_url_clone = self_hosted_mail_clear_url.clone();
        let self_hosted_clear_headers_clone = self_hosted_mail_clear_headers_json.clone();
        let self_hosted_clear_method_clone = self_hosted_mail_clear_method.clone();
        let config_clone = config.clone();

        // 计算当前任务应该使用的卡片索引
        let card_index_for_task = if enable_bank_card_binding.unwrap_or(true) && i < cards.len() {
            // 如果提供了选中的银行卡索引，使用对应的索引
            if let Some(indices) = &selected_card_indices {
                if i < indices.len() {
                    Some(indices[i])
                } else {
                    // 如果索引不够，循环使用
                    Some(indices[i % indices.len()])
                }
            } else {
                // 如果没有提供索引，使用循环索引（保持向后兼容）
                Some(i as u32)
            }
        } else {
            None
        };

        let email_display = if email.is_empty() {
            "自动生成".to_string()
        } else {
            email.clone()
        };
        log_info!(
            "🎯 [任务 {}/{}] 准备注册: {}",
            i + 1,
            total_count,
            email_display
        );

        let semaphore_clone = semaphore.clone();

        // 创建异步任务
        let task = tokio::spawn(async move {
            let task_index = i;

            // 获取信号量许可，如果已经有2个任务在运行，这里会等待
            log_info!(
                "🚦 [任务 {}/{}] 等待获取执行许可...",
                task_index + 1,
                total_count
            );
            let _permit = semaphore_clone.acquire().await.unwrap();
            log_info!(
                "✅ [任务 {}/{}] 获得执行许可，开始执行注册",
                task_index + 1,
                total_count
            );

            // 根据邮箱类型调用不同的注册函数
            let result = match email_type_str.as_str() {
                "cloudflare_temp" => {
                    log_info!("📧 [任务 {}] 使用 Cloudflare 临时邮箱注册", task_index + 1);

                    register_with_cloudflare_temp_email(
                        app_clone.clone(),
                        first_name.clone(),
                        last_name.clone(),
                        use_incognito_clone,
                        enable_bank_card_binding_clone,
                        skip_phone_verification_clone,
                        card_index_for_task,
                        config_clone.clone(),
                    )
                    .await
                }
                "tempmail" => {
                    log_info!(
                        "📧 [任务 {}] 使用 Tempmail 邮箱注册: {}",
                        task_index + 1,
                        email
                    );

                    register_with_tempmail(
                        app_clone.clone(),
                        email.clone(),
                        first_name.clone(),
                        last_name.clone(),
                        tempmail_email_clone.clone().unwrap_or_default(),
                        tempmail_pin_clone.clone().unwrap_or_default(),
                        use_incognito_clone,
                        enable_bank_card_binding_clone,
                        skip_phone_verification_clone,
                        card_index_for_task,
                        config_clone.clone(),
                    )
                    .await
                }
                "outlook" => {
                    log_info!(
                        "📧 [任务 {}] 使用 Outlook 邮箱注册: {}",
                        task_index + 1,
                        email
                    );

                    register_with_outlook(
                        app_clone.clone(),
                        email.clone(),
                        first_name.clone(),
                        last_name.clone(),
                        use_incognito_clone,
                        enable_bank_card_binding_clone,
                        skip_phone_verification_clone,
                        card_index_for_task,
                        config_clone.clone(),
                    )
                    .await
                }
                "self_hosted" => {
                    let url = self_hosted_url_clone.clone().unwrap_or_default();
                    let hdr = self_hosted_headers_clone.clone().unwrap_or_default();
                    let path = self_hosted_path_clone
                        .clone()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or_else(|| "results[0].raw".to_string());
                    if url.trim().is_empty() || hdr.trim().is_empty() {
                        Err("自建邮箱模式需配置 API 请求 URL 与 Headers（JSON 对象）".to_string())
                    } else {
                        log_info!(
                            "📧 [任务 {}] 使用自建邮箱 API 注册: {}",
                            task_index + 1,
                            email
                        );
                        register_with_self_hosted_mail_api(
                            app_clone.clone(),
                            email.clone(),
                            first_name.clone(),
                            last_name.clone(),
                            url,
                            hdr,
                            path,
                            self_hosted_clear_enabled_clone,
                            self_hosted_clear_url_clone.clone(),
                            self_hosted_clear_headers_clone.clone(),
                            self_hosted_clear_method_clone.clone(),
                            use_incognito_clone,
                            enable_bank_card_binding_clone,
                            skip_phone_verification_clone,
                            card_index_for_task,
                            config_clone.clone(),
                        )
                        .await
                    }
                }
                _ => {
                    // custom 或其他：使用指定邮箱
                    log_info!("📧 [任务 {}] 使用自定义邮箱注册: {}", task_index + 1, email);

                    register_with_email(
                        app_clone.clone(),
                        email.clone(),
                        first_name.clone(),
                        last_name.clone(),
                        use_incognito_clone,
                        enable_bank_card_binding_clone,
                        skip_phone_verification_clone,
                        card_index_for_task,
                        config_clone.clone(),
                    )
                    .await
                }
            };

            // 获取实际使用的邮箱（从结果中提取）
            let actual_email = match &result {
                Ok(result_data) => result_data
                    .get("accountInfo")
                    .and_then(|info| info.get("email"))
                    .and_then(|e| e.as_str())
                    .unwrap_or(&email)
                    .to_string(),
                Err(_) => email.clone(),
            };

            let task_result = match result {
                Ok(result) => {
                    log_info!("✅ [任务 {}] 注册成功: {}", task_index + 1, actual_email);
                    Ok(serde_json::json!({
                        "index": task_index,
                        "email": actual_email,
                        "success": true,
                        "result": result
                    }))
                }
                Err(e) => {
                    log_error!(
                        "❌ [任务 {}] 注册失败: {} - {}",
                        task_index + 1,
                        actual_email,
                        e
                    );
                    Err(serde_json::json!({
                        "index": task_index,
                        "email": actual_email,
                        "success": false,
                        "error": e
                    }))
                }
            };

            // _permit 在这里离开作用域，自动释放信号量许可
            drop(_permit);
            log_info!("🔓 [任务 {}/{}] 释放执行许可", task_index + 1, total_count);

            task_result
        });

        tasks.push(task);

        // 添加短暂延迟，避免同时创建太多任务
        if i < total_count - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    log_info!("⏳ 等待所有 {} 个注册任务完成...", tasks.len());

    // 等待所有任务完成
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for task in tasks {
        match task.await {
            Ok(Ok(result)) => {
                results.push(result);
                if let Some(item) = results.last() {
                    emit_batch_registration_progress(
                        &app,
                        "cursor",
                        "parallel",
                        item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        item.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                        true,
                        None,
                        results.len() + errors.len(),
                        total_count,
                        results.len(),
                        errors.len(),
                    );
                }
            }
            Ok(Err(error)) => {
                errors.push(error);
                if let Some(item) = errors.last() {
                    emit_batch_registration_progress(
                        &app,
                        "cursor",
                        "parallel",
                        item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        item.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                        false,
                        item.get("error").and_then(|v| v.as_str()),
                        results.len() + errors.len(),
                        total_count,
                        results.len(),
                        errors.len(),
                    );
                }
            }
            Err(e) => {
                log_error!("❌ 任务执行失败: {}", e);
                errors.push(serde_json::json!({
                    "success": false,
                    "error": format!("任务执行失败: {}", e)
                }));
                emit_batch_registration_progress(
                    &app,
                    "cursor",
                    "parallel",
                    results.len() + errors.len() - 1,
                    "",
                    false,
                    Some(&format!("任务执行失败: {}", e)),
                    results.len() + errors.len(),
                    total_count,
                    results.len(),
                    errors.len(),
                );
            }
        }
    }

    log_info!(
        "🎉 批量注册完成: {} 成功, {} 失败",
        results.len(),
        errors.len()
    );

    Ok(serde_json::json!({
        "success": true,
        "total": total_count,
        "succeeded": results.len(),
        "failed": errors.len(),
        "results": results,
        "errors": errors
    }))
}

/// 批量注册账户（串行执行，一个接一个注册，更稳定）
#[tauri::command]
async fn batch_register_with_email(
    app: tauri::AppHandle,
    emails: Vec<String>,
    first_names: Vec<String>,
    last_names: Vec<String>,
    email_type: Option<String>,
    _outlook_mode: Option<String>,  // 保留用于未来扩展
    tempmail_email: Option<String>, // Tempmail邮箱地址
    tempmail_pin: Option<String>,   // Tempmail PIN码
    self_hosted_mail_url: Option<String>,
    self_hosted_mail_headers_json: Option<String>,
    self_hosted_mail_response_path: Option<String>,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_indices: Option<Vec<u32>>, // 选中的银行卡索引列表
    config: Option<serde_json::Value>,       // 新增：配置JSON，包含订阅配置等
    batch_delay_seconds: Option<u64>,
) -> Result<serde_json::Value, String> {
    let email_type_str = email_type.as_deref().unwrap_or("custom");
    let batch_delay_seconds = batch_delay_seconds.unwrap_or(10).min(600);
    log_info!(
        "🔄 批量注册 {} 个 Cursor 账户（串行模式，邮箱类型：{}）...",
        emails.len(),
        email_type_str
    );

    if emails.len() != first_names.len() || emails.len() != last_names.len() {
        return Err("邮箱、姓名数量不一致".to_string());
    }

    // 读取银行卡配置
    let bank_card_config = read_bank_card_config().await?;
    let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
        .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

    let all_cards =
        if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
            cards_array.clone()
        } else {
            // 如果是旧格式（单张卡），转换为数组
            vec![bank_card_data]
        };

    // 如果提供了选中的银行卡索引，则只使用选中的卡片
    let cards = if let Some(indices) = &selected_card_indices {
        let mut selected_cards = Vec::new();
        for &index in indices.iter() {
            if (index as usize) < all_cards.len() {
                selected_cards.push(all_cards[index as usize].clone());
            } else {
                return Err(format!(
                    "银行卡索引 {} 超出范围（总共 {} 张卡）",
                    index,
                    all_cards.len()
                ));
            }
        }
        selected_cards
    } else {
        // 如果没有提供索引，使用所有卡片（保持向后兼容）
        all_cards
    };

    if enable_bank_card_binding.unwrap_or(true) && cards.len() < emails.len() {
        return Err(format!(
            "选中的银行卡数量({})少于注册账户数量({})，请选择足够的银行卡",
            cards.len(),
            emails.len()
        ));
    }

    log_info!("📋 准备使用 {} 张银行卡进行批量注册", cards.len());

    // 注意：不再需要备份配置，因为现在通过config传递cardIndex，不会修改配置文件

    // 串行执行注册，一个接一个
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for i in 0..emails.len() {
        if i > 0 {
            log_info!(
                "⏳ [任务 {}/{}] 等待 {} 秒后开始下一个注册任务...",
                i + 1,
                emails.len(),
                batch_delay_seconds
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(batch_delay_seconds)).await;
        }

        let email = emails[i].clone();
        let first_name = first_names[i].clone();
        let last_name = last_names[i].clone();

        let email_display = if email.is_empty() {
            "自动生成"
        } else {
            &email
        };
        log_info!(
            "🎯 [任务 {}/{}] 开始注册: {}",
            i + 1,
            emails.len(),
            email_display
        );

        // 计算当前任务应该使用的卡片索引
        let card_index_for_task = if enable_bank_card_binding.unwrap_or(true) && i < cards.len() {
            // 如果提供了选中的银行卡索引，使用对应的索引
            if let Some(indices) = &selected_card_indices {
                if i < indices.len() {
                    Some(indices[i])
                } else {
                    // 如果索引不够，循环使用
                    Some(indices[i % indices.len()])
                }
            } else {
                // 如果没有提供索引，使用循环索引（保持向后兼容）
                Some(i as u32)
            }
        } else {
            None
        };

        // 根据邮箱类型调用不同的注册函数
        let result = match email_type_str {
            "cloudflare_temp" => {
                log_info!(
                    "📧 [任务 {}/{}] 使用 Cloudflare 临时邮箱注册",
                    i + 1,
                    emails.len()
                );

                register_with_cloudflare_temp_email(
                    app.clone(),
                    first_name.clone(),
                    last_name.clone(),
                    use_incognito,
                    enable_bank_card_binding,
                    skip_phone_verification,
                    card_index_for_task, // 传递计算出的卡片索引
                    config.clone(),      // 传递config参数
                )
                .await
            }
            "tempmail" => {
                log_info!(
                    "📧 [任务 {}/{}] 使用 Tempmail 邮箱注册: {}",
                    i + 1,
                    emails.len(),
                    email
                );

                register_with_tempmail(
                    app.clone(),
                    email.clone(),
                    first_name.clone(),
                    last_name.clone(),
                    tempmail_email.clone().unwrap_or_default(),
                    tempmail_pin.clone().unwrap_or_default(),
                    use_incognito,
                    enable_bank_card_binding,
                    skip_phone_verification,
                    card_index_for_task, // 传递计算出的卡片索引
                    config.clone(),      // 传递config参数
                )
                .await
            }
            "outlook" => {
                log_info!(
                    "📧 [任务 {}/{}] 使用 Outlook 邮箱注册: {}",
                    i + 1,
                    emails.len(),
                    email
                );

                register_with_outlook(
                    app.clone(),
                    email.clone(),
                    first_name.clone(),
                    last_name.clone(),
                    use_incognito,
                    enable_bank_card_binding,
                    skip_phone_verification,
                    card_index_for_task, // 传递计算出的卡片索引
                    config.clone(),      // 传递config参数
                )
                .await
            }
            "self_hosted" => {
                let url = self_hosted_mail_url.clone().unwrap_or_default();
                let hdr = self_hosted_mail_headers_json.clone().unwrap_or_default();
                let path = self_hosted_mail_response_path
                    .clone()
                    .filter(|p| !p.trim().is_empty())
                    .unwrap_or_else(|| "results[0].raw".to_string());
                if url.trim().is_empty() || hdr.trim().is_empty() {
                    Err("自建邮箱模式需配置 API 请求 URL 与 Headers（JSON 对象）".to_string())
                } else {
                    log_info!(
                        "📧 [任务 {}/{}] 使用自建邮箱 API 注册: {}",
                        i + 1,
                        emails.len(),
                        email
                    );
                    register_with_self_hosted_mail_api(
                        app.clone(),
                        email.clone(),
                        first_name.clone(),
                        last_name.clone(),
                        url,
                        hdr,
                        path,
                        self_hosted_mail_clear_enabled,
                        self_hosted_mail_clear_url.clone(),
                        self_hosted_mail_clear_headers_json.clone(),
                        self_hosted_mail_clear_method.clone(),
                        use_incognito,
                        enable_bank_card_binding,
                        skip_phone_verification,
                        card_index_for_task,
                        config.clone(),
                    )
                    .await
                }
            }
            _ => {
                // custom 或其他：使用指定邮箱
                log_info!(
                    "📧 [任务 {}/{}] 使用自定义邮箱注册: {}",
                    i + 1,
                    emails.len(),
                    email
                );

                register_with_email(
                    app.clone(),
                    email.clone(),
                    first_name.clone(),
                    last_name.clone(),
                    use_incognito,
                    enable_bank_card_binding,
                    skip_phone_verification,
                    card_index_for_task, // 传递计算出的卡片索引
                    config.clone(),      // 传递config参数
                )
                .await
            }
        };

        // 获取实际使用的邮箱（从结果中提取）
        let actual_email = match &result {
            Ok(result_data) => result_data
                .get("accountInfo")
                .and_then(|info| info.get("email"))
                .and_then(|e| e.as_str())
                .unwrap_or(&email)
                .to_string(),
            Err(_) => email.clone(),
        };

        let (task_success, task_error_message) = match result {
            Ok(result) => {
                log_info!(
                    "✅ [任务 {}/{}] 注册成功: {}",
                    i + 1,
                    emails.len(),
                    actual_email
                );
                results.push(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": true,
                    "result": result
                }));
                (true, None)
            }
            Err(e) => {
                log_error!(
                    "❌ [任务 {}/{}] 注册失败: {} - {}",
                    i + 1,
                    emails.len(),
                    actual_email,
                    e
                );
                errors.push(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": false,
                    "error": e
                }));
                (
                    false,
                    errors
                        .last()
                        .and_then(|item| item.get("error").and_then(|value| value.as_str())),
                )
            }
        };

        emit_batch_registration_progress(
            &app,
            "cursor",
            "serial",
            i,
            &actual_email,
            task_success,
            task_error_message,
            results.len() + errors.len(),
            emails.len(),
            results.len(),
            errors.len(),
        );

    }

    // 注意：不再需要恢复配置，因为现在通过config传递cardIndex，不会修改配置文件

    log_info!(
        "🎉 批量注册完成: {} 成功, {} 失败",
        results.len(),
        errors.len()
    );

    Ok(serde_json::json!({
        "success": true,
        "total": emails.len(),
        "succeeded": results.len(),
        "failed": errors.len(),
        "results": results,
        "errors": errors
    }))
}

#[tauri::command]
async fn register_with_email(
    app: tauri::AppHandle,
    email: String,
    first_name: String,
    last_name: String,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_index: Option<u32>,  // 选中的银行卡索引
    config: Option<serde_json::Value>, // 新增：配置JSON，包含订阅配置等
) -> Result<serde_json::Value, String> {
    log_info!("╔══════════════════════════════════════════════════════════╗");
    log_info!("║  🔒 register_with_email 函数被调用                       ║");
    log_info!("╚══════════════════════════════════════════════════════════╝");
    log_info!("🔄 使用指定邮箱注册 Cursor 账户...");
    log_info!("⚠️  如果您看到多个浏览器窗口，请检查是否重复点击了注册按钮！");
    log_info!("📧 邮箱: {}", email);
    log_info!("👤 姓名: {} {}", first_name, last_name);
    log_info!("🔍 跳过手机号验证: {:?}", skip_phone_verification);
    log_info!("💳 选中的银行卡索引: {:?}", selected_card_index);

    // 获取自定义浏览器路径
    let custom_browser_path = {
        let restorer = MachineIdRestorer::new()
            .map_err(|e| format!("Failed to initialize restorer: {}", e))?;
        restorer.get_custom_browser_path()
    };

    // 如果启用了银行卡绑定，验证配置存在（不再需要备份和临时修改配置）
    if enable_bank_card_binding.unwrap_or(true) {
        log_info!("💳 验证银行卡配置...");

        // 验证银行卡配置存在
        let bank_card_config = read_bank_card_config().await?;
        let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
            .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

        // 获取所有卡片
        let all_cards =
            if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
                cards_array.clone()
            } else {
                // 旧格式：整个配置就是一张卡
                vec![bank_card_data.clone()]
            };

        if all_cards.is_empty() {
            return Err("银行卡配置为空，请先配置至少一张银行卡".to_string());
        }

        // 验证索引有效性
        let card_index = selected_card_index.unwrap_or(0) as usize;
        if card_index >= all_cards.len() {
            return Err(format!(
                "银行卡索引 {} 超出范围（总共 {} 张卡）",
                card_index,
                all_cards.len()
            ));
        }

        log_info!(
            "✅ 将使用卡片索引 {} 进行注册（通过config传递给Python）",
            card_index
        );
    }

    // 获取可执行文件路径
    let executable_path = get_python_executable_path()?;

    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    // 执行Python可执行文件
    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let bank_card_flag = if enable_bank_card_binding.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let skip_phone_flag = if skip_phone_verification.unwrap_or(false) {
        "1"
    } else {
        "0"
    };

    // 获取应用目录
    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();

    // 使用 Base64 编码应用目录路径，避免特殊字符问题
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 从前端传递的config中提取参数，如果没有则使用默认值
    let final_config = if let Some(frontend_config) = config {
        // 合并前端配置，包括代理配置
        let mut config_obj = serde_json::json!({
            "btnIndex": frontend_config.get("btnIndex").and_then(|v| v.as_u64()).unwrap_or(1),
            "subscriptionTier": frontend_config.get("subscriptionTier").and_then(|v| v.as_str()).unwrap_or("pro"),
            "allowAutomaticPayment": frontend_config.get("allowAutomaticPayment").and_then(|v| v.as_bool()).unwrap_or(true),
            "allowTrial": frontend_config.get("allowTrial").and_then(|v| v.as_bool()).unwrap_or(true),
            "useApiForBindCard": frontend_config.get("useApiForBindCard").and_then(|v| v.as_u64()).unwrap_or(1),
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加代理配置
        if let Some(proxy_config) = frontend_config.get("proxy") {
            config_obj["proxy"] = proxy_config.clone();
        }

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, Some(&frontend_config));

        config_obj
    } else {
        // 使用默认配置
        let mut config_obj = serde_json::json!({
            "btnIndex": 1,
            "subscriptionTier": "pro",
            "allowAutomaticPayment": true,
            "allowTrial": true,
            "useApiForBindCard": 1,
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, None);

        config_obj
    };

    let config_json_str = serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());

    // 生成任务ID用于隔离验证码文件（支持并行注册）
    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();

    // 停止信号文件（用于 cancel_registration）
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();

    log_info!("🆔 任务ID: {}", task_id);
    log_info!("📄 验证码文件: {}", code_file_path);

    // 调试：显示将要传递的所有参数
    log_debug!("🔍 [DEBUG] register_with_email 准备传递的参数:");
    log_info!("  - 参数1 (email): {}", email);
    log_info!("  - 参数2 (first_name): {}", first_name);
    log_info!("  - 参数3 (last_name): {}", last_name);
    log_info!("  - 参数4 (incognito_flag): {}", incognito_flag);
    log_info!("  - 参数5 (app_dir_str): {}", app_dir_str);
    log_info!("  - 参数5 (app_dir_base64): {}", app_dir_base64);
    log_info!("  - 参数6 (bank_card_flag): {}", bank_card_flag);
    log_info!("  - 参数7 (skip_phone_flag): {}", skip_phone_flag);
    log_info!("  - 参数8 (config_json): {}", config_json_str);
    log_info!("  - 预期参数总数: 9 (包括脚本名)");

    let mut child = create_hidden_command(&executable_path.to_string_lossy())
        .arg(&email)
        .arg(&first_name)
        .arg(&last_name)
        .arg(incognito_flag)
        .arg(&app_dir_base64) // 使用 Base64 编码的应用目录参数
        .arg(bank_card_flag) // 银行卡绑定标志
        .arg(skip_phone_flag) // 跳过手机号验证标志
        .arg(&config_json_str) // 配置JSON字符串
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path) // 通过环境变量传递验证码文件路径
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path) // 通过环境变量传递停止信号文件路径
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动Python脚本: {}", e))?;

    // 记录进程 PID，供 cancel_registration 主动终止（Cursor/Codex 统一）。
    {
        let pid = child.id();
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.insert(task_id.clone(), pid);
    }

    log_debug!("🔍 [DEBUG] 当前工作目录: {:?}", app_dir_str);

    // 实时读取输出
    use std::io::{BufRead, BufReader};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    let stdout = child.stdout.take().ok_or("无法获取stdout")?;
    let stderr = child.stderr.take().ok_or("无法获取stderr")?;

    let output_lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let error_lines = Arc::new(Mutex::new(Vec::<String>::new()));

    let output_lines_clone = output_lines.clone();
    let error_lines_clone = error_lines.clone();
    let app_clone = app.clone();
    let task_id_for_thread = task_id.clone();
    let email_for_thread = email.clone();
    let email_for_result = email.clone(); // 克隆一份供后续使用

    // 启动线程读取stdout
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                let line = sanitize_python_output_line(&line);
                log_info!("Python输出: {}", line);

                // 发送实时输出事件到前端
                if let Err(e) = app_clone.emit(
                    "registration-output",
                    serde_json::json!({
                        "type": "stdout",
                        "line": line.clone()
                    }),
                ) {
                    log_info!("发送事件失败: {}", e);
                } else {
                    let truncated = line.chars().take(50).collect::<String>();
                    log_info!("✅ 事件已发送: {}", truncated);
                }

                // 检查是否需要验证码
                if line.contains("等待前端输入验证码") || line.contains("request_verification_code")
                {
                    let _ = app_clone.emit(
                        "verification-code-required",
                        serde_json::json!({
                            "message": "请输入验证码",
                            "task_id": task_id_for_thread,
                            "email": email_for_thread
                        }),
                    );
                }

                // 检查验证码是否超时，需要手动输入
                if line.contains("verification_timeout") || line.contains("manual_input_required") {
                    log_info!("⏰ 验证码获取超时，需要用户手动输入");
                    let _ = app_clone.emit(
                        "verification-code-timeout",
                        serde_json::json!({
                            "message": "自动获取验证码超时，请手动输入验证码",
                            "task_id": task_id_for_thread,
                            "email": email_for_thread
                        }),
                    );
                }

                if let Ok(mut lines) = output_lines_clone.lock() {
                    lines.push(line);
                }
            }
        }
    });

    // 启动线程读取stderr
    let app_clone2 = app.clone();
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                let line = sanitize_python_output_line(&line);
                log_info!("Python错误: {}", line);

                // 发送错误输出事件到前端
                let _ = app_clone2.emit(
                    "registration-output",
                    serde_json::json!({
                        "type": "stderr",
                        "line": line.clone()
                    }),
                );

                if let Ok(mut lines) = error_lines_clone.lock() {
                    lines.push(line);
                }
            }
        }
    });

    // 等待一段时间或者进程结束
    let start_time = Instant::now();
    let max_wait_time = Duration::from_secs(150); // 给足够时间输入验证码
    let mut process_exit_status: Option<std::process::ExitStatus> = None;
    let mut was_killed = false;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已结束，保存退出状态
                process_exit_status = Some(status);
                break;
            }
            Ok(None) => {
                // 进程仍在运行
                if start_time.elapsed() > max_wait_time {
                    // 超时，终止进程
                    let _ = child.kill();
                    was_killed = true;
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("检查进程状态失败: {}", e));
            }
        }
    }

    // 等待读取线程完成
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    // 获取最终输出
    let final_output_lines = output_lines.lock().unwrap().clone();
    let final_error_lines = error_lines.lock().unwrap().clone();

    log_info!("收集到 {} 行输出", final_output_lines.len());
    log_info!("收集到 {} 行错误", final_error_lines.len());

    // 构建输出字符串
    let stdout_str = final_output_lines.join("\n");
    let stderr_str = final_error_lines.join("\n");

    // 检查进程退出状态码
    // 如果进程成功退出（exit code 0），说明注册成功（因为只有成功才会结束脚本关闭浏览器）
    let process_success = if was_killed {
        false // 如果进程被杀死，认为失败
    } else if let Some(status) = process_exit_status {
        status.success()
    } else {
        false // 如果没有退出状态，认为失败
    };

    // 尝试解析最后一行的JSON输出
    let mut result: serde_json::Value = if process_success {
        // 如果进程成功退出，即使没有JSON输出，也认为注册成功
        serde_json::json!({
            "success": true,
            "email": email_for_result,
            "message": "注册成功（进程成功退出）",
            "output_lines": final_output_lines,
            "raw_output": stdout_str
        })
    } else {
        // 如果进程失败退出，返回失败结果
        serde_json::json!({
            "success": false,
            "error": if was_killed { "进程超时被终止" } else { "进程执行失败" },
            "output_lines": final_output_lines,
            "raw_output": stdout_str
        })
    };

    // 从后往前查找有效的JSON
    for line in final_output_lines.iter().rev() {
        if line.trim().starts_with('{') {
            match serde_json::from_str::<serde_json::Value>(line.trim()) {
                Ok(mut parsed) => {
                    // 将输出信息添加到结果中
                    parsed["output_lines"] = serde_json::json!(final_output_lines);
                    parsed["raw_output"] = serde_json::json!(stdout_str);
                    if !stderr_str.is_empty() {
                        parsed["error_output"] = serde_json::json!(stderr_str);
                    }
                    // 如果进程成功退出，即使JSON中没有success字段，也设置为成功
                    if process_success && !parsed["success"].as_bool().unwrap_or(false) {
                        parsed["success"] = serde_json::json!(true);
                        if !parsed.get("message").is_some() {
                            parsed["message"] = serde_json::json!("注册成功（进程成功退出）");
                        }
                    }
                    result = parsed;
                    break;
                }
                Err(_) => continue,
            }
        }
    }
    // 前端触发保存
    // if result["success"].as_bool().unwrap_or(false) {
    //     // 注册成功，保存账户信息
    //     let token = result["token"]
    //         .as_str()
    //         .unwrap_or("python_registered_token")
    //         .to_string();
    //     let workos_token = result["workos_cursor_session_token"]
    //         .as_str()
    //         .map(|s| s.to_string());

    //     log_info!("🔑 提取的token: {}", token);
    //     if let Some(ref workos) = workos_token {
    //         log_info!(
    //             "🔐 WorkosCursorSessionToken: {}...",
    //             &workos[..std::cmp::min(50, workos.len())]
    //         );
    //     }

    //     match AccountManager::add_account(
    //         email.clone(),
    //         token,
    //         None,         // refresh_token
    //         workos_token, // workos_cursor_session_token
    //     ) {
    //         Ok(_) => log_info!("💾 账户信息已保存"),
    //         Err(e) => log_warn!("⚠️ 保存账户信息失败: {}", e),
    //     }
    // }

    // 注意：不再需要恢复配置，因为现在通过config传递cardIndex，不会修改配置文件

    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.remove(&task_id);
    }

    Ok(result)
}

#[tauri::command]
async fn register_with_cloudflare_temp_email(
    app: tauri::AppHandle,
    first_name: String,
    last_name: String,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_index: Option<u32>,  // 选中的银行卡索引
    config: Option<serde_json::Value>, // 新增：配置JSON，包含订阅配置等
) -> Result<serde_json::Value, String> {
    log_info!("🔄 使用Cloudflare临时邮箱注册 Cursor 账户...");
    log_info!("👤 姓名: {} {}", first_name, last_name);
    log_info!(
        "🔍 [DEBUG] 前端传递的 use_incognito 参数: {:?}",
        use_incognito
    );
    log_info!("🔍 跳过手机号验证: {:?}", skip_phone_verification);

    // 获取自定义浏览器路径
    let custom_browser_path = {
        let restorer = MachineIdRestorer::new()
            .map_err(|e| format!("Failed to initialize restorer: {}", e))?;
        restorer.get_custom_browser_path()
    };

    // 如果启用了银行卡绑定，验证配置存在（不再需要备份和临时修改配置）
    if enable_bank_card_binding.unwrap_or(true) {
        log_info!("💳 验证银行卡配置...");

        // 验证银行卡配置存在
        let bank_card_config = read_bank_card_config().await?;
        let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
            .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

        // 获取所有卡片
        let all_cards =
            if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
                cards_array.clone()
            } else {
                // 旧格式：整个配置就是一张卡
                vec![bank_card_data.clone()]
            };

        if all_cards.is_empty() {
            return Err("银行卡配置为空，请先配置至少一张银行卡".to_string());
        }

        // 验证索引有效性
        let card_index = selected_card_index.unwrap_or(0) as usize;
        if card_index >= all_cards.len() {
            return Err(format!(
                "银行卡索引 {} 超出范围（总共 {} 张卡）",
                card_index,
                all_cards.len()
            ));
        }

        log_info!(
            "✅ 将使用卡片索引 {} 进行注册（通过config传递给Python）",
            card_index
        );
    }

    // 1. 创建临时邮箱
    let (jwt, email) = create_cloudflare_temp_email().await?;
    log_info!("📧 创建的临时邮箱: {}", email);

    // 2. 获取可执行文件路径
    let executable_path = get_python_executable_path()?;

    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    // 3. 启动注册进程并设置实时输出
    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let bank_card_flag = if enable_bank_card_binding.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let skip_phone_flag = if skip_phone_verification.unwrap_or(false) {
        "1"
    } else {
        "0"
    };

    // 获取应用目录
    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();

    // 使用 Base64 编码应用目录路径，避免特殊字符问题
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 从前端传递的config中提取参数，如果没有则使用默认值
    let final_config = if let Some(frontend_config) = config {
        // 合并前端配置，包括代理配置
        let mut config_obj = serde_json::json!({
            "btnIndex": frontend_config.get("btnIndex").and_then(|v| v.as_u64()).unwrap_or(1),
            "subscriptionTier": frontend_config.get("subscriptionTier").and_then(|v| v.as_str()).unwrap_or("pro"),
            "allowAutomaticPayment": frontend_config.get("allowAutomaticPayment").and_then(|v| v.as_bool()).unwrap_or(true),
            "allowTrial": frontend_config.get("allowTrial").and_then(|v| v.as_bool()).unwrap_or(true),
            "useApiForBindCard": frontend_config.get("useApiForBindCard").and_then(|v| v.as_u64()).unwrap_or(1),
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加代理配置
        if let Some(proxy_config) = frontend_config.get("proxy") {
            config_obj["proxy"] = proxy_config.clone();
        }

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, Some(&frontend_config));

        config_obj
    } else {
        // 使用默认配置
        let mut config_obj = serde_json::json!({
            "btnIndex": 1,
            "subscriptionTier": "pro",
            "allowAutomaticPayment": true,
            "allowTrial": true,
            "useApiForBindCard": 1,
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, None);

        config_obj
    };

    let config_json_str = serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());

    // 生成任务ID用于隔离验证码文件（支持并行注册）
    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();

    // 停止信号文件（用于 cancel_registration）
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();

    log_info!("🆔 任务ID: {}", task_id);
    log_info!("📄 验证码文件: {}", code_file_path);

    // 调试日志
    log_debug!("🔍 [DEBUG] Rust 启动Python脚本:");
    log_info!("  - 可执行文件: {:?}", executable_path);
    log_info!("  - 邮箱: {}", email);
    log_info!("  - 姓名: {} {}", first_name, last_name);
    log_info!("  - use_incognito 原始值: {:?}", use_incognito);
    log_info!("  - incognito_flag: {}", incognito_flag);
    log_info!("  - bank_card_flag: {}", bank_card_flag);
    log_info!("  - skip_phone_flag: {}", skip_phone_flag);
    log_info!("  - config_json: {}", config_json_str);
    log_info!("  - app_dir: {}", app_dir_str);
    log_info!("  - app_dir_base64: {}", app_dir_base64);
    log_info!(
        "  - 传递的参数: [{}, {}, {}, {}, {}, {}, {}, {}]",
        email,
        first_name,
        last_name,
        incognito_flag,
        app_dir_base64,
        bank_card_flag,
        skip_phone_flag,
        config_json_str
    );

    let mut child = create_hidden_command(&executable_path.to_string_lossy())
        .arg(&email)
        .arg(&first_name)
        .arg(&last_name)
        .arg(incognito_flag)
        .arg(&app_dir_base64) // 使用 Base64 编码的应用目录参数
        .arg(bank_card_flag) // 银行卡绑定标志
        .arg(skip_phone_flag) // 跳过手机号验证标志
        .arg(&config_json_str) // 配置JSON字符串
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path) // 通过环境变量传递验证码文件路径
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path) // 通过环境变量传递停止信号文件路径
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动Python脚本: {}", e))?;

    // 获取stdout用于实时读取
    let stdout = child.stdout.take().ok_or("无法获取Python脚本的stdout")?;

    // 启动实时输出读取任务
    let app_for_output = app.clone();
    let jwt_for_verification = jwt.clone();
    let app_for_verification = app.clone();
    let code_file_for_thread = code_file_path.clone();

    // 使用Arc<AtomicBool>来跟踪是否需要获取验证码
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let verification_needed = Arc::new(AtomicBool::new(false));
    let verification_needed_clone = verification_needed.clone();

    // 启动实时输出读取任务（在单独线程中）
    let app_clone = app_for_output.clone();
    let verification_needed_clone = verification_needed_clone.clone();
    let jwt_clone = jwt_for_verification.clone();
    let app_verification_clone = app_for_verification.clone();
    let task_id_for_thread = task_id.clone();
    let email_for_thread = email.clone();

    let output_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};

        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    let line_content = sanitize_python_output_line(&line_content);
                    log_info!("📝 Python输出: {}", line_content);

                    // 检查是否需要验证码
                    if line_content.contains("等待验证码")
                        || line_content.contains("request_verification_code")
                    {
                        log_debug!("🔍 检测到验证码请求，开始自动获取验证码...");
                        verification_needed_clone.store(true, Ordering::Relaxed);

                        // 启动验证码获取任务
                        let jwt_task = jwt_clone.clone();
                        let app_task = app_verification_clone.clone();
                        let code_file_task = code_file_for_thread.clone();
                        std::thread::spawn(move || {
                            // 使用tokio运行时
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                // 等待一小段时间让邮件到达
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                                for attempt in 1..=10 {
                                    log_debug!("🔍 第{}次尝试获取验证码...", attempt);

                                    match get_verification_code_from_cloudflare(&jwt_task).await {
                                        Ok(code) => {
                                            log_info!("🎯 自动获取到验证码: {}", code);

                                            // 将验证码写入任务专属的临时文件
                                            if let Err(e) = std::fs::write(&code_file_task, &code) {
                                                log_error!("❌ 写入验证码文件失败: {}", e);
                                                return;
                                            }

                                            // 发送事件通知前端
                                            if let Err(e) = app_task
                                                .emit("verification-code-auto-filled", &code)
                                            {
                                                log_error!("❌ 发送验证码事件失败: {}", e);
                                            }

                                            log_info!(
                                                "✅ 验证码已自动填入临时文件: {}",
                                                code_file_task
                                            );
                                            return;
                                        }
                                        Err(e) => {
                                            log_debug!("🔍 第{}次获取验证码失败: {}", attempt, e);
                                            if attempt < 10 {
                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_secs(10),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                }

                                log_error!("❌ 自动获取验证码失败，已尝试10次");
                                if let Err(emit_err) =
                                    app_task.emit("verification-code-failed", "获取验证码失败")
                                {
                                    log_error!("❌ 发送失败事件失败: {}", emit_err);
                                }
                            });
                        });
                    }

                    // 检查验证码是否超时，需要手动输入
                    if line_content.contains("verification_timeout")
                        || line_content.contains("manual_input_required")
                    {
                        log_info!("⏰ 验证码获取超时，需要用户手动输入");
                        let _ = app_clone.emit(
                            "verification-code-timeout",
                            serde_json::json!({
                                "message": "自动获取验证码超时，请手动输入验证码",
                                "task_id": task_id_for_thread,
                                "email": email_for_thread
                            }),
                        );
                    }

                    // 发送实时输出到前端
                    if let Err(e) = app_clone.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    ) {
                        log_error!("❌ 发送输出事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取Python输出失败: {}", e);
                    break;
                }
            }
        }
    });

    // 验证码获取已集成到输出读取任务中

    // 4. 等待注册进程完成
    let exit_status = child
        .wait()
        .map_err(|e| format!("等待Python脚本执行失败: {}", e))?;

    log_debug!("🔍 Python进程已结束");

    // 等待输出读取任务完成
    let _ = output_task.join();

    // 6. 处理进程退出状态
    if !exit_status.success() {
        log_error!("❌ Python脚本执行失败，退出码: {:?}", exit_status.code());
        return Err(format!(
            "Python脚本执行失败，退出码: {:?}",
            exit_status.code()
        ));
    }

    // 7. 由于我们已经通过实时输出获取了所有信息，这里需要从最后的输出中解析结果
    // 我们可以通过检查临时文件或其他方式来获取最终结果
    // 简化处理：返回一个成功的结果，具体的注册状态通过实时输出已经传递给前端
    let result = serde_json::json!({
        // "success": true,
        // "message": "注册流程已完成",
        "email": email,
        "email_type": "cloudflare_temp"
    });

    // 8. 邮箱信息已经在创建result时添加了，这里不需要重复添加

    // 9. 如果注册成功，保存账户信息-前端保存
    // if result["success"].as_bool().unwrap_or(false) {
    //     let token = result["token"]
    //         .as_str()
    //         .unwrap_or("python_registered_token")
    //         .to_string();
    //     let workos_token = result["workos_cursor_session_token"]
    //         .as_str()
    //         .map(|s| s.to_string());

    //     log_info!("🔑 提取的token: {}", token);
    //     if let Some(ref workos) = workos_token {
    //         log_info!(
    //             "🔐 WorkosCursorSessionToken: {}...",
    //             &workos[..std::cmp::min(50, workos.len())]
    //         );
    //     }

    //     match AccountManager::add_account(
    //         email.clone(),
    //         token,
    //         None,         // refresh_token
    //         workos_token, // workos_cursor_session_token
    //     ) {
    //         Ok(_) => log_info!("💾 账户信息已保存"),
    //         Err(e) => log_warn!("⚠️ 保存账户信息失败: {}", e),
    //     }
    // }

    // 注意：不再需要恢复配置，因为现在通过config传递cardIndex，不会修改配置文件

    Ok(result)
}

// 使用Tempmail临时邮箱注册账户
#[tauri::command]
async fn register_with_tempmail(
    app: tauri::AppHandle,
    email: String,
    first_name: String,
    last_name: String,
    tempmail_email: String,
    tempmail_pin: String,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_index: Option<u32>,
    config: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    log_info!("╔══════════════════════════════════════════════════════════╗");
    log_info!("║  🔒 register_with_tempmail 函数被调用                    ║");
    log_info!("╚══════════════════════════════════════════════════════════╝");
    log_info!("🔄 使用Tempmail临时邮箱注册 Cursor 账户...");
    log_info!("⚠️  如果您看到多个浏览器窗口，请检查是否重复点击了注册按钮！");
    log_info!("📧 注册邮箱: {}", email);
    log_info!("📧 临时邮箱: {}", tempmail_email);
    log_info!(
        "🔑 PIN码: {}",
        if tempmail_pin.is_empty() {
            "无"
        } else {
            "已设置"
        }
    );
    log_info!("👤 姓名: {} {}", first_name, last_name);
    log_info!("🔍 跳过手机号验证: {:?}", skip_phone_verification);

    // 获取自定义浏览器路径
    let custom_browser_path = {
        let restorer = MachineIdRestorer::new()
            .map_err(|e| format!("Failed to initialize restorer: {}", e))?;
        restorer.get_custom_browser_path()
    };

    // 如果启用了银行卡绑定，验证配置存在
    if enable_bank_card_binding.unwrap_or(true) {
        log_info!("💳 验证银行卡配置...");

        let bank_card_config = read_bank_card_config().await?;
        let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
            .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

        let all_cards =
            if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
                cards_array.clone()
            } else {
                vec![bank_card_data.clone()]
            };

        if all_cards.is_empty() {
            return Err("银行卡配置为空，请先配置至少一张银行卡".to_string());
        }

        let card_index = selected_card_index.unwrap_or(0) as usize;
        if card_index >= all_cards.len() {
            return Err(format!(
                "银行卡索引 {} 超出范围（总共 {} 张卡）",
                card_index,
                all_cards.len()
            ));
        }

        log_info!("✅ 将使用卡片索引 {} 进行注册", card_index);
    }

    // 获取可执行文件路径
    let executable_path = get_python_executable_path()?;
    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    // 设置参数
    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };
    let bank_card_flag = if enable_bank_card_binding.unwrap_or(true) {
        "true"
    } else {
        "false"
    };
    let skip_phone_flag = if skip_phone_verification.unwrap_or(false) {
        "1"
    } else {
        "0"
    };

    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 构建配置JSON
    let final_config = if let Some(frontend_config) = config {
        let mut config_obj = serde_json::json!({
            "btnIndex": frontend_config.get("btnIndex").and_then(|v| v.as_u64()).unwrap_or(1),
            "subscriptionTier": frontend_config.get("subscriptionTier").and_then(|v| v.as_str()).unwrap_or("pro"),
            "allowAutomaticPayment": frontend_config.get("allowAutomaticPayment").and_then(|v| v.as_bool()).unwrap_or(true),
            "allowTrial": frontend_config.get("allowTrial").and_then(|v| v.as_bool()).unwrap_or(true),
            "useApiForBindCard": frontend_config.get("useApiForBindCard").and_then(|v| v.as_u64()).unwrap_or(1),
            "cardIndex": selected_card_index.unwrap_or(0),
            "tempmail_email": tempmail_email.clone(),
            "tempmail_pin": tempmail_pin.clone()
        });

        if let Some(proxy_config) = frontend_config.get("proxy") {
            config_obj["proxy"] = proxy_config.clone();
        }

        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, Some(&frontend_config));

        config_obj
    } else {
        let mut config_obj = serde_json::json!({
            "btnIndex": 1,
            "subscriptionTier": "pro",
            "allowAutomaticPayment": true,
            "allowTrial": true,
            "useApiForBindCard": 1,
            "cardIndex": selected_card_index.unwrap_or(0),
            "tempmail_email": tempmail_email.clone(),
            "tempmail_pin": tempmail_pin.clone()
        });

        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        merge_haozhuma_config_into_runtime_config(&mut config_obj, None);

        config_obj
    };

    let config_json_str = serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());

    // 生成任务ID用于隔离验证码文件（支持并行注册）
    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();
    // 停止信号文件路径，通过环境变量传递给 Python
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();

    log_info!("🆔 任务ID: {}", task_id);
    log_info!("📄 验证码文件: {}", code_file_path);
    log_info!("🛑 停止信号文件: {}", stop_file_path);

    log_info!("🔍 [DEBUG] Rust 启动Python脚本:");
    log_info!("  - 可执行文件: {:?}", executable_path);
    log_info!("  - 注册邮箱: {}", email);
    log_info!("  - 临时邮箱: {}", tempmail_email);
    log_info!("  - 姓名: {} {}", first_name, last_name);
    log_info!("  - config_json: {}", config_json_str);

    // 启动Python脚本
    let mut child = create_hidden_command(&executable_path.to_string_lossy())
        .arg(&email)
        .arg(&first_name)
        .arg(&last_name)
        .arg(incognito_flag)
        .arg(&app_dir_base64)
        .arg(bank_card_flag)
        .arg(skip_phone_flag)
        .arg(&config_json_str)
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path) // 通过环境变量传递验证码文件路径
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path) // 通过环境变量传递停止信号文件路径
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动Python脚本: {}", e))?;

    // 获取进程ID并注册到进程管理器
    let pid = child.id();
    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.insert(task_id.clone(), pid);
        log_info!("📝 已注册进程到管理器: task_id={}, pid={}", task_id, pid);
    }

    let stdout = child.stdout.take().ok_or("无法获取Python脚本的stdout")?;

    // 启动实时输出读取任务
    let app_for_output = app.clone();
    let tempmail_email_for_verification = tempmail_email.clone();
    let tempmail_pin_for_verification = tempmail_pin.clone();
    let register_email_for_verification = email.clone();
    let app_for_verification = app.clone();
    let code_file_for_thread = code_file_path.clone();

    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let verification_needed = Arc::new(AtomicBool::new(false));
    let verification_needed_clone = verification_needed.clone();

    // 启动实时输出读取任务
    let app_clone = app_for_output.clone();
    let verification_needed_clone = verification_needed_clone.clone();
    let tempmail_email_clone = tempmail_email_for_verification.clone();
    let tempmail_pin_clone = tempmail_pin_for_verification.clone();
    let register_email_clone = register_email_for_verification.clone();
    let app_verification_clone = app_for_verification.clone();
    let task_id_for_thread = task_id.clone();

    let output_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    let line_content = sanitize_python_output_line(&line_content);
                    log_info!("📝 Python输出: {}", line_content);

                    // 检查是否需要验证码
                    if line_content.contains("等待验证码")
                        || line_content.contains("request_verification_code")
                    {
                        log_debug!("🔍 检测到验证码请求，开始从tempmail获取验证码...");
                        verification_needed_clone.store(true, Ordering::Relaxed);

                        // 启动验证码获取任务
                        let tempmail_email_task = tempmail_email_clone.clone();
                        let tempmail_pin_task = tempmail_pin_clone.clone();
                        let register_email_task = register_email_clone.clone();
                        let app_task = app_verification_clone.clone();
                        let code_file_task = code_file_for_thread.clone();
                        let task_id_clone = task_id_for_thread.clone();

                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                                match get_verification_code_from_tempmail(
                                    &tempmail_email_task,
                                    &tempmail_pin_task,
                                    &register_email_task,
                                )
                                .await
                                {
                                    Ok(code) => {
                                        log_info!("🎯 自动从tempmail获取到验证码: {}", code);

                                        // 将验证码写入任务专属的临时文件
                                        if let Err(e) = std::fs::write(&code_file_task, &code) {
                                            log_error!("❌ 写入验证码文件失败: {}", e);
                                            return;
                                        }

                                        // 发送事件通知前端
                                        if let Err(e) =
                                            app_task.emit("verification-code-auto-filled", &code)
                                        {
                                            log_error!("❌ 发送验证码事件失败: {}", e);
                                        }

                                        log_info!("✅ 验证码已自动填入");
                                    }
                                    Err(e) => {
                                        log_error!("❌ 从tempmail获取验证码失败: {}", e);
                                        log_info!("⏰ 验证码获取超时，需要用户手动输入");
                                        let _ = app_task.emit(
                                            "verification-code-timeout",
                                            serde_json::json!({
                                                "message": "自动获取验证码超时，请手动输入验证码",
                                                "task_id": task_id_clone,
                                                "email": register_email_task
                                            }),
                                        );
                                    }
                                }
                            });
                        });
                    }

                    // 发送实时输出到前端
                    if let Err(e) = app_clone.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    ) {
                        log_error!("❌ 发送输出事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取Python输出失败: {}", e);
                    break;
                }
            }
        }
    });

    // 等待Python脚本完成
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待Python脚本完成失败: {}", e))?;

    // 等待输出任务完成
    if let Err(e) = output_task.join() {
        log_error!("❌ 输出任务失败: {:?}", e);
    }

    // 首先检查进程退出状态码
    // 如果进程成功退出（exit code 0），说明注册成功（因为只有成功才会结束脚本关闭浏览器）
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_error!("❌ Python脚本执行失败，退出码: {:?}", output.status.code());
        log_error!("📤 stdout: {}", stdout);
        if !stderr.is_empty() {
            log_error!("📤 stderr: {}", stderr);
        }
        return Err(format!(
            "Python脚本执行失败，退出码: {:?}",
            output.status.code()
        ));
    }

    // 进程成功退出，尝试解析JSON输出（如果有的话）
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    log_info!("📤 Python脚本stdout: {}", stdout);
    if !stderr.is_empty() {
        log_info!("📤 Python脚本stderr: {}", stderr);
    }

    // 尝试解析JSON，如果解析失败，仍然认为注册成功（因为进程已成功退出）
    let result: serde_json::Value = if stdout.trim().is_empty() {
        // 如果没有输出，创建一个默认的成功结果
        log_info!("⚠️ Python脚本没有输出JSON，但进程成功退出，认为注册成功");
        serde_json::json!({
            "success": true,
            "email": email,
            "message": "注册成功（进程成功退出）"
        })
    } else {
        match serde_json::from_str::<serde_json::Value>(&stdout) {
            Ok(parsed) => {
                log_info!("🔍 [DEBUG] 解析后的结果: {:?}", parsed);
                parsed
            }
            Err(e) => {
                // JSON解析失败，但进程成功退出，仍然认为注册成功
                log_warn!(
                    "⚠️ 解析Python输出JSON失败: {}，但进程成功退出，认为注册成功",
                    e
                );
                serde_json::json!({
                    "success": true,
                    "email": email,
                    "message": "注册成功（进程成功退出，但JSON解析失败）",
                    "raw_output": stdout
                })
            }
        }
    };

    // 如果JSON中有明确的成功标识，使用JSON中的信息
    if result["success"].as_bool().unwrap_or(false)
        || result
            .get("message")
            .and_then(|m| m.as_str())
            .map_or(false, |s| s.contains("注册成功"))
    {
        // 保存账户信息
        if let Some(email_str) = result["email"].as_str() {
            match AccountManager::add_account(
                email_str.to_string(),
                "python_registered_token".to_string(),
                None,
                None,
            ) {
                Ok(_) => log_info!("💾 账户信息已保存"),
                Err(e) => log_warn!("⚠️ 保存账户信息失败: {}", e),
            }
        }

        log_info!("✅ 注册成功!");
        Ok(result)
    } else {
        // 即使JSON中没有明确成功标识，但进程成功退出，仍然认为注册成功
        log_info!("✅ 注册成功（进程成功退出）!");
        Ok(result)
    }
}

// 使用Outlook邮箱注册账户
#[tauri::command]
async fn register_with_outlook(
    app: tauri::AppHandle,
    email: String,
    first_name: String,
    last_name: String,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_index: Option<u32>,  // 选中的银行卡索引
    config: Option<serde_json::Value>, // 新增：配置JSON，包含订阅配置等
) -> Result<serde_json::Value, String> {
    log_info!("🔄 使用Outlook邮箱注册 Cursor 账户...");
    log_info!("📧 邮箱: {}", email);
    log_info!("👤 姓名: {} {}", first_name, last_name);
    log_info!("🔍 跳过手机号验证: {:?}", skip_phone_verification);
    log_info!(
        "🔍 [DEBUG] 前端传递的 use_incognito 参数: {:?}",
        use_incognito
    );

    // 获取自定义浏览器路径
    let custom_browser_path = {
        let restorer = MachineIdRestorer::new()
            .map_err(|e| format!("Failed to initialize restorer: {}", e))?;
        restorer.get_custom_browser_path()
    };

    // 如果启用了银行卡绑定，先备份并设置银行卡配置（使用第一张卡）
    if enable_bank_card_binding.unwrap_or(true) {
        log_info!("💳 准备设置银行卡配置...");

        // 注意：不再需要备份配置，因为现在通过config传递cardIndex，不会修改配置文件

        // 验证银行卡配置存在
        let bank_card_config = read_bank_card_config().await?;
        let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
            .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

        // 获取所有卡片
        let all_cards =
            if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
                cards_array.clone()
            } else {
                // 旧格式：整个配置就是一张卡
                vec![bank_card_data.clone()]
            };

        if all_cards.is_empty() {
            return Err("银行卡配置为空，请先配置至少一张银行卡".to_string());
        }

        // 验证索引有效性
        let card_index = selected_card_index.unwrap_or(0) as usize;
        if card_index >= all_cards.len() {
            return Err(format!(
                "银行卡索引 {} 超出范围（总共 {} 张卡）",
                card_index,
                all_cards.len()
            ));
        }

        log_info!(
            "✅ 将使用卡片索引 {} 进行注册（通过config传递给Python）",
            card_index
        );
    }

    // 获取可执行文件路径
    let executable_path = get_python_executable_path()?;

    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    // 启动注册进程并设置实时输出
    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let bank_card_flag = if enable_bank_card_binding.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let skip_phone_flag = if skip_phone_verification.unwrap_or(false) {
        "1"
    } else {
        "0"
    };

    // 获取应用目录
    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    // 从前端传递的config中提取参数，如果没有则使用默认值
    let final_config = if let Some(frontend_config) = config {
        // 合并前端配置，包括代理配置
        let mut config_obj = serde_json::json!({
            "btnIndex": frontend_config.get("btnIndex").and_then(|v| v.as_u64()).unwrap_or(1),
            "subscriptionTier": frontend_config.get("subscriptionTier").and_then(|v| v.as_str()).unwrap_or("pro"),
            "allowAutomaticPayment": frontend_config.get("allowAutomaticPayment").and_then(|v| v.as_bool()).unwrap_or(true),
            "allowTrial": frontend_config.get("allowTrial").and_then(|v| v.as_bool()).unwrap_or(true),
            "useApiForBindCard": frontend_config.get("useApiForBindCard").and_then(|v| v.as_u64()).unwrap_or(1),
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加代理配置
        if let Some(proxy_config) = frontend_config.get("proxy") {
            config_obj["proxy"] = proxy_config.clone();
        }

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        config_obj
    } else {
        // 使用默认配置
        let mut config_obj = serde_json::json!({
            "btnIndex": 1,
            "subscriptionTier": "pro",
            "allowAutomaticPayment": true,
            "allowTrial": true,
            "useApiForBindCard": 1,
            "cardIndex": selected_card_index.unwrap_or(0)
        });

        // 添加自定义浏览器路径
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
            log_info!("🌐 添加自定义浏览器路径到配置: {}", browser_path);
        }

        config_obj
    };

    let config_json_str = serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());

    // 生成任务ID用于隔离验证码文件（支持并行注册）
    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();

    // 停止信号文件（用于 cancel_registration）
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();

    log_info!("🆔 任务ID: {}", task_id);
    log_info!("📄 验证码文件: {}", code_file_path);

    log_debug!("🔍 [DEBUG] 准备启动注册进程");
    log_info!("    可执行文件: {:?}", executable_path);
    log_info!("    邮箱: {}", email);
    log_info!("    姓名: {} {}", first_name, last_name);
    log_info!("    隐身模式: {}", incognito_flag);
    log_info!("    银行卡绑定: {}", bank_card_flag);
    log_info!("    跳过手机号验证: {}", skip_phone_flag);
    log_info!("    配置JSON: {}", config_json_str);

    let mut cmd = create_hidden_command(&executable_path.to_string_lossy());
    cmd.arg(&email)
        .arg(&first_name)
        .arg(&last_name)
        .arg(incognito_flag)
        .arg(&app_dir_base64)
        .arg(bank_card_flag)
        .arg(skip_phone_flag)
        .arg(&config_json_str)
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path) // 通过环境变量传递验证码文件路径
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path) // 通过环境变量传递停止信号文件路径
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    log_debug!("🔍 [DEBUG] 命令行: {:?}", cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动注册进程: {}", e))?;

    {
        let pid = child.id();
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.insert(task_id.clone(), pid);
        log_info!("📝 已注册进程到管理器: task_id={}, pid={}", task_id, pid);
    }

    let stdout = child.stdout.take().ok_or("无法获取stdout".to_string())?;

    let stderr = child.stderr.take().ok_or("无法获取stderr".to_string())?;

    // 启动实时输出读取任务（使用同步线程，与Cloudflare注册函数保持一致）
    let app_clone = app.clone();
    let email_clone = email.clone();
    let code_file_for_thread = code_file_path.clone();
    let task_id_clone = task_id.clone();

    // 处理stdout
    let app_for_stdout = app_clone.clone();
    let email_for_stdout = email_clone.clone();
    let task_id_for_stdout = task_id_clone.clone();
    let stdout_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    let line_content = sanitize_python_output_line(&line_content);
                    log_info!("📝 Python输出: {}", line_content);

                    // 检查是否需要验证码
                    if line_content.contains("等待验证码")
                        || line_content.contains("request_verification_code")
                        || line_content.contains("需要邮箱验证码")
                        || line_content.contains("请输入验证码")
                    {
                        log_debug!("🔍 检测到验证码请求，开始从Outlook获取验证码...");

                        // 启动验证码获取任务
                        let app_task = app_for_stdout.clone();
                        let email_task = email_for_stdout.clone();
                        let code_file_task = code_file_for_thread.clone();
                        let task_id_task = task_id_for_stdout.clone();
                        std::thread::spawn(move || {
                            // 使用tokio运行时
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                // 等待一小段时间让邮件到达
                                tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

                                for attempt in 1..=10 {
                                    log_debug!("🔍 第{}次尝试获取Outlook验证码...", attempt);

                                    match get_verification_code_from_outlook(&email_task).await {
                                        Ok(code) => {
                                            log_info!("🎯 自动获取到验证码: {}", code);

                                            // 将验证码写入任务专属的临时文件
                                            if let Err(e) = std::fs::write(&code_file_task, &code) {
                                                log_error!("❌ 写入验证码文件失败: {}", e);
                                                return;
                                            }

                                            // 发送验证码到前端
                                            if let Err(e) =
                                                app_task.emit("verification-code-received", &code)
                                            {
                                                log_error!("❌ 发送验证码事件失败: {}", e);
                                            }

                                            log_info!(
                                                "✅ 验证码已自动填入临时文件: {}",
                                                code_file_task
                                            );
                                            return;
                                        }
                                        Err(e) => {
                                            log_debug!("🔍 第{}次获取验证码失败: {}", attempt, e);
                                            if attempt < 10 {
                                                std::thread::sleep(std::time::Duration::from_secs(
                                                    10,
                                                ));
                                            }
                                        }
                                    }
                                }

                                log_error!("❌ 自动获取验证码失败，已尝试10次，请用户手动输入");
                                if let Err(emit_err) = app_task.emit(
                                    "verification-code-manual-input-required",
                                    serde_json::json!({
                                        "message": "自动获取验证码失败，请手动输入验证码",
                                        "task_id": task_id_task,
                                        "email": email_task
                                    }),
                                ) {
                                    log_error!("❌ 发送手动输入提示事件失败: {}", emit_err);
                                }
                            });
                        });
                    }

                    // 发送实时输出到前端
                    if let Err(e) = app_for_stdout.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    ) {
                        log_error!("❌ 发送输出事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取Python输出失败: {}", e);
                    break;
                }
            }
        }
    });

    // 处理stderr
    let app_for_stderr = app.clone();
    let _stderr_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr);

        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    log_info!("📝 Python错误: {}", line_content);

                    // 发送错误输出到前端
                    if let Err(e) = app_for_stderr.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    ) {
                        log_error!("❌ 发送错误输出事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取Python错误输出失败: {}", e);
                    break;
                }
            }
        }
    });

    // // 等待进程完成
    // let exit_status = child
    //     .wait()
    //     .map_err(|e| format!("等待注册进程完成失败: {}", e))?;

    // log_debug!("🔍 Python进程已结束");

    // // 等待输出读取任务完成
    // let _ = stdout_task.join();
    // let _ = stderr_task.join();

    // log_debug!("🔍 [DEBUG] 注册完成");
    // log_info!("    退出代码: {:?}", exit_status.code());

    // // 构建返回结果
    // let result = if exit_status.success() {
    //     serde_json::json!({
    //         "success": false,
    //         "message": "进程关闭"
    //     })
    // } else {
    //     serde_json::json!({
    //         "success": false,
    //         "message": "进程关闭",
    //         "exit_code": exit_status.code()
    //     })
    // };

    // 4. 等待注册进程完成
    let exit_status = child
        .wait()
        .map_err(|e| format!("等待Python脚本执行失败: {}", e))?;

    log_debug!("🔍 Python进程已结束");

    // 从进程管理器中移除已结束的进程
    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.remove(&task_id);
        log_info!("📝 已从管理器移除进程: task_id={}", task_id);
    }

    // 等待输出读取任务完成
    let _ = stdout_task.join();

    // 6. 处理进程退出状态
    if !exit_status.success() {
        log_error!("❌ Python脚本执行失败，退出码: {:?}", exit_status.code());
        return Err(format!(
            "Python脚本执行失败，退出码: {:?}",
            exit_status.code()
        ));
    }

    // 7. 由于我们已经通过实时输出获取了所有信息，这里需要从最后的输出中解析结果
    // 我们可以通过检查临时文件或其他方式来获取最终结果
    // 简化处理：返回一个成功的结果，具体的注册状态通过实时输出已经传递给前端
    let result = serde_json::json!({
        "success": false,
        "message": "注册进程已退出",
        "email": email,
        "email_type": "outlook-default"
    });

    // 注意：不再需要恢复配置，因为现在通过config传递cardIndex，不会修改配置文件

    Ok(result)
}

#[tauri::command]
async fn batch_register_codex_with_email_parallel(
    app: tauri::AppHandle,
    emails: Vec<String>,
    first_names: Vec<String>,
    last_names: Vec<String>,
    self_hosted_mail_url: Option<String>,
    self_hosted_mail_headers_json: Option<String>,
    self_hosted_mail_response_path: Option<String>,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    config: Option<serde_json::Value>,
    max_concurrent: Option<usize>,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 批量注册 {} 个 Codex 账户（并行模式）...", emails.len());

    if emails.len() != first_names.len() || emails.len() != last_names.len() {
        return Err("邮箱、姓名数量不一致".to_string());
    }

    let url = self_hosted_mail_url.unwrap_or_default();
    let hdr = self_hosted_mail_headers_json.unwrap_or_default();
    let path = self_hosted_mail_response_path
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "results[0].raw".to_string());

    if url.trim().is_empty() || hdr.trim().is_empty() {
        return Err("Codex 自建邮箱模式需配置 API 请求 URL 与 Headers（JSON 对象）".to_string());
    }

    let total_count = emails.len();
    let max_concurrent_tasks = max_concurrent.unwrap_or(3);
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent_tasks));
    let mut tasks = Vec::new();
    let clear_enabled_for_parallel = if total_count > 1 {
        log_warn!("⚠️ Codex 并行批量注册已自动禁用清空邮箱，避免任务间互相干扰验证码");
        false
    } else {
        self_hosted_mail_clear_enabled.unwrap_or(false)
    };

    for i in 0..total_count {
        let app_clone = app.clone();
        let email = emails[i].clone();
        let first_name = first_names[i].clone();
        let last_name = last_names[i].clone();
        let url_clone = url.clone();
        let hdr_clone = hdr.clone();
        let path_clone = path.clone();
        let clear_enabled_clone = Some(clear_enabled_for_parallel);
        let clear_url_clone = self_hosted_mail_clear_url.clone();
        let clear_headers_clone = self_hosted_mail_clear_headers_json.clone();
        let clear_method_clone = self_hosted_mail_clear_method.clone();
        let use_incognito_clone = use_incognito;
        let config_clone = config.clone();
        let semaphore_clone = semaphore.clone();

        let task = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();
            let result = register_codex_with_self_hosted_mail_api(
                app_clone.clone(),
                email.clone(),
                first_name.clone(),
                last_name.clone(),
                url_clone,
                hdr_clone,
                path_clone,
                clear_enabled_clone,
                clear_url_clone,
                clear_headers_clone,
                clear_method_clone,
                use_incognito_clone,
                Some(false),
                config_clone,
            )
            .await;

            let actual_email = match &result {
                Ok(result_data) => result_data
                    .get("accountInfo")
                    .and_then(|info| info.get("email"))
                    .and_then(|e| e.as_str())
                    .unwrap_or(&email)
                    .to_string(),
                Err(_) => email.clone(),
            };

            match result {
                Ok(result) => Ok(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": true,
                    "result": result
                })),
                Err(e) => Err(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": false,
                    "error": e
                })),
            }
        });

        tasks.push(task);

        if i < total_count - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for task in tasks {
        match task.await {
            Ok(Ok(result)) => {
                results.push(result);
                if let Some(item) = results.last() {
                    emit_batch_registration_progress(
                        &app,
                        "codex",
                        "parallel",
                        item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        item.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                        true,
                        None,
                        results.len() + errors.len(),
                        total_count,
                        results.len(),
                        errors.len(),
                    );
                }
            },
            Ok(Err(error)) => {
                errors.push(error);
                if let Some(item) = errors.last() {
                    emit_batch_registration_progress(
                        &app,
                        "codex",
                        "parallel",
                        item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        item.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                        false,
                        item.get("error").and_then(|v| v.as_str()),
                        results.len() + errors.len(),
                        total_count,
                        results.len(),
                        errors.len(),
                    );
                }
            },
            Err(e) => {
                errors.push(serde_json::json!({
                "success": false,
                "error": format!("任务执行失败: {}", e)
                }));
                emit_batch_registration_progress(
                    &app,
                    "codex",
                    "parallel",
                    results.len() + errors.len() - 1,
                    "",
                    false,
                    Some(&format!("浠诲姟鎵ц澶辫触: {}", e)),
                    results.len() + errors.len(),
                    total_count,
                    results.len(),
                    errors.len(),
                );
            }
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "total": total_count,
        "succeeded": results.len(),
        "failed": errors.len(),
        "results": results,
        "errors": errors
    }))
}

#[tauri::command]
async fn batch_register_codex_with_email(
    app: tauri::AppHandle,
    emails: Vec<String>,
    first_names: Vec<String>,
    last_names: Vec<String>,
    self_hosted_mail_url: Option<String>,
    self_hosted_mail_headers_json: Option<String>,
    self_hosted_mail_response_path: Option<String>,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    config: Option<serde_json::Value>,
    batch_delay_seconds: Option<u64>,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 批量注册 {} 个 Codex 账户（串行模式）...", emails.len());
    let batch_delay_seconds = batch_delay_seconds.unwrap_or(10).min(600);

    if emails.len() != first_names.len() || emails.len() != last_names.len() {
        return Err("邮箱、姓名数量不一致".to_string());
    }

    let url = self_hosted_mail_url.unwrap_or_default();
    let hdr = self_hosted_mail_headers_json.unwrap_or_default();
    let path = self_hosted_mail_response_path
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "results[0].raw".to_string());

    if url.trim().is_empty() || hdr.trim().is_empty() {
        return Err("Codex 自建邮箱模式需配置 API 请求 URL 与 Headers（JSON 对象）".to_string());
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for i in 0..emails.len() {
        if i > 0 {
            log_info!(
                "⏳ [任务 {}/{}] 等待 {} 秒后开始下一个 Codex 注册任务...",
                i + 1,
                emails.len(),
                batch_delay_seconds
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(batch_delay_seconds)).await;
        }

        let email = emails[i].clone();
        let first_name = first_names[i].clone();
        let last_name = last_names[i].clone();

        let result = register_codex_with_self_hosted_mail_api(
            app.clone(),
            email.clone(),
            first_name,
            last_name,
            url.clone(),
            hdr.clone(),
            path.clone(),
            self_hosted_mail_clear_enabled,
            self_hosted_mail_clear_url.clone(),
            self_hosted_mail_clear_headers_json.clone(),
            self_hosted_mail_clear_method.clone(),
            use_incognito,
            Some(false),
            config.clone(),
        )
        .await;

        let actual_email = match &result {
            Ok(result_data) => result_data
                .get("accountInfo")
                .and_then(|info| info.get("email"))
                .and_then(|e| e.as_str())
                .unwrap_or(&email)
                .to_string(),
            Err(_) => email.clone(),
        };

        let (task_success, task_error_message) = match result {
            Ok(result) => {
                results.push(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": true,
                    "result": result
                }));
                (true, None)
            }
            Err(e) => {
                errors.push(serde_json::json!({
                    "index": i,
                    "email": actual_email,
                    "success": false,
                    "error": e
                }));
                (
                    false,
                    errors
                        .last()
                        .and_then(|item| item.get("error").and_then(|value| value.as_str())),
                )
            }
        };

        emit_batch_registration_progress(
            &app,
            "codex",
            "serial",
            i,
            &actual_email,
            task_success,
            task_error_message,
            results.len() + errors.len(),
            emails.len(),
            results.len(),
            errors.len(),
        );

    }

    Ok(serde_json::json!({
        "success": true,
        "total": emails.len(),
        "succeeded": results.len(),
        "failed": errors.len(),
        "results": results,
        "errors": errors
    }))
}

/// 使用自建邮箱 API（可配置 URL、Headers、响应 JSON 路径）自动拉取邮件原文并提取验证码
#[tauri::command]
async fn register_codex_with_self_hosted_mail_api(
    app: tauri::AppHandle,
    email: String,
    first_name: String,
    last_name: String,
    self_hosted_mail_url: String,
    self_hosted_mail_headers_json: String,
    self_hosted_mail_response_path: String,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    manual_verification: Option<bool>,
    config: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let use_manual_verification = manual_verification.unwrap_or(false);
    log_info!(
        "🔄 使用 Codex 注册账户（验证码模式: {}）...",
        if use_manual_verification {
            "手动输入"
        } else {
            "自建邮箱 API 自动获取"
        }
    );
    log_info!("📧 注册邮箱: {}", email);
    if !use_manual_verification {
        log_info!("🔗 API URL: {}", self_hosted_mail_url);
    }
    log_info!("👤 姓名: {} {}", first_name, last_name);

    let executable_path = get_python_executable_path_by_name("cdp_flow_runner")?;
    if !executable_path.exists() {
        return Err(format!(
            "找不到 Codex Python 可执行文件: {:?}",
            executable_path
        ));
    }

    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };

    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();
    let continue_file = temp_dir.join(format!("cdp_flow_continue_{}.txt", task_id));
    let continue_file_path = continue_file.to_string_lossy().to_string();
    let cancel_file_path = stop_file_path.clone();

    let mut final_config = if let Some(frontend_config) = config {
        frontend_config
    } else {
        serde_json::json!({})
    };
    let require_manual_confirm_before_post_oauth = final_config
        .get("manualConfirmBeforePostOauth")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let cdp_overrides = final_config
        .get("codexCdpOverrides")
        .cloned()
        .map(|value| {
            serde_json::from_value::<CodexCdpOverrides>(value)
                .map_err(|e| format!("解析 Codex CDP 覆盖配置失败: {}", e))
        })
        .transpose()?;
    if let Some(config_obj) = final_config.as_object_mut() {
        config_obj.remove("manualConfirmBeforePostOauth");
        config_obj.remove("codexCdpOverrides");
    }
    let config_json_str =
        serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());
    let email_config = get_email_config().await.ok();
    let access_password = email_config
        .as_ref()
        .map(|cfg| cfg.access_password.clone())
        .unwrap_or_else(|| "AUTOCURSOT_WUQI_2002".to_string());
    let cdp_url = cdp_overrides
        .as_ref()
        .and_then(|overrides| overrides.url.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://chatgpt.com/".to_string());
    let cdp_steps = match cdp_overrides.as_ref().and_then(|overrides| overrides.steps.as_ref()) {
        Some(steps) => parse_codex_cdp_cli_steps(steps, &email, &access_password)?,
        None => default_codex_cdp_cli_steps(&email, &access_password),
    };
    let element_timeout = cdp_overrides
        .as_ref()
        .and_then(|overrides| overrides.element_timeout)
        .unwrap_or(20.0);
    let wait_after_open = cdp_overrides
        .as_ref()
        .and_then(|overrides| overrides.wait_after_open)
        .unwrap_or(2.0);
    let wait_after_action = cdp_overrides
        .as_ref()
        .and_then(|overrides| overrides.wait_after_action)
        .unwrap_or(1.8);
    let post_oauth_step1_py = cdp_overrides
        .as_ref()
        .and_then(|overrides| overrides.post_oauth_step1_py.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "openai_oauth_step1.py".to_string());

    let mut cmd = create_hidden_command(&executable_path.to_string_lossy());
    cmd.arg("--url")
        .arg(&cdp_url)
        .arg("--incognito")
        .arg(incognito_flag)
        .arg("--custom-config-json")
        .arg(&config_json_str)
        .arg("--wait-after-open")
        .arg(wait_after_open.to_string())
        .arg("--element-timeout")
        .arg(element_timeout.to_string())
        .arg("--wait-after-action")
        .arg(wait_after_action.to_string())
        .arg("--post-oauth-step1-py")
        .arg(&post_oauth_step1_py)
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path)
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path)
        .env("CDP_FLOW_CONTINUE_FILE", &continue_file_path)
        .env("CDP_FLOW_CANCEL_FILE", &cancel_file_path)
        .env(
            "CDP_FLOW_REQUIRE_MANUAL_CONFIRM_BEFORE_POST_OAUTH",
            if require_manual_confirm_before_post_oauth {
                "1"
            } else {
                "0"
            },
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for step in cdp_steps {
        match step {
            CodexCdpCliStep::Click {
                selector,
                wait_for_load,
            } => {
                if wait_for_load {
                    cmd.arg("--click-wait-load").arg(selector);
                } else {
                    cmd.arg("--click").arg(selector);
                }
            }
            CodexCdpCliStep::Input { selector, value } => {
                cmd.arg("--input").arg(format!("{}={}", selector, value));
            }
        }
    }

    log_info!("🚀 启动 Codex 可执行文件: {:?}", executable_path);
    log_info!("🧩 Codex 配置 JSON: {}", config_json_str);
    log_info!("🧭 Codex CDP URL: {}", cdp_url);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 Codex 注册进程: {}", e))?;

    let pid = child.id();
    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.insert(task_id.clone(), pid);
    }
    log_info!("✅ 已记录 Codex 注册进程: task_id={}, pid={}", task_id, pid);

    let stdout = child.stdout.take().ok_or("无法获取stdout".to_string())?;
    let stderr = child.stderr.take().ok_or("无法获取stderr".to_string())?;

    let api_url = self_hosted_mail_url.clone();
    let api_headers = self_hosted_mail_headers_json.clone();
    let api_path = self_hosted_mail_response_path.clone();
    let clear_enabled = self_hosted_mail_clear_enabled.unwrap_or(false);
    let clear_url = self_hosted_mail_clear_url.clone();
    let clear_headers = self_hosted_mail_clear_headers_json.clone();
    let clear_method = self_hosted_mail_clear_method.clone();

    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let self_hosted_fetch_in_progress = Arc::new(AtomicBool::new(false));

    let app_for_stdout = app.clone();
    let email_for_stdout = email.clone();
    let task_id_for_stdout = task_id.clone();
    let code_file_for_thread = code_file_path.clone();
    let stdout_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stdout);
        let self_hosted_fetch_in_progress = self_hosted_fetch_in_progress.clone();
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line_content = decode_python_output(&buffer);
                    log_info!("📝 Codex Python输出: {}", line_content);

                    if line_content.contains("等待验证码")
                        || line_content.contains("request_verification_code")
                        || line_content.contains("需要邮箱验证码")
                        || line_content.contains("请输入验证码")
                    {
                        if use_manual_verification {
                            let _ = app_for_stdout.emit(
                                "verification-code-required",
                                serde_json::json!({
                                    "message": "请输入验证码",
                                    "task_id": task_id_for_stdout,
                                    "email": email_for_stdout
                                }),
                            );
                            continue;
                        }
                        log_info!(
                            "🔍 Codex 检测到验证码请求: task_id={}, email={}",
                            task_id_for_stdout,
                            email_for_stdout
                        );
                        if self_hosted_fetch_in_progress
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_err()
                        {
                            log_debug!(
                                "🔍 [Codex 自建邮箱] 已有验证码拉取任务在运行，跳过重复触发"
                            );
                        } else {
                            let api_url_task = api_url.clone();
                            let api_headers_task = api_headers.clone();
                            let api_path_task = api_path.clone();
                            let clear_url_task = clear_url.clone();
                            let clear_headers_task = clear_headers.clone();
                            let clear_method_task = clear_method.clone();
                            let app_task = app_for_stdout.clone();
                            let code_file_task = code_file_for_thread.clone();
                            let task_id_task = task_id_for_stdout.clone();
                            let email_task = email_for_stdout.clone();
                            let fetch_flag = self_hosted_fetch_in_progress.clone();

                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                rt.block_on(async {
                                    let fetch_result =
                                        get_verification_code_from_self_hosted_mail_api(
                                            &api_url_task,
                                            &api_headers_task,
                                            &api_path_task,
                                            Some(&email_task),
                                            clear_enabled,
                                            clear_url_task.as_deref(),
                                            clear_headers_task.as_deref(),
                                            clear_method_task.as_deref(),
                                        )
                                        .await;
                                    fetch_flag.store(false, Ordering::SeqCst);

                                    match fetch_result {
                                        Ok(code) => {
                                            log_info!("🎯 Codex 自动获取到验证码: {}", code);
                                            let _ = std::fs::write(&code_file_task, &code);
                                            let _ = app_task
                                                .emit("verification-code-auto-filled", &code);
                                        }
                                        Err(e) => {
                                            log_error!("❌ Codex 自动获取验证码失败: {}", e);
                                            let event_name = if e.contains("超时") {
                                                "verification-code-timeout"
                                            } else {
                                                "verification-code-manual-input-required"
                                            };
                                            let _ = app_task.emit(
                                                event_name,
                                                serde_json::json!({
                                                    "message": if e.contains("超时") {
                                                        "自动获取验证码超时，请手动输入验证码"
                                                    } else {
                                                        "自动获取验证码失败，请手动输入验证码"
                                                    },
                                                    "task_id": task_id_task,
                                                    "email": email_task
                                                }),
                                            );
                                        }
                                    }
                                });
                            });
                        }
                    }

                    if let Ok(event_payload) =
                        serde_json::from_str::<serde_json::Value>(&line_content)
                    {
                        if event_payload
                            .get("action")
                            .and_then(|value| value.as_str())
                            == Some("wait_for_user")
                        {
                            let _ = app_for_stdout.emit(
                                "codex-manual-step-required",
                                serde_json::json!({
                                    "task_id": task_id_for_stdout,
                                    "email": email_for_stdout,
                                    "reason": event_payload.get("reason").and_then(|value| value.as_str()).unwrap_or("manual_step"),
                                    "message": event_payload.get("message").and_then(|value| value.as_str()).unwrap_or("请完成手动步骤后继续"),
                                    "continue_file": event_payload.get("continue_file").and_then(|value| value.as_str()),
                                    "cancel_file": event_payload.get("cancel_file").and_then(|value| value.as_str()),
                                    "status": event_payload.get("status").and_then(|value| value.as_str()).unwrap_or("waiting")
                                }),
                            );
                        }
                    }

                    let _ = app_for_stdout.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    );
                }
                Err(e) => {
                    log_error!("❌ 读取 Codex Python 输出失败: {}", e);
                    break;
                }
            }
        }
    });

    let app_for_stderr = app.clone();
    let task_id_for_stderr = task_id.clone();
    let email_for_stderr = email.clone();
    let _stderr_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(stderr);
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let line_content = decode_python_output(&buffer);
                    log_info!("📝 Codex Python错误: {}", line_content);
                    if line_content.contains("manual_input_required") {
                        let _ = app_for_stderr.emit(
                            "verification-code-manual-input-required",
                            serde_json::json!({
                                "message": "自动获取验证码失败，请手动输入验证码",
                                "task_id": task_id_for_stderr,
                                "email": email_for_stderr
                            }),
                        );
                    }
                    let _ = app_for_stderr.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    );
                }
                Err(e) => {
                    log_error!("❌ 读取 Codex Python 错误输出失败: {}", e);
                    break;
                }
            }
        }
    });

    let exit_status = child
        .wait()
        .map_err(|e| format!("等待 Codex Python 脚本执行失败: {}", e))?;

    let _ = stdout_task.join();
    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.remove(&task_id);
    }

    if !exit_status.success() {
        let script_path = get_app_dir()?
            .join("python_scripts")
            .join("cdp_flow_runner.py");
        let packaged_hint = format!("打包可执行文件: {:?}", executable_path);
        let source_hint = format!("源码脚本: {:?}", script_path);
        return Err(format!(
            "Codex Python脚本执行失败，退出码: {:?}。{}。{}。如果日志只显示到 cdp_flow_runner.py 第 1033 行附近，通常是打包后的 exe 内部运行时错误，建议重新打包 src-tauri/python_scripts/build_executable.py 后再测试。",
            exit_status.code(),
            packaged_hint,
            source_hint,
        ));
    }

    Ok(serde_json::json!({
        "success": true,
        "message": "Codex 注册流程已完成",
        "email": email,
        "email_type": "codex_self_hosted_mail_api"
    }))
}

#[tauri::command]
async fn register_with_self_hosted_mail_api(
    app: tauri::AppHandle,
    email: String,
    first_name: String,
    last_name: String,
    self_hosted_mail_url: String,
    self_hosted_mail_headers_json: String,
    self_hosted_mail_response_path: String,
    self_hosted_mail_clear_enabled: Option<bool>,
    self_hosted_mail_clear_url: Option<String>,
    self_hosted_mail_clear_headers_json: Option<String>,
    self_hosted_mail_clear_method: Option<String>,
    use_incognito: Option<bool>,
    enable_bank_card_binding: Option<bool>,
    skip_phone_verification: Option<bool>,
    selected_card_index: Option<u32>,
    config: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 使用自建邮箱 API 注册 Cursor 账户...");
    log_info!("📧 注册邮箱: {}", email);
    log_info!("🔗 API URL: {}", self_hosted_mail_url);
    log_info!("👤 姓名: {} {}", first_name, last_name);

    let custom_browser_path = {
        let restorer = MachineIdRestorer::new()
            .map_err(|e| format!("Failed to initialize restorer: {}", e))?;
        restorer.get_custom_browser_path()
    };

    if enable_bank_card_binding.unwrap_or(true) {
        log_info!("💳 准备设置银行卡配置...");
        let bank_card_config = read_bank_card_config().await?;
        let bank_card_data: serde_json::Value = serde_json::from_str(&bank_card_config)
            .map_err(|e| format!("解析银行卡配置失败: {}", e))?;

        let all_cards =
            if let Some(cards_array) = bank_card_data.get("cards").and_then(|v| v.as_array()) {
                cards_array.clone()
            } else {
                vec![bank_card_data.clone()]
            };

        if all_cards.is_empty() {
            return Err("银行卡配置为空，请先配置至少一张银行卡".to_string());
        }

        let card_index = selected_card_index.unwrap_or(0) as usize;
        if card_index >= all_cards.len() {
            return Err(format!(
                "银行卡索引 {} 超出范围（总共 {} 张卡）",
                card_index,
                all_cards.len()
            ));
        }

        log_info!(
            "✅ 将使用卡片索引 {} 进行注册（通过config传递给Python）",
            card_index
        );
    }

    let executable_path = get_python_executable_path()?;
    if !executable_path.exists() {
        return Err(format!("找不到Python可执行文件: {:?}", executable_path));
    }

    let incognito_flag = if use_incognito.unwrap_or(true) {
        "true"
    } else {
        "false"
    };
    let bank_card_flag = if enable_bank_card_binding.unwrap_or(true) {
        "true"
    } else {
        "false"
    };
    let skip_phone_flag = if skip_phone_verification.unwrap_or(false) {
        "1"
    } else {
        "0"
    };

    let app_dir = get_app_dir()?;
    let app_dir_str = app_dir.to_string_lossy().to_string();
    let app_dir_base64 = general_purpose::STANDARD.encode(&app_dir_str);

    let final_config = if let Some(frontend_config) = config {
        let mut config_obj = serde_json::json!({
            "btnIndex": frontend_config.get("btnIndex").and_then(|v| v.as_u64()).unwrap_or(1),
            "subscriptionTier": frontend_config.get("subscriptionTier").and_then(|v| v.as_str()).unwrap_or("pro"),
            "allowAutomaticPayment": frontend_config.get("allowAutomaticPayment").and_then(|v| v.as_bool()).unwrap_or(true),
            "allowTrial": frontend_config.get("allowTrial").and_then(|v| v.as_bool()).unwrap_or(true),
            "useApiForBindCard": frontend_config.get("useApiForBindCard").and_then(|v| v.as_u64()).unwrap_or(1),
            "cardIndex": selected_card_index.unwrap_or(0)
        });
        if let Some(proxy_config) = frontend_config.get("proxy") {
            config_obj["proxy"] = proxy_config.clone();
        }
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
        }
        merge_haozhuma_config_into_runtime_config(&mut config_obj, Some(&frontend_config));
        config_obj
    } else {
        let mut config_obj = serde_json::json!({
            "btnIndex": 1,
            "subscriptionTier": "pro",
            "allowAutomaticPayment": true,
            "allowTrial": true,
            "useApiForBindCard": 1,
            "cardIndex": selected_card_index.unwrap_or(0)
        });
        if let Some(browser_path) = &custom_browser_path {
            config_obj["custom_browser_path"] = serde_json::Value::String(browser_path.clone());
        }
        merge_haozhuma_config_into_runtime_config(&mut config_obj, None);
        config_obj
    };

    let config_json_str = serde_json::to_string(&final_config).unwrap_or_else(|_| "{}".to_string());

    let task_id = generate_task_id(&email);
    let temp_dir = std::env::temp_dir();
    let code_file = temp_dir.join(format!("cursor_verification_code_{}.txt", task_id));
    let code_file_path = code_file.to_string_lossy().to_string();

    // 停止信号文件（用于 cancel_registration）
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));
    let stop_file_path = stop_file.to_string_lossy().to_string();

    log_info!("🆔 任务ID: {}", task_id);
    log_info!("📄 验证码文件: {}", code_file_path);

    let mut cmd = create_hidden_command(&executable_path.to_string_lossy());
    cmd.arg(&email)
        .arg(&first_name)
        .arg(&last_name)
        .arg(incognito_flag)
        .arg(&app_dir_base64)
        .arg(bank_card_flag)
        .arg(skip_phone_flag)
        .arg(&config_json_str)
        .env("CURSOR_VERIFICATION_CODE_FILE", &code_file_path)
        .env("CURSOR_REGISTRATION_STOP_FILE", &stop_file_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动注册进程: {}", e))?;

    {
        let pid = child.id();
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.insert(task_id.clone(), pid);
        log_info!("📝 已注册进程到管理器: task_id={}, pid={}", task_id, pid);
    }

    let stdout = child.stdout.take().ok_or("无法获取stdout".to_string())?;
    let stderr = child.stderr.take().ok_or("无法获取stderr".to_string())?;

    let api_url = self_hosted_mail_url.clone();
    let api_headers = self_hosted_mail_headers_json.clone();
    let api_path = self_hosted_mail_response_path.clone();
    let clear_enabled = self_hosted_mail_clear_enabled.unwrap_or(false);
    let clear_url = self_hosted_mail_clear_url.clone();
    let clear_headers = self_hosted_mail_clear_headers_json.clone();
    let clear_method = self_hosted_mail_clear_method.clone();

    let app_clone = app.clone();
    let email_clone = email.clone();
    let code_file_for_thread = code_file_path.clone();
    let task_id_clone = task_id.clone();

    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let self_hosted_fetch_in_progress = Arc::new(AtomicBool::new(false));

    let app_for_stdout = app_clone.clone();
    let email_for_stdout = email_clone.clone();
    let task_id_for_stdout = task_id_clone.clone();
    let stdout_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);
        let self_hosted_fetch_in_progress = self_hosted_fetch_in_progress.clone();

        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    let line_content = sanitize_python_output_line(&line_content);
                    log_info!("📝 Python输出: {}", line_content);

                    if line_content.contains("等待验证码")
                        || line_content.contains("request_verification_code")
                        || line_content.contains("需要邮箱验证码")
                        || line_content.contains("请输入验证码")
                    {
                        if self_hosted_fetch_in_progress
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_err()
                        {
                            log_debug!("🔍 [自建邮箱] 已有验证码拉取任务在运行，跳过重复触发");
                        } else {
                            let api_url_task = api_url.clone();
                            let api_headers_task = api_headers.clone();
                            let api_path_task = api_path.clone();
                            let clear_url_task = clear_url.clone();
                            let clear_headers_task = clear_headers.clone();
                            let clear_method_task = clear_method.clone();
                            let app_task = app_for_stdout.clone();
                            let code_file_task = code_file_for_thread.clone();
                            let task_id_task = task_id_for_stdout.clone();
                            let email_task = email_for_stdout.clone();
                            let fetch_flag = self_hosted_fetch_in_progress.clone();

                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().unwrap();
                                rt.block_on(async {
                                    let fetch_result =
                                        get_verification_code_from_self_hosted_mail_api(
                                            &api_url_task,
                                            &api_headers_task,
                                            &api_path_task,
                                            Some(&email_task),
                                            clear_enabled,
                                            clear_url_task.as_deref(),
                                            clear_headers_task.as_deref(),
                                            clear_method_task.as_deref(),
                                        )
                                        .await;
                                    fetch_flag.store(false, Ordering::SeqCst);

                                    match fetch_result {
                                        Ok(code) => {
                                            log_info!("🎯 自动获取到验证码: {}", code);
                                            if let Err(e) = std::fs::write(&code_file_task, &code) {
                                                log_error!("❌ 写入验证码文件失败: {}", e);
                                                return;
                                            }
                                            if let Err(e) = app_task
                                                .emit("verification-code-auto-filled", &code)
                                            {
                                                log_error!("❌ 发送验证码事件失败: {}", e);
                                            }
                                            log_info!("✅ 验证码已写入临时文件");
                                        }
                                        Err(e) => {
                                            log_error!("❌ 自建邮箱 API 自动获取验证码失败: {}", e);
                                            let event_name = if e.contains("超时") {
                                                "verification-code-timeout"
                                            } else {
                                                "verification-code-manual-input-required"
                                            };
                                            let _ = app_task.emit(
                                                event_name,
                                                serde_json::json!({
                                                    "message": if e.contains("超时") {
                                                        "自动获取验证码超时，请手动输入验证码"
                                                    } else {
                                                        "自动获取验证码失败，请手动输入验证码"
                                                    },
                                                    "task_id": task_id_task,
                                                    "email": email_task
                                                }),
                                            );
                                        }
                                    }
                                });
                            });
                        }
                    }

                    if let Err(e) = app_for_stdout.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    ) {
                        log_error!("❌ 发送输出事件失败: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取Python输出失败: {}", e);
                    break;
                }
            }
        }
    });

    let app_for_stderr = app.clone();
    let _stderr_task = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    log_info!("📝 Python错误: {}", line_content);
                    let _ = app_for_stderr.emit(
                        "registration-output",
                        serde_json::json!({
                            "line": line_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }),
                    );
                }
                Err(e) => {
                    log_error!("❌ 读取Python错误输出失败: {}", e);
                    break;
                }
            }
        }
    });

    let exit_status = child
        .wait()
        .map_err(|e| format!("等待Python脚本执行失败: {}", e))?;

    let _ = stdout_task.join();

    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        processes.remove(&task_id);
        log_info!("📝 已从管理器移除进程: task_id={}", task_id);
    }

    if !exit_status.success() {
        return Err(format!(
            "Python脚本执行失败，退出码: {:?}",
            exit_status.code()
        ));
    }

    Ok(serde_json::json!({
        "success": false,
        "message": "注册进程已退出",
        "email": email,
        "email_type": "self_hosted_mail_api"
    }))
}

#[tauri::command]
async fn submit_verification_code(
    code: String,
    task_id: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!("🔢 接收到验证码: {}", code);

    // 验证验证码格式
    if !code.chars().all(|c| c.is_ascii_digit()) || code.len() != 6 {
        return Err("验证码必须是6位数字".to_string());
    }

    // 将验证码写入临时文件，供Python脚本读取
    let temp_dir = std::env::temp_dir();

    // 如果提供了task_id，使用对应的文件名（用于并行注册）
    // 否则使用默认文件名（向后兼容单个注册）
    let code_file = if let Some(tid) = task_id {
        let filename = format!("cursor_verification_code_{}.txt", tid);
        log_info!("🆔 使用任务ID: {}", tid);
        temp_dir.join(filename)
    } else {
        log_info!("📝 使用默认验证码文件（单个注册模式）");
        temp_dir.join("cursor_verification_code.txt")
    };

    log_info!("📁 临时目录: {:?}", temp_dir);
    log_info!("📄 验证码文件: {:?}", code_file);

    match std::fs::write(&code_file, &code) {
        Ok(_) => {
            log_info!("✅ 验证码已保存到临时文件: {:?}", code_file);
            Ok(serde_json::json!({
                "success": true,
                "message": "验证码已提交"
            }))
        }
        Err(e) => Err(format!("保存验证码失败: {}", e)),
    }
}

#[tauri::command]
async fn signal_registration_continue(
    task_id: String,
    action: Option<String>,
) -> Result<serde_json::Value, String> {
    let signal = action
        .unwrap_or_else(|| "continue".to_string())
        .trim()
        .to_string();
    if signal.is_empty() {
        return Err("继续信号不能为空".to_string());
    }

    let signal_file = std::env::temp_dir().join(format!("cdp_flow_continue_{}.txt", task_id));
    std::fs::write(&signal_file, &signal)
        .map_err(|e| format!("写入继续信号失败: {}", e))?;

    Ok(serde_json::json!({
        "success": true,
        "message": "继续信号已发送",
        "task_id": task_id,
        "action": signal
    }))
}

fn emit_batch_registration_progress(
    app: &tauri::AppHandle,
    provider: &str,
    mode: &str,
    index: usize,
    email: &str,
    success: bool,
    error: Option<&str>,
    completed: usize,
    total: usize,
    succeeded: usize,
    failed: usize,
) {
    let _ = app.emit(
        "batch-registration-progress",
        serde_json::json!({
            "provider": provider,
            "mode": mode,
            "index": index,
            "email": email,
            "success": success,
            "error": error,
            "completed": completed,
            "total": total,
            "succeeded": succeeded,
            "failed": failed
        }),
    );
}

#[tauri::command]
async fn cancel_registration() -> Result<String, String> {
    // 通过文件通知所有正在运行的注册进程停止
    let processes = REGISTRATION_PROCESSES.lock().unwrap();

    if processes.is_empty() {
        log_info!("ℹ️ 没有正在运行的注册进程");
        return Ok("没有正在运行的注册进程".to_string());
    }

    // 克隆任务ID列表
    let task_entries: Vec<(String, u32)> = processes
        .iter()
        .map(|(task_id, pid)| (task_id.clone(), *pid))
        .collect();
    drop(processes); // 释放锁

    let temp_dir = std::env::temp_dir();
    let mut notified_count = 0;
    let mut killed_count = 0;

    // 为每个任务写入停止信号文件
    for (task_id, pid) in &task_entries {
        let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));

        log_info!(
            "🚫 正在写入停止信号文件: task_id={}, file={:?}",
            task_id,
            stop_file
        );

        match fs::write(&stop_file, "stop") {
            Ok(_) => {
                notified_count += 1;
                log_info!("✅ 成功写入停止信号文件: task_id={}", task_id);
            }
            Err(e) => {
                log_error!("❌ 写入停止信号文件失败: task_id={}, error={}", task_id, e);
            }
        }

        // 额外主动终止整个进程树，避免子进程（如 cdp_flow_runner）残留。
        if terminate_process_tree(*pid) {
            killed_count += 1;
            log_info!("🛑 已终止注册进程树: task_id={}, pid={}", task_id, pid);
        } else {
            log_warn!(
                "⚠️ 终止进程树失败或进程已退出: task_id={}, pid={}",
                task_id,
                pid
            );
        }
    }

    // 清理进程表，避免遗留脏 pid
    {
        let mut processes = REGISTRATION_PROCESSES.lock().unwrap();
        for (task_id, _) in &task_entries {
            processes.remove(task_id);
        }
    }

    if notified_count > 0 {
        log_info!(
            "✅ 已通知 {} 个注册进程停止，并强制终止 {} 个进程",
            notified_count,
            killed_count
        );
        Ok(format!(
            "已通知 {} 个注册进程停止，并强制终止 {} 个进程",
            notified_count, killed_count
        ))
    } else {
        log_warn!("⚠️ 未能通知任何注册进程");
        Ok("未能通知任何注册进程".to_string())
    }
}

/// 仅取消/关闭某一个注册任务窗口（只写入对应 task_id 的停止信号文件）
#[tauri::command]
async fn cancel_registration_task(task_id: String) -> Result<String, String> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err("task_id 不能为空".to_string());
    }

    let temp_dir = std::env::temp_dir();
    let stop_file = temp_dir.join(format!("cursor_registration_stop_{}.txt", task_id));

    log_info!(
        "🚫 [CANCEL_TASK] 写入停止信号: task_id={}, file={:?}",
        task_id,
        stop_file
    );

    match std::fs::write(&stop_file, "stop") {
        Ok(_) => Ok(format!("已通知 task_id={} 停止", task_id)),
        Err(e) => Err(format!(
            "写入停止信号文件失败: task_id={}, error={}",
            task_id, e
        )),
    }
}

#[tauri::command]
async fn get_saved_accounts() -> Result<Vec<serde_json::Value>, String> {
    // 获取已保存的账户列表功能暂时不可用
    match AccountManager::load_accounts() {
        Ok(accounts) => {
            // 将AccountInfo转换为serde_json::Value
            let json_accounts: Vec<serde_json::Value> = accounts
                .into_iter()
                .map(|account| serde_json::to_value(account).unwrap_or(serde_json::Value::Null))
                .collect();
            Ok(json_accounts)
        }
        Err(e) => Err(format!("获取保存的账户失败: {}", e)),
    }
}

// Bank Card Configuration Commands
#[tauri::command]
async fn check_and_convert_bank_card_config() -> Result<String, String> {
    log_info!("🔍 检查并转换银行卡配置格式...");

    // 读取当前配置
    let config_str = read_bank_card_config().await?;
    let config_data: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| format!("解析银行卡配置失败: {}", e))?;

    // 检查是否已经是数组格式
    if config_data
        .get("cards")
        .and_then(|v| v.as_array())
        .is_some()
    {
        log_info!("✅ 银行卡配置已经是数组格式，无需转换");
        return Ok("配置格式正确，无需转换".to_string());
    }

    log_info!("🔄 检测到旧格式配置，开始转换为数组格式...");

    // 检查是否包含必需的银行卡字段
    let required_fields = [
        "cardNumber",
        "cardExpiry",
        "cardCvc",
        "billingName",
        "billingCountry",
        "billingPostalCode",
        "billingAdministrativeArea",
        "billingLocality",
        "billingDependentLocality",
        "billingAddressLine1",
    ];

    for field in &required_fields {
        if !config_data
            .get(field)
            .and_then(|v| v.as_str())
            .map_or(false, |s| !s.is_empty())
        {
            return Err(format!("银行卡配置缺少必需字段: {}", field));
        }
    }

    // 转换为数组格式
    let new_config = serde_json::json!({
        "cards": [config_data]
    });

    // 保存转换后的配置
    let new_config_str =
        serde_json::to_string_pretty(&new_config).map_err(|e| format!("序列化配置失败: {}", e))?;

    save_bank_card_config(new_config_str.clone()).await?;

    log_info!("✅ 银行卡配置已成功转换为数组格式并保存");
    Ok("配置已转换为数组格式".to_string())
}

#[tauri::command]
async fn read_bank_card_config() -> Result<String, String> {
    read_config_with_legacy_fallback("bank_card_config.json")
}

#[tauri::command]
async fn save_bank_card_config(config: String) -> Result<(), String> {
    use std::fs;

    let config_path = get_primary_config_path("bank_card_config.json")?;

    // 验证JSON格式
    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|e| format!("Invalid JSON format: {}", e))?;

    fs::write(&config_path, config)
        .map_err(|e| format!("Failed to save bank card config: {}", e))?;

    log_info!("✅ 银行卡配置已保存到: {:?}", config_path);
    Ok(())
}

// 备份银行卡配置
async fn backup_bank_card_config() -> Result<String, String> {
    use std::fs;

    let config_path = get_primary_config_path("bank_card_config.json")?;
    let backup_path = get_primary_config_path("bank_card_config.backup.json")?;

    if config_path.exists() {
        let config_content =
            fs::read_to_string(&config_path).map_err(|e| format!("读取银行卡配置失败: {}", e))?;

        fs::write(&backup_path, &config_content)
            .map_err(|e| format!("备份银行卡配置失败: {}", e))?;

        log_info!("✅ 银行卡配置已备份到: {:?}", backup_path);
        Ok(config_content)
    } else {
        Ok(String::new())
    }
}

// 恢复银行卡配置
async fn restore_bank_card_config() -> Result<(), String> {
    use std::fs;

    let config_path = get_primary_config_path("bank_card_config.json")?;
    let backup_path = get_primary_config_path("bank_card_config.backup.json")?;

    if backup_path.exists() {
        let backup_content =
            fs::read_to_string(&backup_path).map_err(|e| format!("读取备份配置失败: {}", e))?;

        if !backup_content.is_empty() {
            fs::write(&config_path, backup_content)
                .map_err(|e| format!("恢复银行卡配置失败: {}", e))?;

            log_info!("✅ 银行卡配置已从备份恢复");
        }

        // 删除备份文件
        let _ = fs::remove_file(&backup_path);
    }

    Ok(())
}

// Email Configuration Commands
#[tauri::command]
async fn read_email_config() -> Result<String, String> {
    read_config_with_legacy_fallback("email_config.json")
}

#[tauri::command]
async fn save_email_config(config: String) -> Result<(), String> {
    use std::fs;

    let config_path = get_primary_config_path("email_config.json")?;

    // 验证JSON格式
    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|e| format!("Invalid JSON format: {}", e))?;

    fs::write(&config_path, config).map_err(|e| format!("Failed to save email config: {}", e))?;

    log_info!("✅ 邮箱配置已保存到: {:?}", config_path);
    Ok(())
}

fn read_haozhuma_config_sync() -> Result<String, String> {
    read_config_with_legacy_fallback("haozhuma_config.json")
}

#[tauri::command]
async fn read_haozhuma_config() -> Result<String, String> {
    read_haozhuma_config_sync()
}

#[tauri::command]
async fn save_haozhuma_config(config: String) -> Result<(), String> {
    let config_path = get_primary_config_path("haozhuma_config.json")?;

    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|e| format!("Invalid JSON format: {}", e))?;

    fs::write(&config_path, config).map_err(|e| format!("Failed to save haozhuma config: {}", e))?;
    log_info!("✅ 豪猪配置已保存到: {:?}", config_path);
    Ok(())
}

#[derive(Debug, Serialize)]
struct HaozhumaTestResult {
    success: bool,
    message: String,
    token_len: Option<usize>,
    phone_last4: Option<String>,
}

fn extract_digits(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn extract_token_from_text(text: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        for key in ["token", "Token", "data", "result"] {
            if let Some(val) = v.get(key) {
                if let Some(s) = val.as_str() {
                    let s = s.trim();
                    if s.len() >= 8 {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }

    for sep in ["|", ",", ";"] {
        if text.contains(sep) {
            let parts: Vec<&str> = text.split(sep).collect();
            for part in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
                if part.len() >= 8 {
                    return Some(part.to_string());
                }
            }
        }
    }

    // Fallback: long enough contiguous alnum chunk
    let token_re = Regex::new(r"([A-Za-z0-9]{8,})").ok()?;
    token_re
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

async fn haozhuma_test_login(
    api_domain: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let base = api_domain.trim();
    let base = if base.starts_with("http://") || base.starts_with("https://") {
        base.to_string()
    } else {
        format!("https://{}", base)
    };
    let url = format!("{}/sms/", base.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let resp_text = client
        .get(url)
        .query(&[
            ("api", "login"),
            ("user", username),
            ("pass", password),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("豪猪登录请求失败: {}", e))?
        .text()
        .await
        .map_err(|e| format!("豪猪登录响应读取失败: {}", e))?;

    extract_token_from_text(&resp_text).ok_or_else(|| {
        format!(
            "豪猪登录未解析到 token，响应: {}",
            resp_text.chars().take(300).collect::<String>()
        )
    })
}

async fn haozhuma_test_get_phone(
    api_domain: &str,
    token: &str,
    sid: &str,
    phone_filters: &serde_json::Value,
) -> Result<String, String> {
    let base = api_domain.trim();
    let base = if base.starts_with("http://") || base.starts_with("https://") {
        base.to_string()
    } else {
        format!("https://{}", base)
    };
    let url = format!("{}/sms/", base.trim_end_matches('/'));

    let mut params: Vec<(&str, String)> = vec![
        ("api", "getPhone".to_string()),
        ("token", token.to_string()),
        ("sid", sid.to_string()),
    ];

    let get_str = |k: &str| -> Option<String> {
        phone_filters
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    if let Some(v) = get_str("isp") {
        params.push(("isp", v));
    }
    if let Some(v) = get_str("province") {
        params.push(("Province", v));
    }
    if let Some(v) = get_str("ascription") {
        params.push(("ascription", v));
    }
    if let Some(v) = get_str("paragraph") {
        params.push(("paragraph", v));
    }
    if let Some(v) = get_str("exclude") {
        params.push(("exclude", v));
    }
    if let Some(v) = get_str("uid") {
        params.push(("uid", v));
    }
    if let Some(v) = get_str("author") {
        params.push(("author", v));
    }

    let client = reqwest::Client::new();
    let resp_text = client
        .get(url)
        .query(&params)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("豪猪取号请求失败: {}", e))?
        .text()
        .await
        .map_err(|e| format!("豪猪取号响应读取失败: {}", e))?;

    let digits = extract_digits(&resp_text);
    if digits.len() < 6 {
        return Err(format!(
            "豪猪取号未解析到号码，响应: {}",
            resp_text.chars().take(300).collect::<String>()
        ));
    }

    Ok(digits)
}

#[tauri::command]
async fn test_haozhuma_api(config: serde_json::Value) -> Result<String, String> {
    let api_domain = config.get("api_domain").and_then(|v| v.as_str()).unwrap_or("").trim();
    let username = config.get("username").and_then(|v| v.as_str()).unwrap_or("").trim();
    let password = config.get("password").and_then(|v| v.as_str()).unwrap_or("").trim();
    let project_id = config.get("project_id").and_then(|v| v.as_str()).unwrap_or("").trim();
    let fixed_phone = config.get("fixed_phone").and_then(|v| v.as_str()).unwrap_or("").trim();
    let phone_filters = config
        .get("phone_filters")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    if api_domain.is_empty() || username.is_empty() || password.is_empty() || project_id.is_empty() {
        return Err("豪猪配置不完整：需要 api_domain / username / password / project_id".to_string());
    }

    let token = haozhuma_test_login(api_domain, username, password).await?;
    let phone = if !fixed_phone.is_empty() {
        let digits = extract_digits(fixed_phone);
        if digits.len() < 6 {
            return Err(format!(
                "指定手机号无效（至少 6 位）：{}",
                fixed_phone
            ));
        }
        digits
    } else {
        haozhuma_test_get_phone(
            api_domain,
            &token,
            project_id,
            &phone_filters,
        )
        .await?
    };

    let last4 = if phone.len() >= 4 {
        Some(phone.chars().rev().take(4).collect::<String>().chars().rev().collect())
    } else {
        None
    };

    let result = HaozhumaTestResult {
        success: true,
        message: if fixed_phone.is_empty() {
            "豪猪 API 连接测试成功".to_string()
        } else {
            "豪猪 API 连接测试成功（使用指定手机号）".to_string()
        },
        token_len: Some(token.len()),
        phone_last4: last4,
    };

    serde_json::to_string(&result).map_err(|e| format!("序列化测试结果失败: {}", e))
}

// 获取应用版本
#[tauri::command]
async fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    let package_info = app.package_info();
    Ok(package_info.version.to_string())
}

// 打开更新链接
#[tauri::command]
async fn open_update_url(url: String) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", &url])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    Ok(())
}

// 手动触发复制 pyBuild 文件夹的命令

#[tauri::command]
async fn copy_pybuild_resources(app_handle: tauri::AppHandle) -> Result<String, String> {
    if cfg!(debug_assertions) {
        log_info!("Development mode: Manually copying pyBuild directory");
    }
    copy_pybuild_to_app_dir(&app_handle)?;
    let env_type = if cfg!(debug_assertions) {
        "development"
    } else {
        "production"
    };
    Ok(format!(
        "pyBuild directory copied successfully in {} mode",
        env_type
    ))
}

// 获取邮箱配置的辅助函数
async fn get_email_config() -> Result<EmailConfig, String> {
    match read_email_config().await {
        Ok(config_str) if !config_str.is_empty() => {
            match serde_json::from_str::<EmailConfig>(&config_str) {
                Ok(config) => {
                    // 验证配置是否完整
                    if config.worker_domain.is_empty()
                        || config.email_domain.is_empty()
                        || config.admin_password.is_empty()
                        || config.access_password.is_empty()
                    {
                        return Err("邮箱配置不完整，请先在前端配置邮箱域名和密码".to_string());
                    }
                    Ok(config)
                }
                Err(e) => Err(format!("解析邮箱配置失败: {}", e)),
            }
        }
        _ => Err("未找到邮箱配置，请先在前端配置邮箱域名和密码".to_string()),
    }
}

#[tauri::command]
async fn auto_login_and_get_cookie(
    app: tauri::AppHandle,
    email: String,
    password: String,
    show_window: Option<bool>,
) -> Result<serde_json::Value, String> {
    log_info!("🚀 开始自动登录获取Cookie: {}", email);

    // 检查是否已经有同名窗口，如果有则关闭
    if let Some(existing_window) = app.get_webview_window("auto_login") {
        log_info!("🔄 关闭现有的自动登录窗口");
        if let Err(e) = existing_window.close() {
            log_error!("❌ Failed to close existing auto login window: {}", e);
        } else {
            log_info!("✅ Existing auto login window closed successfully");
        }
        // 等待一小段时间确保窗口完全关闭
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 根据参数决定是否显示窗口
    let should_show_window = show_window.unwrap_or(false);
    log_info!(
        "🖥️ 窗口显示设置: {}",
        if should_show_window {
            "显示"
        } else {
            "隐藏"
        }
    );

    // 创建新的 WebView 窗口（根据配置显示/隐藏，启用无痕模式）
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app,
        "auto_login",
        tauri::WebviewUrl::External("https://authenticator.cursor.sh/".parse().unwrap()),
    )
    .title("Cursor - 自动登录")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .visible(should_show_window) // 根据参数决定是否显示
    .incognito(true) // 启用无痕模式
    .on_page_load(move |window, _payload| {
        let email_clone = email.clone();
        let password_clone = password.clone();

        // 创建自动登录脚本
        let login_script = format!(
            r#"
            (function() {{
                console.log('自动登录脚本已注入');
                
                function performLogin() {{
                    console.log('开始执行登录流程');
                    console.log('Current page URL:', window.location.href);
                    console.log('Page title:', document.title);
                    
                    // 检查是否已经登录成功（在dashboard页面）
                    if (window.location.href.includes('/dashboard')) {{
                        console.log('检测到已经在dashboard页面，直接获取cookie');
                        window.__TAURI_INTERNALS__.invoke('check_login_cookies');
                        return;
                    }}
                    
                    // 等待页面完全加载
                    if (document.readyState !== 'complete') {{
                        console.log('页面未完全加载，等待中...');
                        return;
                    }}
                    
                    // 步骤1: 填写邮箱
                    setTimeout(() => {{
                        console.log('步骤1: 填写邮箱');
                        const emailInput = document.querySelector('.rt-reset .rt-TextFieldInput');
                        if (emailInput) {{
                            emailInput.value = '{}';
                            console.log('邮箱已填写:', emailInput.value);
                            
                            // 触发input事件以确保值被正确设置
                            emailInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                            emailInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        }} else {{
                            console.error('未找到邮箱输入框');
                        }}
                    }}, 1000);
                    
                    // 步骤2: 点击第一个按钮（继续）
                    setTimeout(() => {{
                        console.log('步骤2: 点击继续按钮');
                        const firstButton = document.querySelector('.BrandedButton');
                        if (firstButton) {{
                            firstButton.click();
                            console.log('继续按钮已点击');
                        }} else {{
                            console.error('未找到继续按钮');
                        }}
                    }}, 2000);
                    
                    // 步骤3: 填写密码
                    setTimeout(() => {{
                        console.log('步骤3: 填写密码');
                        const passwordInput = document.querySelector('[name="password"]');
                        if (passwordInput) {{
                            passwordInput.value = '{}';
                            console.log('密码已填写');
                            
                            // 触发input事件以确保值被正确设置
                            passwordInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                            passwordInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        }} else {{
                            console.error('未找到密码输入框');
                        }}
                    }}, 6000);
                    
                    // 步骤4: 点击登录按钮
                    setTimeout(() => {{
                        console.log('步骤4: 点击登录按钮');
                        const loginButton = document.querySelector('.BrandedButton');
                        if (loginButton) {{
                            loginButton.click();
                            console.log('登录按钮已点击');
                            
                            // 等待登录完成后检查cookie
                            setTimeout(() => {{
                                console.log('检查登录状态和cookie');
                                checkLoginSuccess();
                            }}, 3000);
                        }} else {{
                            console.error('未找到登录按钮');
                        }}
                    }}, 9000);
                }}
                
                function checkLoginSuccess() {{
                    console.log('检查登录是否成功');
                    console.log('当前URL:', window.location.href);
                    
                    // 检查是否登录成功（通过URL变化或页面元素判断）
                    if (window.location.href.includes('/dashboard')) {{
                        console.log('登录成功，通知Rust获取cookie');
                        
                        // 通知Rust后端登录成功，让Rust获取httpOnly cookie
                        // window.__TAURI_INTERNALS__.invoke('check_login_cookies');
                    }} else {{
                        console.log('登录可能未完成，继续检查...');
                        // 再次检查
                        setTimeout(() => {{
                            checkLoginSuccess();
                        }}, 2000);
                    }}
                }}
                
                // 监听URL变化（用于检测重定向）
                let lastUrl = location.href;
                new MutationObserver(() => {{
                    const url = location.href;
                    if (url !== lastUrl) {{
                        lastUrl = url;
                        console.log('检测到URL变化:', url);
                        // 如果重定向到dashboard，直接获取cookie
                        if (url.includes('dashboard') || url.includes('app')) {{
                            console.log('重定向到dashboard，获取cookie');
                            setTimeout(() => {{
                                // window.__TAURI_INTERNALS__.invoke('check_login_cookies');
                            }}, 1000);
                        }}
                    }}
                }}).observe(document, {{ subtree: true, childList: true }});

                // 检查页面加载状态
                if (document.readyState === 'complete') {{
                    console.log('页面已经加载完成，开始登录流程');
                    setTimeout(() => {{
                        performLogin();
                    }}, 1000);
                }} else {{
                    // 监听页面加载完成事件
                    window.addEventListener('load', function() {{
                        console.log('window load 事件触发，开始登录流程');
                        setTimeout(() => {{
                            performLogin();
                        }}, 1000);
                    }});
                }}
            }})();
            "#,
            email_clone, password_clone
        );

        if let Err(e) = window.eval(&login_script) {
            log_error!("❌ Failed to inject login script: {}", e);
        } else {
            log_info!("✅ Login script injected successfully");
        }
    })
    .build();

    match webview_window {
        Ok(_window) => {
            let message = if should_show_window {
                "自动登录窗口已打开，正在执行登录流程..."
            } else {
                "正在后台执行自动登录流程..."
            };
            log_info!(
                "✅ Successfully created auto login WebView window ({})",
                if should_show_window {
                    "visible"
                } else {
                    "hidden"
                }
            );

            Ok(serde_json::json!({
                "success": true,
                "message": message
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to create auto login WebView window: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("无法打开自动登录窗口: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn verification_code_login(
    app: tauri::AppHandle,
    email: String,
    verification_code: String,
    show_window: Option<bool>,
) -> Result<serde_json::Value, String> {
    log_info!("🚀 开始验证码登录: {}", email);

    // 检查是否已经有同名窗口，如果有则关闭
    if let Some(existing_window) = app.get_webview_window("verification_code_login") {
        log_info!("🔄 关闭现有的验证码登录窗口");
        if let Err(e) = existing_window.close() {
            log_error!(
                "❌ Failed to close existing verification code login window: {}",
                e
            );
        } else {
            log_info!("✅ Existing verification code login window closed successfully");
        }
        // 等待一小段时间确保窗口完全关闭
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 根据参数决定是否显示窗口
    let should_show_window = show_window.unwrap_or(false);
    log_info!(
        "🖥️ 窗口显示设置: {}",
        if should_show_window {
            "显示"
        } else {
            "隐藏"
        }
    );

    // 创建新的 WebView 窗口（根据配置显示/隐藏，启用无痕模式）
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app,
        "verification_code_login",
        tauri::WebviewUrl::External("https://authenticator.cursor.sh/".parse().unwrap()),
    )
    .title("Cursor - 验证码登录")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .visible(should_show_window) // 根据参数决定是否显示
    .incognito(true) // 启用无痕模式
    .on_page_load(move |window, _payload| {
        let email_clone = email.clone();
        let code_clone = verification_code.clone();
        
        // 创建验证码登录脚本（先用自动登录的脚本，你后面修改）
        let login_script = format!(
            r#"
            (function() {{
                console.log('验证码登录脚本已注入');
                
                function performLogin() {{
                    console.log('开始执行验证码登录流程');
                    console.log('Current page URL:', window.location.href);
                    console.log('Page title:', document.title);
                    
                    // 检查是否已经登录成功（在dashboard页面）
                    if (window.location.href.includes('/dashboard')) {{
                        console.log('检测到已经在dashboard页面，直接获取cookie');
                        window.__TAURI_INTERNALS__.invoke('check_verification_login_cookies');
                        return;
                    }}
                    
                    // 等待页面完全加载
                    if (document.readyState !== 'complete') {{
                        console.log('页面未完全加载，等待中...');
                        return;
                    }}
                    
                    // TODO: 你需要修改这里的脚本来实现验证码登录
                    // 步骤1: 填写邮箱
                    setTimeout(() => {{
                        console.log('步骤1: 填写邮箱');
                        const emailInput = document.querySelector('.rt-reset .rt-TextFieldInput');
                        if (emailInput) {{
                            emailInput.value = '{}';
                            console.log('邮箱已填写:', emailInput.value);
                            
                            // 触发input事件以确保值被正确设置
                            emailInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                            emailInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        }} else {{
                            console.error('未找到邮箱输入框');
                        }}
                    }}, 1000);
                    
                    // 步骤2: 点击第一个按钮（继续）
                    setTimeout(() => {{
                        console.log('步骤2: 点击继续按钮');
                        const firstButton = document.querySelector('.BrandedButton');
                        if (firstButton) {{
                            firstButton.click();
                            console.log('继续按钮已点击');
                        }} else {{
                            console.error('未找到继续按钮');
                        }}
                    }}, 2000);
                            
                     // 点击验证码登录
                     setTimeout(() => {{
                        console.log('步骤2: 点击继续按钮');
                        const firstButton2 = document.querySelector('.rt-Button.ak-AuthButton');

                        if (firstButton2) {{
                            firstButton2.click();
                            console.log('继续按钮已点击');
                        }} else {{
                            console.error('未找到继续按钮');
                        }}
                    }}, 6000);
                    
                    // // 步骤3: 填写验证码（这里需要修改）
                    // setTimeout(() => {{
                    //     console.log('步骤3: 填写验证码');
                    //     // TODO: 修改为验证码输入框的选择器
                    //     const codeInput = document.querySelector('[name="verification_code"]');
                    //     if (codeInput) {{
                    //         codeInput.value = '{}';
                    //         console.log('验证码已填写');
                            
                    //         // 触发input事件以确保值被正确设置
                    //         codeInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    //         codeInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    //     }} else {{
                    //         console.error('未找到验证码输入框');
                    //     }}
                    // }}, 6000);
                    
                    // // 步骤4: 点击登录按钮
                    // setTimeout(() => {{
                    //     console.log('步骤4: 点击登录按钮');
                    //     const loginButton = document.querySelector('.BrandedButton');
                    //     if (loginButton) {{
                    //         loginButton.click();
                    //         console.log('登录按钮已点击');
                            
                    //         // 等待登录完成后检查cookie
                    //         setTimeout(() => {{
                    //             console.log('检查登录状态和cookie');
                    //             checkLoginSuccess();
                    //         }}, 3000);
                    //     }} else {{
                    //         console.error('未找到登录按钮');
                    //     }}
                    // }}, 9000);
                }}
                
                function checkLoginSuccess() {{
                    console.log('检查登录是否成功');
                    console.log('当前URL:', window.location.href);
                    
                    // 检查是否登录成功（通过URL变化或页面元素判断）
                    if (window.location.href.includes('/dashboard')) {{
                        console.log('登录成功，通知Rust获取cookie');
                        // 通知Rust后端登录成功，让Rust获取httpOnly cookie
                        // window.__TAURI_INTERNALS__.invoke('check_verification_login_cookies');
                    }} else {{
                        console.log('登录可能未完成，继续检查...');
                        // 再次检查
                        setTimeout(() => {{
                            checkLoginSuccess();
                        }}, 2000);
                    }}
                }}
                
                // 监听URL变化（用于检测重定向）
                let lastUrl = location.href;
                new MutationObserver(() => {{
                    const url = location.href;
                    if (url !== lastUrl) {{
                        lastUrl = url;
                        console.log('检测到URL变化:', url);
                        // 如果重定向到dashboard，直接获取cookie
                        if (url.includes('dashboard') || url.includes('app')) {{
                            console.log('重定向到dashboard，获取cookie');
                            setTimeout(() => {{
                                // window.__TAURI_INTERNALS__.invoke('check_verification_login_cookies');
                            }}, 1000);
                        }}
                    }}
                }}).observe(document, {{ subtree: true, childList: true }});

                // 检查页面加载状态
                if (document.readyState === 'complete') {{
                    console.log('页面已经加载完成，开始登录流程');
                    setTimeout(() => {{
                        performLogin();
                    }}, 1000);
                }} else {{
                    // 监听页面加载完成事件
                    window.addEventListener('load', function() {{
                        console.log('window load 事件触发，开始登录流程');
                        setTimeout(() => {{
                            performLogin();
                        }}, 1000);
                    }});
                }}
            }})();
            "#,
            email_clone, code_clone
        );

        if let Err(e) = window.eval(&login_script) {
            log_error!("❌ Failed to inject verification code login script: {}", e);
        } else {
            log_info!("✅ Verification code login script injected successfully");
        }
    })
    .build();

    match webview_window {
        Ok(_window) => {
            let message = if should_show_window {
                "验证码登录窗口已打开，正在执行登录流程..."
            } else {
                "正在后台执行验证码登录流程..."
            };
            log_info!(
                "✅ Successfully created verification code login WebView window ({})",
                if should_show_window {
                    "visible"
                } else {
                    "hidden"
                }
            );

            Ok(serde_json::json!({
                "success": true,
                "message": message
            }))
        }
        Err(e) => {
            log_error!(
                "❌ Failed to create verification code login WebView window: {}",
                e
            );
            Ok(serde_json::json!({
                "success": false,
                "message": format!("无法打开验证码登录窗口: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn check_verification_login_cookies(app: tauri::AppHandle) -> Result<(), String> {
    log_info!("🔍 开始检查验证码登录Cookie");

    if let Some(window) = app.get_webview_window("verification_code_login") {
        // 尝试多个可能的URL来获取cookie
        let urls_to_try = vec![
            "https://authenticator.cursor.sh/",
            "https://cursor.com/",
            "https://app.cursor.com/",
            "https://www.cursor.com/",
        ];

        for url_str in urls_to_try {
            log_info!("🔍 尝试从 {} 获取cookie", url_str);
            let url = url_str
                .parse()
                .map_err(|e| format!("Invalid URL {}: {}", url_str, e))?;

            match window.cookies_for_url(url) {
                Ok(cookies) => {
                    log_info!("📋 从 {} 找到 {} 个cookie", url_str, cookies.len());

                    // 查找 WorkosCursorSessionToken
                    for cookie in cookies {
                        log_info!(
                            "🍪 Cookie: {} = {}...",
                            cookie.name(),
                            &cookie.value()[..cookie.value().len().min(20)]
                        );

                        if cookie.name() == "WorkosCursorSessionToken" {
                            let token = cookie.value().to_string();
                            log_info!(
                                "✅ 找到 WorkosCursorSessionToken: {}...",
                                &token[..token.len().min(50)]
                            );

                            // 发送事件到前端
                            let _ = app.emit(
                                "verification-login-cookie-found",
                                serde_json::json!({
                                    "WorkosCursorSessionToken": token
                                }),
                            );

                            // 关闭窗口
                            if let Err(e) = window.close() {
                                log_error!("❌ 关闭验证码登录窗口失败: {}", e);
                            } else {
                                log_info!("✅ 验证码登录窗口已关闭");
                            }

                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ 从 {} 获取cookie失败: {}", url_str, e);
                }
            }
        }

        log_error!("❌ 未找到 WorkosCursorSessionToken");
        Err("未找到登录Token".to_string())
    } else {
        log_error!("❌ 未找到验证码登录窗口");
        Err("验证码登录窗口不存在".to_string())
    }
}

#[tauri::command]
async fn check_login_cookies(app: tauri::AppHandle) -> Result<(), String> {
    log_info!("🔍 开始检查登录Cookie");

    if let Some(window) = app.get_webview_window("auto_login") {
        // 尝试多个可能的URL来获取cookie
        let urls_to_try = vec![
            "https://authenticator.cursor.sh/",
            "https://cursor.com/",
            "https://app.cursor.com/",
            "https://www.cursor.com/",
        ];

        for url_str in urls_to_try {
            log_info!("🔍 尝试从 {} 获取cookie", url_str);
            let url = url_str
                .parse()
                .map_err(|e| format!("Invalid URL {}: {}", url_str, e))?;

            match window.cookies_for_url(url) {
                Ok(cookies) => {
                    log_info!("📋 从 {} 找到 {} 个cookie", url_str, cookies.len());

                    // 查找 WorkosCursorSessionToken
                    for cookie in cookies {
                        log_info!(
                            "🍪 Cookie: {} = {}...",
                            cookie.name(),
                            &cookie.value()[..cookie.value().len().min(20)]
                        );

                        if cookie.name() == "WorkosCursorSessionToken" {
                            let token = cookie.value().to_string();
                            log_info!(
                                "🎉 在 {} 找到 WorkosCursorSessionToken: {}...",
                                url_str,
                                &token[..token.len().min(20)]
                            );

                            // 关闭自动登录窗口
                            if let Err(e) = window.close() {
                                log_error!("❌ Failed to close auto login window: {}", e);
                            } else {
                                log_info!("✅ Auto login window closed successfully");
                            }

                            // 发送事件通知前端获取到了token
                            if let Err(e) = app.emit(
                                "auto-login-success",
                                serde_json::json!({
                                    "token": token
                                }),
                            ) {
                                log_error!("❌ Failed to emit auto login success event: {}", e);
                            } else {
                                log_info!("✅ Auto login success event emitted");
                            }

                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ 从 {} 获取cookie失败: {}", url_str, e);
                }
            }
        }

        // 如果所有URL都没找到目标cookie
        log_info!("⏳ 在所有URL中都未找到 WorkosCursorSessionToken");
        if let Err(e) = app.emit(
            "auto-login-failed",
            serde_json::json!({
                "error": "未找到 WorkosCursorSessionToken cookie"
            }),
        ) {
            log_error!("❌ Failed to emit auto login failed event: {}", e);
        }
    } else {
        log_error!("❌ 未找到自动登录窗口");
        if let Err(e) = app.emit(
            "auto-login-failed",
            serde_json::json!({
                "error": "未找到自动登录窗口"
            }),
        ) {
            log_error!("❌ Failed to emit auto login failed event: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
async fn auto_login_success(app: tauri::AppHandle, token: String) -> Result<(), String> {
    log_info!(
        "🎉 自动登录成功，获取到Token: {}...",
        &token[..token.len().min(20)]
    );

    // 关闭自动登录窗口
    if let Some(window) = app.get_webview_window("auto_login") {
        if let Err(e) = window.close() {
            log_error!("❌ Failed to close auto login window: {}", e);
        } else {
            log_info!("✅ Auto login window closed successfully");
        }
    }

    // 发送事件通知前端获取到了token
    if let Err(e) = app.emit(
        "auto-login-success",
        serde_json::json!({
            "token": token
        }),
    ) {
        log_error!("❌ Failed to emit auto login success event: {}", e);
    } else {
        log_info!("✅ Auto login success event emitted");
    }

    Ok(())
}

#[tauri::command]
async fn auto_login_failed(app: tauri::AppHandle, error: String) -> Result<(), String> {
    log_error!("❌ 自动登录失败: {}", error);

    // 关闭自动登录窗口
    if let Some(window) = app.get_webview_window("auto_login") {
        if let Err(e) = window.close() {
            log_error!("❌ Failed to close auto login window: {}", e);
        }
    }

    // 发送事件通知前端登录失败
    if let Err(e) = app.emit(
        "auto-login-failed",
        serde_json::json!({
            "error": error
        }),
    ) {
        log_error!("❌ Failed to emit auto login failed event: {}", e);
    }

    Ok(())
}

#[tauri::command]
async fn open_cursor_dashboard(
    app: tauri::AppHandle,
    workos_cursor_session_token: String,
) -> Result<serde_json::Value, String> {
    log_info!("🔄 Opening Cursor dashboard with WorkOS token...");

    let url = "https://cursor.com/dashboard";

    // 先尝试关闭已存在的窗口
    if let Some(existing_window) = app.get_webview_window("cursor_dashboard") {
        log_info!("🔄 Closing existing cursor dashboard window...");
        if let Err(e) = existing_window.close() {
            log_error!("❌ Failed to close existing window: {}", e);
        } else {
            log_info!("✅ Existing window closed successfully");
        }
        // 等待一小段时间确保窗口完全关闭
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 创建新的 WebView 窗口
    let webview_window = tauri::WebviewWindowBuilder::new(
        &app,
        "cursor_dashboard",
        tauri::WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Cursor - 主页")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .initialization_script(&format!(
        r#"
        // 在页面加载前设置 Cookie
        document.cookie = 'WorkosCursorSessionToken={}; domain=.cursor.com; path=/; secure; samesite=none';
        console.log('Cookie injected for dashboard view');
        console.log('Current cookies:', document.cookie);
        "#,
        workos_cursor_session_token
    ))
    .visible(true)
    .build();

    match webview_window {
        Ok(window) => {
            // 添加窗口关闭事件监听器
            window.on_window_event(move |event| match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    log_info!("🔄 Cursor dashboard window close requested by user");
                }
                tauri::WindowEvent::Destroyed => {
                    log_info!("🔄 Cursor dashboard window destroyed");
                }
                _ => {}
            });

            log_info!("✅ Successfully opened Cursor dashboard window");
            Ok(serde_json::json!({
                "success": true,
                "message": "已打开Cursor主页"
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to create Cursor dashboard window: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "message": format!("无法打开Cursor主页: {}", e)
            }))
        }
    }
}

#[tauri::command]
async fn show_auto_login_window(app: tauri::AppHandle) -> Result<(), String> {
    log_info!("🔍 Attempting to show auto login window");

    if let Some(window) = app.get_webview_window("auto_login") {
        window
            .show()
            .map_err(|e| format!("Failed to show auto login window: {}", e))?;
        log_info!("✅ Auto login window shown successfully");
    } else {
        log_error!("❌ Auto login window not found");
        return Err("Auto login window not found".to_string());
    }

    Ok(())
}

// ==================== Web服务器管理 ====================

#[tauri::command]
async fn get_web_server_port() -> Result<u16, String> {
    // 从配置文件读取端口，默认34567
    let app_dir = get_app_dir()?;
    let config_path = app_dir.join("web_server_config.json");

    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(port) = config.get("port").and_then(|p| p.as_u64()) {
                        return Ok(port as u16);
                    }
                }
            }
            Err(e) => {
                log_error!("读取Web服务器配置失败: {}", e);
            }
        }
    }

    Ok(34567) // 默认端口
}

#[tauri::command]
async fn set_web_server_port(port: u16) -> Result<String, String> {
    let app_dir = get_app_dir()?;
    let config_path = app_dir.join("web_server_config.json");

    let config = serde_json::json!({
        "port": port
    });

    match std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()) {
        Ok(_) => {
            log_info!("Web服务器端口已设置为: {}", port);
            Ok(format!("端口已设置为: {}, 需要重启应用生效", port))
        }
        Err(e) => {
            log_error!("保存Web服务器配置失败: {}", e);
            Err(format!("保存配置失败: {}", e))
        }
    }
}

#[tauri::command]
async fn get_seamless_switch_web_config() -> Result<serde_json::Value, String> {
    match NextWorkWebServer::read_seamless_switch_config().await {
        Ok(config) => Ok(serde_json::to_value(config).unwrap()),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn start_web_server() -> Result<serde_json::Value, String> {
    log_info!("🌐 [CMD] 收到启动Web服务器命令");

    // 获取配置的端口
    let port = match get_web_server_port().await {
        Ok(port) => port,
        Err(e) => {
            log_error!("获取Web服务器端口失败: {}, 使用默认端口34567", e);
            34567
        }
    };

    log_info!("🌐 准备启动Web服务器，端口: {}", port);

    // 使用tokio::spawn在后台启动Web服务器
    tokio::spawn(async move {
        let web_server = NextWorkWebServer::new(port);
        if let Err(e) = web_server.start().await {
            log_error!("❌ Web服务器启动失败: {}", e);
        } else {
            log_info!("✅ Web服务器启动成功");
        }
    });

    Ok(serde_json::json!({
        "success": true,
        "message": format!("Web服务器正在启动，端口: {}", port),
        "port": port
    }))
}

// ==================== 无感换号功能 ====================

#[tauri::command]
async fn check_seamless_switch_status() -> Result<serde_json::Value, String> {
    match MachineIdRestorer::check_seamless_switch_status() {
        Ok(is_enabled) => Ok(serde_json::json!({
            "success": true,
            "enabled": is_enabled,
            "message": if is_enabled { "无感换号功能已启用" } else { "无感换号功能未启用" }
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "enabled": false,
            "message": format!("检查无感换号状态失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn get_seamless_switch_full_status() -> Result<serde_json::Value, String> {
    match MachineIdRestorer::get_seamless_switch_full_status() {
        Ok(status) => Ok(serde_json::json!({
            "success": true,
            "workbench_modified": status.workbench_modified,
            "extension_host_modified": status.extension_host_modified,
            "fully_enabled": status.fully_enabled,
            "need_reset_warning": status.need_reset_warning,
            "message": if status.fully_enabled {
                "无感换号+无感重置ID功能已完全启用"
            } else if status.workbench_modified {
                "无感换号已启用，但无感重置ID未启用"
            } else {
                "无感换号功能未启用"
            }
        })),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "workbench_modified": false,
            "extension_host_modified": false,
            "fully_enabled": false,
            "need_reset_warning": false,
            "message": format!("检查无感换号状态失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn enable_seamless_switch() -> Result<serde_json::Value, String> {
    match MachineIdRestorer::enable_seamless_switch() {
        Ok(message) => {
            // 启用成功后，自动重启Cursor（如果正在运行）
            log_info!("🔄 [DEBUG] 无感换号启用成功，准备重启Cursor...");

            // 使用现有的switch_account逻辑来重启Cursor
            match AccountManager::force_kill_cursor_processes() {
                Ok(killed_count) => {
                    log_info!("✅ [DEBUG] 强制结束了 {} 个Cursor进程", killed_count);
                }
                Err(e) => {
                    log_warn!("⚠️ [DEBUG] 强制结束Cursor进程时出错: {}", e);
                }
            }

            // 等待进程结束
            std::thread::sleep(std::time::Duration::from_millis(2000));

            // 重启Cursor
            match AccountManager::start_cursor() {
                Ok(()) => {
                    log_info!("✅ [DEBUG] Cursor重启成功");
                }
                Err(e) => {
                    log_warn!("⚠️ [DEBUG] Cursor重启失败: {}", e);
                }
            }

            // 初始化Web服务器配置文件为空状态（刚启用时没有token）
            if let Err(e) = NextWorkWebServer::write_seamless_switch_config(
                &next_work_web::SeamlessSwitchStatus::default(),
            )
            .await
            {
                log_warn!("⚠️ [DEBUG] 初始化Web配置失败: {}", e);
            }

            Ok(serde_json::json!({
                "success": true,
                "message": format!("{}，Cursor已重启", message)
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("启用无感换号失败: {}", e)
        })),
    }
}

#[tauri::command]
async fn disable_seamless_switch() -> Result<serde_json::Value, String> {
    match MachineIdRestorer::disable_seamless_switch() {
        Ok(message) => {
            // 禁用成功后，自动重启Cursor（如果正在运行）
            log_info!("🔄 [DEBUG] 无感换号禁用成功，准备重启Cursor...");

            // 使用现有的switch_account逻辑来重启Cursor
            match AccountManager::force_kill_cursor_processes() {
                Ok(killed_count) => {
                    log_info!("✅ [DEBUG] 强制结束了 {} 个Cursor进程", killed_count);
                }
                Err(e) => {
                    log_warn!("⚠️ [DEBUG] 强制结束Cursor进程时出错: {}", e);
                }
            }

            // 等待进程结束
            std::thread::sleep(std::time::Duration::from_millis(2000));

            // 重启Cursor
            match AccountManager::start_cursor() {
                Ok(()) => {
                    log_info!("✅ [DEBUG] Cursor重启成功");
                }
                Err(e) => {
                    log_warn!("⚠️ [DEBUG] Cursor重启失败: {}", e);
                }
            }

            Ok(serde_json::json!({
                "success": true,
                "message": format!("{}，Cursor已重启", message)
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "message": format!("禁用无感换号失败: {}", e)
        })),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当用户尝试启动第二个实例时，显示并聚焦现有窗口
            log_info!("🔔 [SINGLE_INSTANCE] 检测到第二个实例启动请求，显示并聚焦现有窗口");

            if let Some(window) = app.get_webview_window("main") {
                // 显示窗口（如果被隐藏）
                if let Err(e) = window.show() {
                    log_error!("❌ [SINGLE_INSTANCE] 显示窗口失败: {}", e);
                }

                // 取消最小化（如果被最小化）
                if let Err(e) = window.unminimize() {
                    log_error!("❌ [SINGLE_INSTANCE] 取消最小化失败: {}", e);
                }

                // 聚焦窗口
                if let Err(e) = window.set_focus() {
                    log_error!("❌ [SINGLE_INSTANCE] 聚焦窗口失败: {}", e);
                } else {
                    log_info!("✅ [SINGLE_INSTANCE] 窗口已显示并聚焦");
                }

                // macOS 特殊处理：激活应用程序
                #[cfg(target_os = "macos")]
                {
                    use cocoa::appkit::NSApplication;
                    use cocoa::base::nil;
                    unsafe {
                        let app = cocoa::appkit::NSApp();
                        app.activateIgnoringOtherApps_(cocoa::base::YES);
                    }
                    log_info!("✅ [SINGLE_INSTANCE] macOS 应用已激活");
                }
            } else {
                log_error!("❌ [SINGLE_INSTANCE] 未找到主窗口");
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // 只拦截主窗口的关闭请求，其他窗口允许正常关闭
                    if window.label() == "main" {
                        // 阻止主窗口关闭，改为隐藏
                        window.hide().unwrap();
                        api.prevent_close();
                        log_info!("🫥 [TRAY] 主窗口关闭请求被拦截，窗口已隐藏到托盘");
                    } else {
                        // 其他窗口（如验证码登录窗口）允许正常关闭
                        log_info!("✅ [WINDOW] 窗口 '{}' 正常关闭", window.label());
                    }
                }
                tauri::WindowEvent::Resized(_) => {
                    // 窗口大小改变事件
                }
                _ => {}
            }
        })
        .setup(|app| {
            // 初始化全局应用句柄
            if let Err(_) = APP_HANDLE.set(app.handle().clone()) {
                log_error!("Failed to set global app handle");
            }

            // 初始化日志系统
            if let Err(e) = logger::Logger::init() {
                eprintln!("Failed to initialize logger: {}", e);
            } else {
                log_info!("Application starting up...");
            }

            // 只在生产环境下复制 pyBuild 文件夹（macOS 和 Linux），开发模式下跳过
            if !cfg!(debug_assertions) && (cfg!(target_os = "macos") || cfg!(target_os = "linux")) {
                if let Err(e) = copy_pybuild_to_app_dir(app.handle()) {
                    log_error!("Failed to copy pyBuild directory on startup: {}", e);
                    // 不阻断应用启动，只记录错误
                }
            } else {
                if cfg!(debug_assertions) {
                    log_info!("Development mode detected, skipping pyBuild directory copy");
                } else {
                    log_info!("Windows platform detected, skipping pyBuild directory copy");
                }
            }

            // 创建系统托盘
            if let Err(e) = tray::create_tray(app.handle()) {
                log_error!("❌ [TRAY] 系统托盘创建失败: {}", e);
                return Err(Box::new(e));
            }

            // 应用启动完成后启动Web服务器
            let _app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // 延迟一下确保应用完全启动
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                log_info!("🌐 [STARTUP] 应用启动完成，开始启动Web服务器...");

                // 获取配置的端口
                let port = match get_web_server_port().await {
                    Ok(port) => port,
                    Err(e) => {
                        log_error!("获取Web服务器端口失败: {}, 使用默认端口34567", e);
                        34567
                    }
                };

                log_info!("🌐 准备启动Web服务器，端口: {}", port);

                let web_server = NextWorkWebServer::new(port);
                if let Err(e) = web_server.start().await {
                    log_error!("❌ Web服务器启动失败: {}", e);
                } else {
                    log_info!("✅ Web服务器启动成功，监听端口: {}", port);

                    // 启动后刷新符合条件的账户缓存
                    log_info!("🔄 [STARTUP] 开始刷新账户缓存...");
                    match NextWorkWebServer::refresh_eligible_accounts_cache().await {
                        Ok(count) => {
                            log_info!("✅ [STARTUP] 账户缓存刷新成功: {} 个符合条件的账户", count);
                        }
                        Err(e) => {
                            log_warn!("⚠️ [STARTUP] 账户缓存刷新失败: {}", e);
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_available_backups,
            extract_backup_ids,
            delete_backup,
            flash_window,
            restore_machine_ids,
            get_cursor_paths,
            check_cursor_installation,
            get_cursor_version,
            reset_machine_ids,
            complete_cursor_reset,
            get_log_file_path,
            get_log_config,
            write_weblog,
            get_weblog_file_path,
            get_weblog_config,
            get_recent_weblogs,
            test_logging,
            debug_windows_cursor_paths,
            set_custom_cursor_path,
            get_custom_cursor_path,
            clear_custom_cursor_path,
            select_browser_file,
            set_custom_browser_path,
            get_custom_browser_path,
            clear_custom_browser_path,
            open_log_file,
            open_log_directory,
            get_current_machine_ids,
            get_machine_id_file_content,
            get_backup_directory_info,
            check_user_authorization,
            get_user_info,
            get_subscription_info_only,
            get_current_period_usage,
            get_token_auto,
            list_codex_token_files,
            debug_cursor_paths,
            get_account_list,
            add_account,
            edit_account,
            update_account_custom_tags,
            switch_account,
            switch_account_with_token,
            remove_account,
            logout_current_account,
            export_accounts,
            import_accounts,
            open_cancel_subscription_page,
            show_cancel_subscription_window,
            cancel_subscription_failed,
            open_manual_bind_card_page,
            get_bind_card_url,
            get_bind_card_url_for_python,
            acknowledge_grace_period,
            show_manual_bind_card_window,
            manual_bind_card_failed,
            delete_cursor_account,
            trigger_authorization_login,
            trigger_authorization_login_poll,
            get_usage_for_period,
            get_user_analytics,
            get_usage_events,
            refresh_eligible_accounts_cache,
            register_cursor_account,
            create_temp_email,
            register_with_email,
            batch_register_with_email,
            batch_register_with_email_parallel,
            batch_register_codex_with_email,
            batch_register_codex_with_email_parallel,
            register_with_cloudflare_temp_email,
            register_with_tempmail,
            register_with_outlook,
            register_with_self_hosted_mail_api,
            register_codex_with_self_hosted_mail_api,
            submit_verification_code,
            signal_registration_continue,
            cancel_registration,
            cancel_registration_task,
            get_saved_accounts,
            check_and_convert_bank_card_config,
            read_bank_card_config,
            save_bank_card_config,
            read_email_config,
            save_email_config,
            read_haozhuma_config,
            save_haozhuma_config,
            test_haozhuma_api,
            get_app_version,
            open_update_url,
            copy_pybuild_resources,
            auto_login_and_get_cookie,
            check_login_cookies,
            auto_login_success,
            auto_login_failed,
            show_auto_login_window,
            open_cursor_dashboard,
            verification_code_login,
            check_verification_login_cookies,
            check_seamless_switch_status,
            get_seamless_switch_full_status,
            enable_seamless_switch,
            disable_seamless_switch,
            get_web_server_port,
            set_web_server_port,
            get_seamless_switch_web_config,
            start_web_server,
            get_cursor_backup_info,
            backup_cursor_data,
            restore_cursor_data,
            get_backup_list,
            cancel_backup,
            delete_cursor_backup,
            open_cursor_settings_dir,
            open_cursor_workspace_dir,
            open_backup_dir,
            open_directory_by_path,
            get_workspace_storage_items,
            get_workspace_details,
            debug_workspace_sqlite,
            get_conversation_detail,
            delete_backup,
            start_vless_proxy,
            stop_vless_proxy,
            cancel_vless_xray_download,
            other::read_clipboard,
            other::write_clipboard,
            other::clear_tempmail_inbox
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
