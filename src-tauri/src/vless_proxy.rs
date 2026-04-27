use once_cell::sync::Lazy;
use reqwest::Url;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

struct VlessRuntime {
    child: Child,
    config_path: String,
}

static VLESS_RUNTIME: Lazy<Mutex<Option<VlessRuntime>>> = Lazy::new(|| Mutex::new(None));
static VLESS_DOWNLOAD_CANCELLED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartVlessProxyResult {
    pub http_proxy: String,
    pub socks_proxy: String,
    pub http_port: u16,
    pub socks_port: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VlessDownloadProgressPayload {
    stage: String,
    message: String,
    received_bytes: u64,
    total_bytes: Option<u64>,
    percent: Option<f64>,
}

fn emit_download_progress(
    app: &AppHandle,
    stage: &str,
    message: String,
    received_bytes: u64,
    total_bytes: Option<u64>,
) {
    let percent = total_bytes.and_then(|total| {
        if total > 0 {
            Some((received_bytes as f64 / total as f64) * 100.0)
        } else {
            None
        }
    });
    let payload = VlessDownloadProgressPayload {
        stage: stage.to_string(),
        message,
        received_bytes,
        total_bytes,
        percent,
    };
    let _ = app.emit("vless-xray-download-progress", &payload);
}

fn sanitize_port(port: Option<u16>, fallback: u16) -> u16 {
    let candidate = port.unwrap_or(fallback);
    if (1..=65535).contains(&candidate) {
        candidate
    } else {
        fallback
    }
}

fn stop_vless_runtime_internal() -> Result<(), String> {
    let mut guard = VLESS_RUNTIME
        .lock()
        .map_err(|_| "vless runtime lock poisoned".to_string())?;

    if let Some(mut runtime) = guard.take() {
        let _ = runtime.child.kill();
        let _ = runtime.child.wait();
        let _ = fs::remove_file(&runtime.config_path);
    }
    Ok(())
}

fn append_if_exists(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() {
        candidates.push(path);
    }
}

fn find_in_path(binary_name: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let Some(path_os) = env::var_os("PATH") else {
        return results;
    };

    for dir in env::split_paths(&path_os) {
        let candidate = dir.join(binary_name);
        if candidate.is_file() {
            results.push(candidate);
        }
    }
    results
}

fn collect_xray_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    let binary_name = "xray.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "xray";

    let mut candidates = Vec::new();

    if let Ok(custom) = env::var("XRAY_BIN") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            append_if_exists(&mut candidates, PathBuf::from(trimmed));
        }
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            append_if_exists(&mut candidates, exe_dir.join(binary_name));

            #[cfg(target_os = "windows")]
            let platform = "windows";
            #[cfg(target_os = "macos")]
            let platform = "macos";
            #[cfg(target_os = "linux")]
            let platform = "linux";

            append_if_exists(&mut candidates, exe_dir.join("pyBuild").join(platform).join(binary_name));
            append_if_exists(
                &mut candidates,
                exe_dir
                    .join("../Resources/pyBuild")
                    .join(platform)
                    .join(binary_name),
            );
        }
    }

    append_if_exists(&mut candidates, PathBuf::from(format!("/usr/local/bin/{}", binary_name)));
    append_if_exists(&mut candidates, PathBuf::from(format!("/opt/homebrew/bin/{}", binary_name)));
    append_if_exists(&mut candidates, PathBuf::from(format!("/usr/bin/{}", binary_name)));

    for path_hit in find_in_path(binary_name) {
        append_if_exists(&mut candidates, path_hit);
    }

    candidates.sort();
    candidates.dedup();
    candidates
}

fn pick_xray_executable() -> Result<PathBuf, String> {
    let candidates = collect_xray_candidates();
    if let Some(path) = candidates.first() {
        return Ok(path.clone());
    }

    Err(
        "未找到 xray 可执行文件。请安装 xray，或设置 XRAY_BIN 指向 xray 绝对路径。已尝试：XRAY_BIN、应用目录、pyBuild 目录和系统 PATH"
            .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn platform_xray_binary_name() -> &'static str {
    "xray.exe"
}

#[cfg(not(target_os = "windows"))]
fn platform_xray_binary_name() -> &'static str {
    "xray"
}

