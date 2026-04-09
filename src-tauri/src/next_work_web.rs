use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tokio::sync::RwLock;
use tokio::time;
use warp::Filter;
use rand::Rng;
use sha2::{Digest, Sha256, Sha512};
use uuid::Uuid;

use crate::{get_app_dir, log_error, log_info, log_warn};

// 无感换号状态结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamlessSwitchStatus {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "isSwitch")]
    pub is_switch: i32,
    #[serde(rename = "machineIds", skip_serializing_if = "Option::is_none")]
    pub machine_ids: Option<MachineIds>,
    #[serde(rename = "email", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

// 机器ID结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineIds {
    #[serde(rename = "devDeviceId")]
    pub dev_device_id: String,
    #[serde(rename = "macMachineId")]
    pub mac_machine_id: String,
    #[serde(rename = "machineId")]
    pub machine_id: String,
    #[serde(rename = "sqmId")]
    pub sqm_id: String,
    #[serde(rename = "serviceMachineId")]
    pub service_machine_id: String,
}

impl Default for SeamlessSwitchStatus {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            is_switch: 0,
            machine_ids: None,
            email: None,
        }
    }
}

// 自动轮换配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    #[serde(rename = "autoSwitchEnabled")]
    pub auto_switch_enabled: bool,
    #[serde(rename = "costThreshold")]
    pub cost_threshold: f64, // 单位：美元
    #[serde(rename = "manualConfigEnabled", default)]
    pub manual_config_enabled: bool, // 开启手动配置轮换账户
}

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            auto_switch_enabled: false,
            cost_threshold: 10.0,
            manual_config_enabled: false,
        }
    }
}

// 用于防抖的错误记录
static LAST_ERROR_TIME: once_cell::sync::Lazy<Arc<RwLock<u64>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(0)));

// 缓存符合条件的账户（试用版和Pro及以上）
static ELIGIBLE_ACCOUNTS_CACHE: once_cell::sync::Lazy<Arc<RwLock<Vec<(String, String)>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(Vec::new())));

// Web服务器结构
pub struct NextWorkWebServer {
    port: u16,
}

impl NextWorkWebServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    // 获取配置文件路径
    fn get_config_path() -> Result<PathBuf, String> {
        let app_dir = get_app_dir()?;
        Ok(app_dir.join("seamless_switch_config.json"))
    }

    // 获取自动轮换配置文件路径
    fn get_auto_switch_config_path() -> Result<PathBuf, String> {
        let app_dir = get_app_dir()?;
        Ok(app_dir.join("auto_switch_config.json"))
    }

    // 读取自动轮换配置
    pub async fn read_auto_switch_config() -> Result<AutoSwitchConfig, String> {
        let config_path = Self::get_auto_switch_config_path()?;

        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => {
                    if content.trim().is_empty() {
                        return Ok(AutoSwitchConfig::default());
                    }

                    match serde_json::from_str::<AutoSwitchConfig>(&content) {
                        Ok(config) => {
                            log_info!(
                                "📖 [AUTO_SWITCH] 读取自动轮换配置成功: enabled={}, threshold=${}",
                                config.auto_switch_enabled,
                                config.cost_threshold
                            );
                            Ok(config)
                        }
                        Err(e) => {
                            log_error!("❌ [AUTO_SWITCH] 解析自动轮换配置失败: {}", e);
                            Ok(AutoSwitchConfig::default())
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ [AUTO_SWITCH] 读取自动轮换配置文件失败: {}", e);
                    Ok(AutoSwitchConfig::default())
                }
            }
        } else {
            log_info!("ℹ️ [AUTO_SWITCH] 自动轮换配置文件不存在，返回默认配置");
            Ok(AutoSwitchConfig::default())
        }
    }

    // 写入自动轮换配置
    pub async fn write_auto_switch_config(config: &AutoSwitchConfig) -> Result<(), String> {
        let config_path = Self::get_auto_switch_config_path()?;

        match serde_json::to_string_pretty(config) {
            Ok(json_content) => match fs::write(&config_path, json_content) {
                Ok(_) => {
                    log_info!(
                        "💾 [AUTO_SWITCH] 自动轮换配置保存成功: enabled={}, threshold=${}",
                        config.auto_switch_enabled,
                        config.cost_threshold
                    );
                    Ok(())
                }
                Err(e) => {
                    log_error!("❌ [AUTO_SWITCH] 保存自动轮换配置文件失败: {}", e);
                    Err(format!("保存配置文件失败: {}", e))
                }
            },
            Err(e) => {
                log_error!("❌ [AUTO_SWITCH] 序列化自动轮换配置失败: {}", e);
                Err(format!("序列化配置失败: {}", e))
            }
        }
    }

    // 刷新符合条件的账户缓存
    pub async fn refresh_eligible_accounts_cache() -> Result<usize, String> {
        use crate::account_manager::AccountManager;
        use crate::auth_checker::AuthChecker;

        log_info!("🔄 [CACHE] 开始刷新符合条件的账户缓存");

        let account_list_result = AccountManager::get_account_list();
        if !account_list_result.success {
            return Err(format!("获取账户列表失败: {}", account_list_result.message));
        }

        let accounts = account_list_result.accounts;
        log_info!("📋 [CACHE] 获取到 {} 个账户", accounts.len());

        // 并发获取所有账户的状态信息
        let mut info_tasks: Vec<tokio::task::JoinHandle<Result<Option<(String, String)>, String>>> =
            Vec::new();

        for account in accounts {
            let email = account.email.clone();
            let token = account.token.clone();

            let task = tokio::spawn(async move {
                match AuthChecker::get_user_info(&token).await {
                    Ok(result) if result.success => {
                        if let Some(user_info) = result.user_info {
                            if let Some(account_info) = user_info.account_info {
                                if let Some(sub_type) = account_info.subscription_type {
                                    let sub_type_lower = sub_type.to_lowercase();

                                    if sub_type_lower.contains("trial")
                                        || sub_type_lower.contains("pro")
                                        || sub_type_lower.contains("business")
                                        || sub_type_lower.contains("ultra")
                                    {
                                        return Ok(Some((email, token)));
                                    }
                                }

                            }

                        }

                        Ok(None)
                    }
                    _ => Ok(None),
                }
            });

            info_tasks.push(task);
        }

        let info_results = futures::future::join_all(info_tasks).await;

        let mut eligible_accounts = Vec::new();
        for result in info_results {
            if let Ok(Ok(Some((email, token)))) = result {
                eligible_accounts.push((email, token));
            }
        }

        let count = eligible_accounts.len();

        // 更新缓存
        let mut cache = ELIGIBLE_ACCOUNTS_CACHE.write().await;
        *cache = eligible_accounts;

        log_info!("✅ [CACHE] 缓存刷新完成: {} 个符合条件的账户", count);

        Ok(count)
    }

    // 读取无感换号配置
    pub async fn read_seamless_switch_config() -> Result<SeamlessSwitchStatus, String> {
        let config_path = Self::get_config_path()?;

        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => {
                    if content.trim().is_empty() {
                        return Ok(SeamlessSwitchStatus::default());
                    }

                    match serde_json::from_str::<SeamlessSwitchStatus>(&content) {
                        Ok(config) => {
                            // log_info!(
                            //     "📖 [WEB] 读取无感换号配置成功: token={}..., isSwitch={}",
                            //     &config.access_token[..config.access_token.len().min(20)],
                            //     config.is_switch
                            // );
                            Ok(config)
                        }
                        Err(e) => {
                            log_error!("❌ [WEB] 解析无感换号配置失败: {}", e);
                            Ok(SeamlessSwitchStatus::default())
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ [WEB] 读取无感换号配置文件失败: {}", e);
                    Ok(SeamlessSwitchStatus::default())
                }
            }
        } else {
            log_info!("ℹ️ [WEB] 无感换号配置文件不存在，返回默认配置");
            Ok(SeamlessSwitchStatus::default())
        }
    }

    // 写入无感换号配置
    pub async fn write_seamless_switch_config(config: &SeamlessSwitchStatus) -> Result<(), String> {
        let config_path = Self::get_config_path()?;

        match serde_json::to_string_pretty(config) {
            Ok(json_content) => match fs::write(&config_path, json_content) {
                Ok(_) => {
                    log_info!(
                        "💾 [WEB] 无感换号配置保存成功: token={}..., isSwitch={}, machineIds={}",
                        &config.access_token[..config.access_token.len().min(20)],
                        config.is_switch,
                        if config.machine_ids.is_some() { "已设置" } else { "未设置" }
                    );
                    Ok(())
                }
                Err(e) => {
                    log_error!("❌ [WEB] 保存无感换号配置文件失败: {}", e);
                    Err(format!("保存配置文件失败: {}", e))
                }
            },
            Err(e) => {
                log_error!("❌ [WEB] 序列化无感换号配置失败: {}", e);
                Err(format!("序列化配置失败: {}", e))
            }
        }
    }

    // 生成新的机器ID
    fn generate_new_machine_ids() -> MachineIds {
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

        MachineIds {
            dev_device_id: dev_device_id.clone(),
            mac_machine_id,
            machine_id,
            sqm_id,
            service_machine_id: dev_device_id, // Same as dev_device_id
        }
    }

    // 更新无感换号token并设置切换状态
    pub async fn update_seamless_switch_token(access_token: String, email: String, is_auto_switch: bool, reset_machine_id: bool) -> Result<(), String> {
        log_info!(
            "🔄 [WEB] 开始更新无感换号token，长度: {}, email: {}, reset_machine_id: {}",
            access_token.len(),
            email,
            reset_machine_id
        );

        let mut config = Self::read_seamless_switch_config().await?;
        log_info!("🔄 [WEB] 读取到当前配置: isSwitch={}", config.is_switch);

        // 根据参数决定是否生成新的机器ID
        if reset_machine_id {
            let new_machine_ids = Self::generate_new_machine_ids();
            log_info!(
                "🆔 [WEB] 生成新的机器ID: devDeviceId={}, machineId长度={}, macMachineId长度={}, sqmId={}",
                new_machine_ids.dev_device_id,
                new_machine_ids.machine_id.len(),
                new_machine_ids.mac_machine_id.len(),
                new_machine_ids.sqm_id
            );
            config.machine_ids = Some(new_machine_ids); // 设置新的机器ID
        } else {
            log_info!("⏭️ [WEB] 跳过生成新机器ID，保持原有机器ID不变");
            // 不修改 config.machine_ids，保持原有值
        }

        config.access_token = access_token.clone();
        config.is_switch = 1; // 标记为需要切换
        config.email = Some(email.clone()); // 设置邮箱

        let machine_ids_status = if reset_machine_id { "机器ID已生成" } else { "保持原有机器ID" };
        log_info!(
            "🔄 [WEB] 准备写入新配置: isSwitch=1, token={}..., email={}, {}",
            &access_token[..access_token.len().min(20)],
            email,
            machine_ids_status
        );

        Self::write_seamless_switch_config(&config).await?;

        log_info!("✅ [WEB] 无感换号配置写入成功，isSwitch已设置为1，{}，邮箱已设置", machine_ids_status);

        // 启动异步任务，10秒后将is_switch重置为0
        tokio::spawn(async move {
            time::sleep(Duration::from_secs(10)).await;

            match Self::read_seamless_switch_config().await {
                Ok(mut current_config) => {
                    if current_config.is_switch == 1 {
                        current_config.is_switch = 0;
                        if let Err(e) = Self::write_seamless_switch_config(&current_config).await {
                            log_error!("❌ [WEB] 自动重置切换状态失败: {}", e);
                        } else {
                            log_info!("✅ [WEB] 10秒后自动重置：切换状态已重置为0");
                            
                            // 只在自动轮换时才发射事件通知前端
                            if is_auto_switch {
                                if let Some(app_handle) = crate::APP_HANDLE.get() {
                                    let payload = serde_json::json!({
                                        "success": true,
                                        "message": "自动轮换账户成功",
                                        "timestamp": chrono::Utc::now().timestamp()
                                    });
                                    
                                    if let Err(e) = app_handle.emit("auto-switch-success", &payload) {
                                        log_error!("❌ [WEB] 发射自动轮换成功事件失败: {}", e);
                                    } else {
                                        log_info!("✅ [WEB] 已通知前端自动轮换成功");
                                    }
                                }
                            } else {
                                log_info!("ℹ️ [WEB] 手动切换完成，不发射自动轮换事件");
                            }
                        }
                    }
                }
                Err(e) => {
                    log_error!("❌ [WEB] 读取配置用于重置状态失败: {}", e);
                }
            }
        });

        Ok(())
    }

    // 启动Web服务器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log_info!("🌐 [WEB] 启动无感换号Web服务器，端口: {}", self.port);

        // CORS配置
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "authorization", "accept"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

        // 获取无感换号配置的路由
        let get_config = warp::path!("api" / "seamless-switch" / "config")
            .and(warp::get())
            .and_then(handle_get_seamless_switch_config)
            .with(&cors);

        // 更新无感换号token的路由
        let update_token = warp::path!("api" / "seamless-switch" / "token")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(handle_update_seamless_switch_token)
            .with(&cors);

        // 错误上报路由
        let error_report = warp::path!("api" / "error-report")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(handle_error_report)
            .with(&cors);

        // 获取自动轮换配置的路由
        let get_auto_switch_config = warp::path!("api" / "auto-switch" / "config")
            .and(warp::get())
            .and_then(handle_get_auto_switch_config)
            .with(&cors);

        // 更新自动轮换配置的路由
        let update_auto_switch_config = warp::path!("api" / "auto-switch" / "config")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(handle_update_auto_switch_config)
            .with(&cors);

        // 批量设置账户自动轮换标识的路由
        let batch_set_auto_switch = warp::path!("api" / "accounts" / "batch-auto-switch")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(handle_batch_set_auto_switch)
            .with(&cors);

        // 健康检查路由
        let health = warp::path!("health")
            .and(warp::get())
            .map(|| {
                log_info!("🏥 [WEB] 健康检查请求");
                warp::reply::json(&serde_json::json!({"status": "ok"}))
            })
            .with(&cors);

        let routes = get_config
            .or(update_token)
            .or(error_report)
            .or(get_auto_switch_config)
            .or(update_auto_switch_config)
            .or(batch_set_auto_switch)
            .or(health)
            .with(warp::log("next_work_web"));

        // log_info!("🚀 [WEB] Web服务器启动成功，监听端口: {}", self.port);
        // log_info!("📋 [WEB] 可用接口:");
        // log_info!("   GET  /api/seamless-switch/config - 获取无感换号配置");
        // log_info!("   POST /api/seamless-switch/token  - 更新无感换号token");
        // log_info!("   POST /api/error-report           - 接收Cursor错误上报");
        // log_info!("   GET  /api/auto-switch/config     - 获取自动轮换配置");
        // log_info!("   POST /api/auto-switch/config     - 更新自动轮换配置");
        // log_info!("   GET  /health                     - 健康检查");

        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;

        Ok(())
    }
}