fn platform_pybuild_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn platform_xray_asset_name() -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            Some("Xray-macos-arm64-v8a.zip")
        } else {
            Some("Xray-macos-64.zip")
        }
    }
    #[cfg(target_os = "linux")]
    {
        if cfg!(target_arch = "aarch64") {
            Some("Xray-linux-arm64-v8a.zip")
        } else {
            Some("Xray-linux-64.zip")
        }
    }
    #[cfg(target_os = "windows")]
    {
        if cfg!(target_arch = "aarch64") {
            Some("Xray-windows-arm64-v8a.zip")
        } else {
            Some("Xray-windows-64.zip")
        }
    }
}

fn pybuild_xray_target_path() -> Result<PathBuf, String> {
    let app_dir = crate::get_app_dir()?;
    let target_dir = app_dir.join("pyBuild").join(platform_pybuild_name());
    fs::create_dir_all(&target_dir).map_err(|e| format!("创建 pyBuild 目录失败: {}", e))?;
    Ok(target_dir.join(platform_xray_binary_name()))
}

async fn download_xray_to_pybuild(app: &AppHandle) -> Result<PathBuf, String> {
    let asset = platform_xray_asset_name()
        .ok_or_else(|| "当前平台暂不支持自动下载 xray".to_string())?;
    let download_url = format!(
        "https://github.com/XTLS/Xray-core/releases/latest/download/{}",
        asset
    );

    emit_download_progress(
        app,
        "prepare",
        format!("开始下载 Xray-core: {}", asset),
        0,
        None,
    );

    let response = reqwest::get(&download_url)
        .await
        .map_err(|e| format!("下载 xray 失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载 xray 失败，HTTP 状态码: {}", response.status()));
    }
    let total_bytes = response.content_length();
    emit_download_progress(
        app,
        "downloading",
        "正在下载 xray 压缩包...".to_string(),
        0,
        total_bytes,
    );

    let mut downloaded = Vec::new();
    let mut received_bytes: u64 = 0;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("读取 xray 下载内容失败: {}", e))?
    {
        if VLESS_DOWNLOAD_CANCELLED.load(Ordering::Relaxed) {
            emit_download_progress(
                app,
                "cancelled",
                "已取消 xray 下载".to_string(),
                received_bytes,
                total_bytes,
            );
            return Err("用户已取消 xray 下载".to_string());
        }
        received_bytes += chunk.len() as u64;
        downloaded.extend_from_slice(&chunk);
        emit_download_progress(
            app,
            "downloading",
            "正在下载 xray 压缩包...".to_string(),
            received_bytes,
            total_bytes,
        );
    }

    emit_download_progress(
        app,
        "extracting",
        "下载完成，开始解压 xray...".to_string(),
        received_bytes,
        total_bytes,
    );

    let mut archive = zip::ZipArchive::new(Cursor::new(downloaded))
        .map_err(|e| format!("解析 xray 压缩包失败: {}", e))?;
    let binary_name = platform_xray_binary_name();
    let target_path = pybuild_xray_target_path()?;

    let mut extracted = false;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取压缩包文件失败: {}", e))?;
        let normalized_name = file.name().replace('\\', "/");
        if normalized_name.ends_with(binary_name) {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| format!("读取 xray 文件内容失败: {}", e))?;
            fs::write(&target_path, buffer).map_err(|e| format!("写入 xray 文件失败: {}", e))?;
            #[cfg(unix)]
            {
                let mut permissions = fs::metadata(&target_path)
                    .map_err(|e| format!("读取 xray 文件权限失败: {}", e))?
                    .permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&target_path, permissions)
                    .map_err(|e| format!("设置 xray 可执行权限失败: {}", e))?;
            }
            extracted = true;
            break;
        }
    }

    if !extracted {
        return Err("压缩包中未找到 xray 可执行文件".to_string());
    }

    emit_download_progress(
        app,
        "completed",
        format!("xray 已下载并写入: {}", target_path.display()),
        received_bytes,
        total_bytes,
    );

    Ok(target_path)
}

async fn ensure_xray_executable(app: &AppHandle) -> Result<PathBuf, String> {
    match pick_xray_executable() {
        Ok(path) => Ok(path),
        Err(_) => download_xray_to_pybuild(app).await,
    }
}