// 处理获取无感换号配置的请求
async fn handle_get_seamless_switch_config() -> Result<impl warp::Reply, Infallible> {
    // log_info!("📖 [WEB] 收到获取无感换号配置请求");

    match NextWorkWebServer::read_seamless_switch_config().await {
        Ok(config) => {
            // log_info!("✅ [WEB] 返回无感换号配置: isSwitch={}", config.is_switch);
            Ok(warp::reply::json(&config))
        }
        Err(e) => {
            log_error!("❌ [WEB] 获取无感换号配置失败: {}", e);
            let error_response = serde_json::json!({
                "error": e,
                "accessToken": "",
                "isSwitch": 0
            });
            Ok(warp::reply::json(&error_response))
        }
    }
}

// 处理更新无感换号token的请求
#[derive(Deserialize)]
struct UpdateTokenRequest {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "email")]
    email: String,
}

// 批量设置账户自动轮换标识的请求
#[derive(Deserialize)]
struct BatchSetAutoSwitchRequest {
    emails: Vec<String>,
    #[serde(rename = "isAutoSwitch")]
    is_auto_switch: bool,
}

async fn handle_update_seamless_switch_token(
    request: UpdateTokenRequest,
) -> Result<impl warp::Reply, Infallible> {
    log_info!(
        "🔄 [WEB] 收到更新无感换号token请求: {}..., email: {}",
        &request.access_token[..request.access_token.len().min(20)],
        request.email
    );

    // HTTP API调用默认重置机器ID以确保安全性
    match NextWorkWebServer::update_seamless_switch_token(request.access_token, request.email, false, true).await {
        Ok(_) => {
            log_info!("✅ [WEB] 无感换号token更新成功");
            let response = serde_json::json!({
                "success": true,
                "message": "Token更新成功，将在10秒后自动重置切换状态"
            });
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            log_error!("❌ [WEB] 更新无感换号token失败: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": e
            });
            Ok(warp::reply::json(&response))
        }
    }
}