fn build_xray_config(
    vless_url: &str,
    http_port: u16,
    socks_port: u16,
) -> Result<serde_json::Value, String> {
    let parsed = Url::parse(vless_url).map_err(|e| format!("无效的 vless 链接: {}", e))?;
    if parsed.scheme() != "vless" {
        return Err("仅支持 vless:// 链接".to_string());
    }

    let user_id = parsed.username().trim();
    if user_id.is_empty() {
        return Err("vless 链接缺少用户ID".to_string());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "vless 链接缺少 host".to_string())?;
    let port = parsed
        .port()
        .ok_or_else(|| "vless 链接缺少端口".to_string())?;

    let mut flow = String::new();
    let mut security = String::new();
    let mut sni = String::new();
    let mut pbk = String::new();
    let mut fp = String::from("chrome");

    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "flow" => flow = value.into_owned(),
            "security" => security = value.into_owned(),
            "sni" => sni = value.into_owned(),
            "pbk" => pbk = value.into_owned(),
            "fp" => fp = value.into_owned(),
            _ => {}
        }
    }

    if security != "reality" {
        return Err("当前仅支持 VLESS + REALITY (security=reality)".to_string());
    }
    if sni.is_empty() || pbk.is_empty() {
        return Err("vless reality 链接缺少 sni 或 pbk 参数".to_string());
    }

    Ok(serde_json::json!({
      "log": { "loglevel": "warning" },
      "inbounds": [
        {
          "tag": "http-in",
          "listen": "127.0.0.1",
          "port": http_port,
          "protocol": "http",
          "settings": {}
        },
        {
          "tag": "socks-in",
          "listen": "127.0.0.1",
          "port": socks_port,
          "protocol": "socks",
          "settings": { "auth": "noauth", "udp": true }
        }
      ],
      "outbounds": [
        {
          "tag": "vless-out",
          "protocol": "vless",
          "settings": {
            "vnext": [
              {
                "address": host,
                "port": port,
                "users": [
                  {
                    "id": user_id,
                    "encryption": "none",
                    "flow": flow
                  }
                ]
              }
            ]
          },
          "streamSettings": {
            "network": "tcp",
            "security": "reality",
            "realitySettings": {
              "serverName": sni,
              "publicKey": pbk,
              "fingerprint": if fp.is_empty() { "chrome".to_string() } else { fp }
            }
          }
        }
      ]
    }))
}

#[tauri::command]
pub async fn start_vless_proxy(
    app: AppHandle,
    vless_url: String,
    http_port: Option<u16>,
    socks_port: Option<u16>,
) -> Result<StartVlessProxyResult, String> {
    VLESS_DOWNLOAD_CANCELLED.store(false, Ordering::Relaxed);
    stop_vless_runtime_internal()?;

    let http_port = sanitize_port(http_port, 8990);
    let socks_port = sanitize_port(socks_port, 1990);
    let config = build_xray_config(&vless_url, http_port, socks_port)?;

    let xray_path = ensure_xray_executable(&app).await?;

    let config_path = std::env::temp_dir().join(format!("cursor_xray_{}.json", Uuid::new_v4()));
    let config_text =
        serde_json::to_string_pretty(&config).map_err(|e| format!("生成 xray 配置失败: {}", e))?;
    fs::write(&config_path, config_text).map_err(|e| format!("写入 xray 配置失败: {}", e))?;

    let child = Command::new(&xray_path)
        .args(["run", "-c", &config_path.to_string_lossy()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 xray 失败({}): {}", xray_path.display(), e))?;

    {
        let mut guard = VLESS_RUNTIME
            .lock()
            .map_err(|_| "vless runtime lock poisoned".to_string())?;
        *guard = Some(VlessRuntime {
            child,
            config_path: config_path.to_string_lossy().to_string(),
        });
    }

    Ok(StartVlessProxyResult {
        http_proxy: format!("127.0.0.1:{}", http_port),
        socks_proxy: format!("127.0.0.1:{}", socks_port),
        http_port,
        socks_port,
    })
}

#[tauri::command]
pub fn stop_vless_proxy() -> Result<(), String> {
    stop_vless_runtime_internal()
}

#[tauri::command]
pub fn cancel_vless_xray_download() -> Result<(), String> {
    VLESS_DOWNLOAD_CANCELLED.store(true, Ordering::Relaxed);
    Ok(())
}