// 处理错误上报的请求
#[derive(Deserialize)]
struct ErrorReportRequest {
    error: String,
}

async fn handle_error_report(request: ErrorReportRequest) -> Result<impl warp::Reply, Infallible> {
    // 打印错误信息到日志
    if request.error.is_empty() {
        log_warn!("⚠️ [ERROR_REPORT] 收到空的错误上报");
        let response = serde_json::json!({
            "success": true,
            "message": "Error reported successfully"
        });
        return Ok(warp::reply::json(&response));
    }

    log_error!("🚨 [ERROR_REPORT] Cursor错误上报: {}", request.error);

    // 尝试解析JSON错误信息
    match serde_json::from_str::<serde_json::Value>(&request.error) {
        Ok(error_json) => {
            // 检查是否是 ERROR_RATE_LIMITED_CHANGEABLE 错误
            // details 是一个数组，需要遍历查找
            if let Some(details_array) = error_json.get("details").and_then(|d| d.as_array()) {
                for detail in details_array {
                    if let Some(debug) = detail.get("debug") {
                        if let Some(error_type) = debug.get("error") {
                            if error_type.as_str() == Some("ERROR_RATE_LIMITED_CHANGEABLE") {
                                log_warn!(
                                    "⚠️ [ERROR_REPORT] 检测到 ERROR_RATE_LIMITED_CHANGEABLE 错误"
                                );

                                // 防抖：5秒内只处理一次
                                let current_time = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();

                                let mut last_time = LAST_ERROR_TIME.write().await;
                                if current_time - *last_time < 5 {
                                    log_info!("⏭️ [ERROR_REPORT] 5秒内已处理过，跳过本次处理");
                                } else {
                                    *last_time = current_time;
                                    drop(last_time); // 释放锁

                                    log_info!("🔄 [ERROR_REPORT] 触发自动轮换账户逻辑");

                                    // 异步触发自动轮换
                                    tokio::spawn(async {
                                        if let Err(e) = trigger_auto_switch().await {
                                            log_error!("❌ [AUTO_SWITCH] 自动轮换失败: {}", e);
                                        }
                                    });
                                }

                                // 找到后跳出循环
                                break;
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            log_warn!("⚠️ [ERROR_REPORT] 无法解析错误JSON: {}", e);
        }
    }

    let response = serde_json::json!({
        "success": true,
        "message": "Error reported successfully"
    });
    Ok(warp::reply::json(&response))
}

// 获取自动轮换配置
async fn handle_get_auto_switch_config() -> Result<impl warp::Reply, Infallible> {
    log_info!("📖 [AUTO_SWITCH] 收到获取自动轮换配置请求");

    match NextWorkWebServer::read_auto_switch_config().await {
        Ok(config) => {
            log_info!(
                "✅ [AUTO_SWITCH] 返回自动轮换配置: enabled={}, threshold=${}",
                config.auto_switch_enabled,
                config.cost_threshold
            );
            Ok(warp::reply::json(&config))
        }
        Err(e) => {
            log_error!("❌ [AUTO_SWITCH] 获取自动轮换配置失败: {}", e);
            let error_response = serde_json::json!({
                "error": e,
                "autoSwitchEnabled": false,
                "costThreshold": 10.0
            });
            Ok(warp::reply::json(&error_response))
        }
    }
}

// 更新自动轮换配置
async fn handle_update_auto_switch_config(
    config: AutoSwitchConfig,
) -> Result<impl warp::Reply, Infallible> {
    log_info!(
        "🔄 [AUTO_SWITCH] 收到更新自动轮换配置请求: enabled={}, threshold=${}",
        config.auto_switch_enabled,
        config.cost_threshold
    );

    match NextWorkWebServer::write_auto_switch_config(&config).await {
        Ok(_) => {
            log_info!("✅ [AUTO_SWITCH] 自动轮换配置更新成功");
            let response = serde_json::json!({
                "success": true,
                "message": "自动轮换配置更新成功"
            });
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            log_error!("❌ [AUTO_SWITCH] 更新自动轮换配置失败: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": e
            });
            Ok(warp::reply::json(&response))
        }
    }
}

// 触发自动轮换账户
async fn trigger_auto_switch() -> Result<(), String> {
    log_info!("🔄 [AUTO_SWITCH] 开始执行自动轮换逻辑");

    // 1. 读取自动轮换配置
    let config = NextWorkWebServer::read_auto_switch_config().await?;

    if !config.auto_switch_enabled {
        log_info!("⏭️ [AUTO_SWITCH] 自动轮换未启用，跳过");
        return Ok(());
    }

    log_info!(
        "✅ [AUTO_SWITCH] 自动轮换已启用，费用阈值: ${}, 手动配置模式: {}",
        config.cost_threshold,
        config.manual_config_enabled
    );

    // 2. 读取无感换号配置，检查是否启用
    let seamless_config = NextWorkWebServer::read_seamless_switch_config().await?;
    if seamless_config.access_token.is_empty() {
        return Err("无感换号未启用，无法执行自动轮换".to_string());
    }

    log_info!("✅ [AUTO_SWITCH] 无感换号已启用，开始查找可用账户");

    // 3. 优先检查手动配置的账户（如果启用了手动配置模式）
    if config.manual_config_enabled {
        log_info!("🎯 [AUTO_SWITCH] 手动配置模式已启用，优先使用手动配置的账户");
        
        // 获取所有账户并筛选出手动配置的账户
        use crate::account_manager::AccountManager;
        let accounts_result = AccountManager::get_account_list();
        
        if accounts_result.success {
            let manual_accounts: Vec<_> = accounts_result.accounts
                .into_iter()
                .filter(|account| account.is_auto_switch == Some(true))
                .collect();
            
            if !manual_accounts.is_empty() {
                log_info!(
                    "📋 [AUTO_SWITCH] 找到 {} 个手动配置的账户，直接使用第一个",
                    manual_accounts.len()
                );
                
                let target_account = &manual_accounts[0];
                log_info!(
                    "🎯 [AUTO_SWITCH] 使用手动配置的账户: {}，立即切换！",
                    target_account.email
                );
                
                // 立即执行切换并返回
                return perform_account_switch(&target_account.email).await;
            } else {
                log_warn!("⚠️ [AUTO_SWITCH] 手动配置模式已启用，但没有找到配置的账户，回退到自动模式");
            }
        } else {
            log_warn!("⚠️ [AUTO_SWITCH] 无法获取账户列表，回退到自动模式");
        }
    }

    // 4. 回退到原有的基于用量的自动轮换逻辑
    log_info!("🔄 [AUTO_SWITCH] 使用基于用量的自动轮换模式");

    // 从缓存中获取符合条件的账户
    let mut eligible_accounts = {
        let cache = ELIGIBLE_ACCOUNTS_CACHE.read().await;
        cache.clone()
    };

    if eligible_accounts.is_empty() {
        log_warn!("⚠️ [AUTO_SWITCH] 缓存为空，尝试刷新缓存");
        NextWorkWebServer::refresh_eligible_accounts_cache().await?;

        // 刷新后重新读取缓存
        eligible_accounts = {
            let cache = ELIGIBLE_ACCOUNTS_CACHE.read().await;
            cache.clone()
        };

        if eligible_accounts.is_empty() {
            return Err("没有符合条件的账户（需要试用版或Pro及以上）".to_string());
        }
    }

    log_info!(
        "📋 [AUTO_SWITCH] 从缓存获取到 {} 个符合条件的账户",
        eligible_accounts.len()
    );

    // 5. 获取最近30天的时间范围
    let now = chrono::Utc::now();
    let one_month_ago = now - chrono::Duration::days(30);
    let start_date = one_month_ago.timestamp_millis() as u64;
    let end_date = now.timestamp_millis() as u64;

    log_info!(
        "📅 [AUTO_SWITCH] 时间范围: {} 到 {}",
        one_month_ago.format("%Y-%m-%d %H:%M:%S"),
        now.format("%Y-%m-%d %H:%M:%S")
    );

    // 6. 串行检查账户用量，找到第一个可用的就立即切换
    use crate::auth_checker::AuthChecker;

    log_info!(
        "🔄 [AUTO_SWITCH] 开始串行检查 {} 个账户的用量",
        eligible_accounts.len()
    );

    let mut checked_count = 0;
    let mut over_threshold_count = 0;
    let mut no_usage_count = 0;
    let mut error_count = 0;

    for (email, token) in eligible_accounts {
        checked_count += 1;
        log_info!(
            "🔍 [AUTO_SWITCH] [{}/总数] 检查账户: {}",
            checked_count,
            email
        );

        match AuthChecker::get_usage_for_period(&token, start_date, end_date, -1).await {
            Ok(Some(usage_data)) => {
                let total_cost_cents = usage_data.total_cost_cents;
                let total_cost_dollars = total_cost_cents / 100.0;

                log_info!(
                    "💰 [AUTO_SWITCH] 账户 {} 当前费用: ${:.2} (阈值: ${:.2})",
                    email,
                    total_cost_dollars,
                    config.cost_threshold
                );

                // 检查是否低于阈值
                if total_cost_dollars < config.cost_threshold {
                    log_info!(
                        "🎯 [AUTO_SWITCH] 找到可用账户: {} (费用: ${:.2} < 阈值: ${:.2})，立即切换！",
                        email,
                        total_cost_dollars,
                        config.cost_threshold
                    );

                    // 立即执行切换并返回
                    return perform_account_switch(&email).await;
                } else {
                    over_threshold_count += 1;
                    log_info!(
                        "⏭️ [AUTO_SWITCH] 账户 {} 费用 ${:.2} >= 阈值 ${:.2}，继续检查下一个",
                        email,
                        total_cost_dollars,
                        config.cost_threshold
                    );
                }
            }
            Ok(None) => {
                no_usage_count += 1;
                log_warn!(
                    "⚠️ [AUTO_SWITCH] 账户 {} 没有用量数据，继续检查下一个",
                    email
                );
            }
            Err(e) => {
                error_count += 1;
                log_error!(
                    "❌ [AUTO_SWITCH] 获取账户 {} 用量失败: {}，继续检查下一个",
                    email,
                    e
                );
            }
        }
    }

    log_error!(
        "❌ [AUTO_SWITCH] 所有账户检查完毕，未找到可用账户 - 统计: 总数={}, 超阈值={}, 无用量={}, 错误={}",
        checked_count,
        over_threshold_count,
        no_usage_count,
        error_count
    );

    Err("所有符合条件的账户都超过费用阈值或获取用量失败".to_string())
}

// 处理批量设置账户自动轮换标识的请求
async fn handle_batch_set_auto_switch(
    request: BatchSetAutoSwitchRequest,
) -> Result<impl warp::Reply, Infallible> {
    log_info!(
        "🔄 [BATCH_AUTO_SWITCH] 收到批量设置请求: {} 个账户，设置为: {}",
        request.emails.len(),
        request.is_auto_switch
    );

    use crate::account_manager::AccountManager;

    // 获取当前账户列表
    let accounts_result = AccountManager::get_account_list();
    if !accounts_result.success {
        let error_response = serde_json::json!({
            "success": false,
            "message": "无法获取账户列表",
            "updated_count": 0
        });
        return Ok(warp::reply::json(&error_response));
    }

    let mut updated_accounts = accounts_result.accounts;
    let mut updated_count = 0;

    // 更新指定账户的isAutoSwitch字段
    for account in &mut updated_accounts {
        if request.emails.contains(&account.email) {
            account.is_auto_switch = Some(request.is_auto_switch);
            updated_count += 1;
            log_info!(
                "✅ [BATCH_AUTO_SWITCH] 更新账户 {} 的自动轮换标识为: {}",
                account.email,
                request.is_auto_switch
            );
        }
    }

    // 保存更新后的账户列表
    match AccountManager::save_account_list(&updated_accounts) {
        Ok(_) => {
            log_info!(
                "✅ [BATCH_AUTO_SWITCH] 批量设置完成，成功更新 {} 个账户",
                updated_count
            );
            let response = serde_json::json!({
                "success": true,
                "message": format!("成功更新 {} 个账户的自动轮换设置", updated_count),
                "updated_count": updated_count
            });
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            log_error!("❌ [BATCH_AUTO_SWITCH] 保存账户列表失败: {}", e);
            let error_response = serde_json::json!({
                "success": false,
                "message": format!("保存失败: {}", e),
                "updated_count": 0
            });
            Ok(warp::reply::json(&error_response))
        }
    }
}

// 执行账户切换
async fn perform_account_switch(email: &str) -> Result<(), String> {
    log_info!("🔄 [AUTO_SWITCH] 开始切换到账户: {}", email);

    use crate::account_manager::AccountManager;
    use crate::machine_id::MachineIdRestorer;

    // 1. 执行完全重置机器码
    log_info!("🔄 [AUTO_SWITCH] 第一步：重置机器码");
    let restorer =
        MachineIdRestorer::new().map_err(|e| format!("创建MachineIdRestorer失败: {}", e))?;
    let reset_result = restorer
        .complete_cursor_reset()
        .map_err(|e| format!("重置机器码失败: {}", e))?;

    if !reset_result.success {
        return Err(format!("重置机器码失败: {}", reset_result.message));
    }

    log_info!("✅ [AUTO_SWITCH] 机器码重置成功");

    // 2. 执行传统切换（更新 Storage 文件）
    log_info!("🔄 [AUTO_SWITCH] 第二步：执行传统切换");
    let switch_result = AccountManager::switch_account(email.to_string(), false);

    if !switch_result.success {
        return Err(format!("传统切换失败: {}", switch_result.message));
    }

    log_info!("✅ [AUTO_SWITCH] 传统切换成功");

    // 3. 获取账户 token 并更新无感换号配置
    log_info!("🔄 [AUTO_SWITCH] 第三步：更新无感换号配置");

    // 从账户列表中获取 token
    let accounts_result = AccountManager::get_account_list();
    if !accounts_result.success {
        return Err("无法获取账户列表".to_string());
    }

    let target_account = accounts_result
        .accounts
        .iter()
        .find(|acc| acc.email == email)
        .ok_or_else(|| format!("找不到账户: {}", email))?;

    // 更新无感换号 token (自动轮换模式，默认重置机器ID)
    NextWorkWebServer::update_seamless_switch_token(target_account.token.clone(), email.to_string(), true, true).await?;

    log_info!("✅ [AUTO_SWITCH] 无感换号配置已更新");

    // 轮换到当前账户后，清除该账户的「自动轮换」标识，避免下次仍被当作待轮换队列首位
    let accounts_after = AccountManager::get_account_list();
    if accounts_after.success {
        let mut accounts = accounts_after.accounts;
        let mut cleared = false;
        for acc in &mut accounts {
            if acc.email == email && acc.is_auto_switch == Some(true) {
                acc.is_auto_switch = Some(false);
                cleared = true;
                break;
            }
        }
        if cleared {
            match AccountManager::save_account_list(&accounts) {
                Ok(_) => log_info!(
                    "✅ [AUTO_SWITCH] 已清除账户 {} 的自动轮换标识",
                    email
                ),
                Err(e) => log_warn!(
                    "⚠️ [AUTO_SWITCH] 清除自动轮换标识后保存账户列表失败: {}",
                    e
                ),
            }
        }
    }

    log_info!("✅ [AUTO_SWITCH] 账户自动轮换完成: {}", email);

    Ok(())
}
