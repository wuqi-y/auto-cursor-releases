use crate::auth_checker::AuthChecker;
use crate::{log_error, log_info, log_warn};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipboardResult {
    pub success: bool,
    pub data: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TempmailClearResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TempmailListResponse {
    result: bool,
    first_id: Option<u64>,
    #[serde(default)]
    mail_list: Vec<serde_json::Value>,
}

/// 从剪贴板读取文本
#[command]
pub async fn read_clipboard() -> Result<ClipboardResult, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| format!("Failed to execute pbpaste: {}", e))?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(ClipboardResult {
                success: true,
                data: Some(text),
                message: "读取剪贴板成功".to_string(),
            })
        } else {
            Ok(ClipboardResult {
                success: false,
                data: None,
                message: "读取剪贴板失败".to_string(),
            })
        }
    }

    #[cfg(target_os = "windows")]
    {
        use clipboard_win::{formats, get_clipboard};

        match get_clipboard::<String, formats::Unicode>(formats::Unicode) {
            Ok(text) => Ok(ClipboardResult {
                success: true,
                data: Some(text),
                message: "读取剪贴板成功".to_string(),
            }),
            Err(e) => Ok(ClipboardResult {
                success: false,
                data: None,
                message: format!("读取剪贴板失败: {}", e),
            }),
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // 尝试使用 xclip
        let output = Command::new("xclip")
            .args(&["-selection", "clipboard", "-o"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                Ok(ClipboardResult {
                    success: true,
                    data: Some(text),
                    message: "读取剪贴板成功".to_string(),
                })
            }
            _ => {
                // 尝试使用 xsel 作为备选
                let output2 = Command::new("xsel")
                    .args(&["--clipboard", "--output"])
                    .output();

                match output2 {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout).to_string();
                        Ok(ClipboardResult {
                            success: true,
                            data: Some(text),
                            message: "读取剪贴板成功".to_string(),
                        })
                    }
                    _ => Ok(ClipboardResult {
                        success: false,
                        data: None,
                        message: "读取剪贴板失败，请确保安装了 xclip 或 xsel".to_string(),
                    }),
                }
            }
        }
    }
}

/// 写入文本到剪贴板
#[command]
pub async fn write_clipboard(text: String) -> Result<ClipboardResult, String> {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn pbcopy: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
        }

        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for pbcopy: {}", e))?;

        if status.success() {
            Ok(ClipboardResult {
                success: true,
                data: None,
                message: "写入剪贴板成功".to_string(),
            })
        } else {
            Ok(ClipboardResult {
                success: false,
                data: None,
                message: "写入剪贴板失败".to_string(),
            })
        }
    }

    #[cfg(target_os = "windows")]
    {
        use clipboard_win::{formats, set_clipboard};

        match set_clipboard(formats::Unicode, &text) {
            Ok(_) => Ok(ClipboardResult {
                success: true,
                data: None,
                message: "写入剪贴板成功".to_string(),
            }),
            Err(e) => Ok(ClipboardResult {
                success: false,
                data: None,
                message: format!("写入剪贴板失败: {}", e),
            }),
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};

        // 尝试使用 xclip
        let mut child = Command::new("xclip")
            .args(&["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn();

        match child {
            Ok(mut process) => {
                if let Some(mut stdin) = process.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        if process.wait().is_ok() {
                            return Ok(ClipboardResult {
                                success: true,
                                data: None,
                                message: "写入剪贴板成功".to_string(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        // 尝试使用 xsel 作为备选
        let mut child2 = Command::new("xsel")
            .args(&["--clipboard", "--input"])
            .stdin(Stdio::piped())
            .spawn();

        match child2 {
            Ok(mut process) => {
                if let Some(mut stdin) = process.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        if process.wait().is_ok() {
                            return Ok(ClipboardResult {
                                success: true,
                                data: None,
                                message: "写入剪贴板成功".to_string(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(ClipboardResult {
            success: false,
            data: None,
            message: "写入剪贴板失败，请确保安装了 xclip 或 xsel".to_string(),
        })
    }
}

/// 清空 Tempmail 邮箱
#[command]
pub async fn clear_tempmail_inbox(
    email: String,
    pin: Option<String>,
) -> Result<TempmailClearResult, String> {
    let client = reqwest::Client::new();

    log_info!("🗑️ 清空 Tempmail 邮箱: {}", email);
    log_info!("📝 请求参数: email={}, pin={:?}", email, pin);

    // 第一步：获取邮件列表以获取 first_id
    let encoded_email = urlencoding::encode(&email);
    let list_url = if let Some(p) = &pin {
        format!(
            "https://tempmail.plus/api/mails?email={}&limit=1&epin={}",
            encoded_email, p
        )
    } else {
        format!(
            "https://tempmail.plus/api/mails?email={}&limit=1",
            encoded_email
        )
    };

    log_info!("📬 获取邮件列表: {}", list_url);

    let first_id = match client.get(&list_url).send().await {
        Ok(response) => {
            let status = response.status();
            log_info!("📡 列表响应状态码: {}", status);

            if status.is_success() {
                match response.json::<TempmailListResponse>().await {
                    Ok(list_data) => {
                        log_info!(
                            "📋 邮件列表数据: result={}, first_id={:?}, mail_count={}",
                            list_data.result,
                            list_data.first_id,
                            list_data.mail_list.len()
                        );
                        list_data.first_id
                    }
                    Err(e) => {
                        log_warn!("⚠️ 解析邮件列表失败: {}, 继续尝试删除", e);
                        None
                    }
                }
            } else {
                log_warn!("⚠️ 获取邮件列表失败，状态码: {}, 继续尝试删除", status);
                None
            }
        }
        Err(e) => {
            log_warn!("⚠️ 请求邮件列表失败: {}, 继续尝试删除", e);
            None
        }
    };

    // 第二步：删除邮件，带上 first_id
    let delete_url = "https://tempmail.plus/api/mails/";
    let mut form = vec![("email", email.as_str())];
    let pin_str;
    if let Some(p) = &pin {
        pin_str = p.clone();
        form.push(("epin", pin_str.as_str()));
    }

    let first_id_str;
    if let Some(id) = first_id {
        first_id_str = id.to_string();
        form.push(("first_id", first_id_str.as_str()));
        log_info!("🔑 添加 first_id 参数: {}", id);
    } else {
        log_info!("⚠️ 未获取到 first_id，尝试不带此参数删除");
    }

    log_info!("🗑️ 发送删除请求，参数: {:?}", form);

    match client.delete(delete_url).form(&form).send().await {
        Ok(response) => {
            let status = response.status();
            log_info!("📡 删除响应状态码: {}", status);

            // 尝试读取响应体
            match response.text().await {
                Ok(body) => {
                    log_info!("📄 删除响应内容: {}", body);

                    if status.is_success() {
                        log_info!("✅ Tempmail 邮箱清空成功");
                        Ok(TempmailClearResult {
                            success: true,
                            message: "Tempmail 邮箱清空成功".to_string(),
                        })
                    } else {
                        log_warn!(
                            "⚠️ Tempmail 邮箱清空失败，状态码: {}, 响应: {}",
                            status,
                            body
                        );
                        Ok(TempmailClearResult {
                            success: false,
                            message: format!("清空失败，状态码: {}, 响应: {}", status, body),
                        })
                    }
                }
                Err(e) => {
                    log_error!("❌ 读取响应内容失败: {}", e);
                    if status.is_success() {
                        Ok(TempmailClearResult {
                            success: true,
                            message: "Tempmail 邮箱清空成功（无响应内容）".to_string(),
                        })
                    } else {
                        Ok(TempmailClearResult {
                            success: false,
                            message: format!("清空失败，状态码: {}", status),
                        })
                    }
                }
            }
        }
        Err(e) => {
            log_error!("❌ 清空 Tempmail 邮箱失败: {}", e);
            Ok(TempmailClearResult {
                success: false,
                message: format!("清空失败: {}", e),
            })
        }
    }
}

// ===== Usage API相关代码 =====

// Helper enum for protobuf field values
enum FieldValue {
    Varint(u64),
    Fixed64(u64),
    Fixed32(u32),
    String(String),
    Bytes(Vec<u8>),
}

impl FieldValue {
    fn to_json(&self) -> serde_json::Value {
        match self {
            FieldValue::Varint(v) => serde_json::json!(v),
            FieldValue::Fixed64(v) => serde_json::json!(v),
            FieldValue::Fixed32(v) => serde_json::json!(v),
            FieldValue::String(s) => serde_json::json!(s),
            FieldValue::Bytes(b) => serde_json::json!(hex::encode(b)),
        }
    }
}

/// Parse protobuf varint
fn parse_varint(data: &[u8], offset: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;

    while *offset < data.len() {
        let byte = data[*offset];
        *offset += 1;

        result |= ((byte & 0x7F) as u64) << shift;

        if (byte & 0x80) == 0 {
            return Some(result);
        }

        shift += 7;
        if shift >= 64 {
            return None; // Overflow
        }
    }

    None
}

/// Parse GetCurrentPeriodUsage protobuf response
fn parse_period_usage_protobuf(data: &[u8]) -> serde_json::Value {
    let mut offset = 0;
    let mut result = serde_json::json!({
        "raw_fields": {},
        "parsed": {},
        "raw_hex": hex::encode(data)
    });

    log_info!(
        "🔍 开始解析 GetCurrentPeriodUsage protobuf 响应，长度: {} bytes",
        data.len()
    );

    // Parse all fields first
    let mut fields: std::collections::HashMap<u32, Vec<FieldValue>> =
        std::collections::HashMap::new();

    while offset < data.len() {
        // Read field key
        if let Some(key) = parse_varint(data, &mut offset) {
            let field_number = (key >> 3) as u32;
            let wire_type = (key & 0x7) as u8;

            log_info!("📝 Field {}, wire type {}", field_number, wire_type);

            let field_value = match wire_type {
                0 => {
                    // Varint
                    if let Some(value) = parse_varint(data, &mut offset) {
                        log_info!("   Varint 值: {}", value);
                        Some(FieldValue::Varint(value))
                    } else {
                        None
                    }
                }
                2 => {
                    // Length-delimited (string, bytes, embedded message)
                    if let Some(length) = parse_varint(data, &mut offset) {
                        let length = length as usize;
                        if offset + length <= data.len() {
                            let field_data = &data[offset..offset + length];
                            offset += length;

                            // Try to parse as UTF-8 string
                            if let Ok(text) = String::from_utf8(field_data.to_vec()) {
                                log_info!("   String 值: {}", text);
                                Some(FieldValue::String(text))
                            } else {
                                // If not valid UTF-8, might be embedded message
                                log_info!("   Bytes/Message 长度: {}", length);
                                Some(FieldValue::Bytes(field_data.to_vec()))
                            }
                        } else {
                            log_error!("❌ Length-delimited 字段超出边界");
                            break;
                        }
                    } else {
                        None
                    }
                }
                1 => {
                    // 64-bit
                    if offset + 8 <= data.len() {
                        let value = u64::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                            data[offset + 4],
                            data[offset + 5],
                            data[offset + 6],
                            data[offset + 7],
                        ]);
                        offset += 8;
                        log_info!("   64-bit 值: {}", value);
                        Some(FieldValue::Fixed64(value))
                    } else {
                        None
                    }
                }
                5 => {
                    // 32-bit
                    if offset + 4 <= data.len() {
                        let value = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]);
                        offset += 4;
                        log_info!("   32-bit 值: {}", value);
                        Some(FieldValue::Fixed32(value))
                    } else {
                        None
                    }
                }
                _ => {
                    log_error!("❌ 未知的 wire type: {}", wire_type);
                    break;
                }
            };

            if let Some(val) = field_value {
                fields
                    .entry(field_number)
                    .or_insert_with(Vec::new)
                    .push(val);
            }
        } else {
            break;
        }
    }

    // Convert fields to JSON for raw output
    for (field_num, values) in &fields {
        for (idx, val) in values.iter().enumerate() {
            let key = if values.len() == 1 {
                format!("field_{}", field_num)
            } else {
                format!("field_{}_{}", field_num, idx)
            };
            result["raw_fields"][&key] = val.to_json();
        }
    }

    // Parse into structured data based on Cursor's protobuf schema
    // 根据实际数据重新映射：
    // field_1: billingCycleStart (毫秒时间戳)
    if let Some(values) = fields.get(&1) {
        if let Some(FieldValue::Varint(v)) = values.first() {
            result["parsed"]["billingCycleStart"] = serde_json::json!(v);
        }
    }

    // field_2: billingCycleEnd (毫秒时间戳)
    if let Some(values) = fields.get(&2) {
        if let Some(FieldValue::Varint(v)) = values.first() {
            result["parsed"]["billingCycleEnd"] = serde_json::json!(v);
        }
    }

    // field_3: spendLimitUsage 或其他嵌入消息
    if let Some(values) = fields.get(&3) {
        if let Some(FieldValue::Bytes(bytes)) = values.first() {
            // 尝试解析为 spendLimitUsage
            result["parsed"]["spendLimitUsage"] = parse_spend_limit_usage(bytes);
        }
    }

    // field_4: displayMessage (可能不是标准字符串)
    if let Some(values) = fields.get(&4) {
        if let Some(FieldValue::String(s)) = values.first() {
            result["parsed"]["field4Data"] = serde_json::json!(s);
        }
    }

    // field_5: displayThreshold
    if let Some(values) = fields.get(&5) {
        if let Some(FieldValue::Varint(v)) = values.first() {
            result["parsed"]["displayThreshold"] = serde_json::json!(v);
        }
    }

    // field_6: enabled
    if let Some(values) = fields.get(&6) {
        if let Some(FieldValue::Varint(v)) = values.first() {
            result["parsed"]["enabled"] = serde_json::json!(*v != 0);
        }
    }

    // field_7: displayMessage / usageProgressMessage
    if let Some(values) = fields.get(&7) {
        if let Some(FieldValue::String(msg)) = values.first() {
            result["parsed"]["displayMessage"] = serde_json::json!(msg);
            result["parsed"]["usageProgressMessage"] = serde_json::json!(msg);

            // Extract progress percentage
            let progress = extract_usage_progress(msg);
            result["parsed"]["usageProgressPercentage"] = serde_json::json!(progress);
            log_info!("📊 使用进度: {}%", progress);
        }
    }

    result
}

/// Extract usage progress percentage from displayMessage
fn extract_usage_progress(message: &str) -> u32 {
    // Check if user hit the limit
    if message.contains("You've hit your usage limit")
        || message.contains("You have hit your usage limit")
    {
        log_info!("🔴 用户已达到使用限制");
        return 100;
    }

    // Try to extract percentage from "You've used X% of your usage limit"
    if let Some(pos) = message.find("You've used ") {
        let after_prefix = &message[pos + 12..]; // "You've used ".len() = 12
        if let Some(percent_pos) = after_prefix.find('%') {
            let percent_str = &after_prefix[..percent_pos].trim();
            if let Ok(percent) = percent_str.parse::<u32>() {
                log_info!("📊 提取到使用进度: {}%", percent);
                return percent;
            }
        }
    }

    // Try alternative format "You have used X% of your usage limit"
    if let Some(pos) = message.find("You have used ") {
        let after_prefix = &message[pos + 14..]; // "You have used ".len() = 14
        if let Some(percent_pos) = after_prefix.find('%') {
            let percent_str = &after_prefix[..percent_pos].trim();
            if let Ok(percent) = percent_str.parse::<u32>() {
                log_info!("📊 提取到使用进度: {}%", percent);
                return percent;
            }
        }
    }

    log_warn!("⚠️ 无法从消息中提取进度: {}", message);
    0 // Default to 0 if cannot parse
}

// Parse planUsage message
fn parse_plan_usage(data: &[u8]) -> serde_json::Value {
    let mut offset = 0;
    let mut result = serde_json::json!({});

    while offset < data.len() {
        if let Some(key) = parse_varint(data, &mut offset) {
            let field_number = (key >> 3) as u32;
            let wire_type = (key & 0x7) as u8;

            match (field_number, wire_type) {
                (1, 0) => {
                    // totalSpend
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["totalSpend"] = serde_json::json!(value);
                        result["totalSpendDollars"] = serde_json::json!(value as f64 / 100.0);
                    }
                }
                (2, 0) => {
                    // includedSpend
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["includedSpend"] = serde_json::json!(value);
                        result["includedSpendDollars"] = serde_json::json!(value as f64 / 100.0);
                    }
                }
                (3, 0) => {
                    // bonusSpend
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["bonusSpend"] = serde_json::json!(value);
                        result["bonusSpendDollars"] = serde_json::json!(value as f64 / 100.0);
                    }
                }
                (4, 0) => {
                    // limit
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["limit"] = serde_json::json!(value);
                        result["limitDollars"] = serde_json::json!(value as f64 / 100.0);
                    }
                }
                (5, 0) => {
                    // remainingBonus
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["remainingBonus"] = serde_json::json!(value != 0);
                    }
                }
                (6, 2) => {
                    // bonusTooltip (string)
                    if let Some(length) = parse_varint(data, &mut offset) {
                        let length = length as usize;
                        if offset + length <= data.len() {
                            if let Ok(text) =
                                String::from_utf8(data[offset..offset + length].to_vec())
                            {
                                result["bonusTooltip"] = serde_json::json!(text);
                            }
                            offset += length;
                        }
                    }
                }
                _ => {
                    skip_field(data, &mut offset, wire_type);
                }
            }
        } else {
            break;
        }
    }

    result
}

// Parse spendLimitUsage message
fn parse_spend_limit_usage(data: &[u8]) -> serde_json::Value {
    let mut offset = 0;
    let mut result = serde_json::json!({});

    while offset < data.len() {
        if let Some(key) = parse_varint(data, &mut offset) {
            let field_number = (key >> 3) as u32;
            let wire_type = (key & 0x7) as u8;

            match (field_number, wire_type) {
                (1, 0) => {
                    // individualUsed
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["individualUsed"] = serde_json::json!(value);
                        result["individualUsedDollars"] = serde_json::json!(value as f64 / 100.0);
                    }
                }
                (2, 0) => {
                    // field_2 (可能是另一个 used 值)
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["field_2"] = serde_json::json!(value);
                    }
                }
                (3, 0) => {
                    // field_3 (可能是某个限额)
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["field_3"] = serde_json::json!(value);
                    }
                }
                (4, 0) => {
                    // field_4 (可能是真实的剩余额度)
                    if let Some(value) = parse_varint(data, &mut offset) {
                        result["individualLimit"] = serde_json::json!(value);
                        result["individualLimitDollars"] = serde_json::json!(value as f64 / 100.0);
                        log_info!(
                            "💰 从 field_4 解析到个人剩余额度: {} cents (${:.2})",
                            value,
                            value as f64 / 100.0
                        );
                    }
                }
                _ => {
                    skip_field(data, &mut offset, wire_type);
                }
            }
        } else {
            break;
        }
    }

    result
}

// Helper function to skip unknown fields
fn skip_field(data: &[u8], offset: &mut usize, wire_type: u8) {
    match wire_type {
        0 => {
            parse_varint(data, offset);
        }
        1 => {
            *offset += 8;
        }
        2 => {
            if let Some(length) = parse_varint(data, offset) {
                *offset += length as usize;
            }
        }
        5 => {
            *offset += 4;
        }
        _ => {}
    }
}

/// Get current period usage with Bearer token (using GetCurrentPeriodUsage API)
pub async fn get_current_period_usage_impl(token: &str) -> Result<serde_json::Value> {
    // 使用 AuthChecker 的方法清理 token 和生成 checksum
    let clean_token = AuthChecker::clean_token_public(token)?;
    let checksum = AuthChecker::generate_cursor_checksum_public(&clean_token)?;

    let client = reqwest::Client::new();

    // Generate random UUIDs for request
    use uuid::Uuid;
    let request_id = Uuid::new_v4().to_string();
    let session_id = Uuid::new_v4().to_string();
    let trace_id = format!("{:032x}", Uuid::new_v4().as_u128());
    let trace_parent = format!("{:016x}", Uuid::new_v4().as_u128() >> 64);
    let client_key = "7e56afa7f20c8f588133dccd7cc5cbaccee5baf4330f2360e40ab6873b0b00d6";
    let config_version = "f8182f4c-83ba-4b18-a457-2ef3840a7808";

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("User-Agent", "connect-es/1.6.1".parse()?);
    headers.insert("Connection", "close".parse()?);
    headers.insert("Accept-Encoding", "gzip".parse()?);
    headers.insert("authorization", format!("Bearer {}", clean_token).parse()?);
    headers.insert("connect-protocol-version", "1".parse()?);
    headers.insert("content-length", "0".parse()?);
    headers.insert("content-type", "application/proto".parse()?);
    headers.insert(
        "traceparent",
        format!("00-{}-{}-00", trace_id, trace_parent).parse()?,
    );
    headers.insert("x-amzn-trace-id", format!("Root={}", request_id).parse()?);
    headers.insert("x-client-key", client_key.parse()?);
    headers.insert("x-cursor-checksum", checksum.parse()?);
    headers.insert("x-cursor-client-version", "1.5.5".parse()?);
    headers.insert("x-cursor-config-version", config_version.parse()?);
    headers.insert("x-cursor-streaming", "true".parse()?);
    headers.insert("x-cursor-timezone", "Asia/Shanghai".parse()?);
    headers.insert("x-ghost-mode", "false".parse()?);
    headers.insert("x-new-onboarding-completed", "true".parse()?);
    headers.insert("x-request-id", request_id.clone().parse()?);
    headers.insert("x-session-id", session_id.parse()?);

    log_info!("🔍 调用 GetCurrentPeriodUsage API");
    log_info!("📝 Request ID: {}", request_id);

    let response = client
        .post("https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage")
        .headers(headers)
        .body(vec![]) // Empty body
        .timeout(std::time::Duration::from_secs(40))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            log_info!("📡 响应状态: {}", status);

            if status.is_success() {
                if let Ok(body_bytes) = resp.bytes().await {
                    log_info!("✅ 成功! 响应长度: {} bytes", body_bytes.len());

                    if body_bytes.len() > 0 {
                        // Output hex dump for debugging
                        let hex_string: String = body_bytes
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<String>>()
                            .join(" ");
                        log_info!(
                            "📦 响应十六进制（前100字节）: {}",
                            &hex_string[..std::cmp::min(300, hex_string.len())]
                        );

                        // Parse protobuf response
                        log_info!("📦 这是protobuf格式，开始解析");
                        let parsed = parse_period_usage_protobuf(&body_bytes);

                        return Ok(serde_json::json!({
                            "success": true,
                            "message": "成功获取当前周期用量",
                            "response_length": body_bytes.len(),
                            "parsed_data": parsed
                        }));
                    } else {
                        log_info!("⚠️ 返回空响应");
                        return Err(anyhow!("Empty response from API"));
                    }
                } else {
                    log_error!("❌ 无法读取响应体");
                    return Err(anyhow!("Failed to read response body"));
                }
            } else {
                log_error!("❌ API 返回错误状态: {}", status);
                if let Ok(error_text) = resp.text().await {
                    log_error!("错误详情: {}", error_text);
                    return Err(anyhow!("API error {}: {}", status, error_text));
                }
                return Err(anyhow!("API error: {}", status));
            }
        }
        Err(e) => {
            log_error!("❌ 请求失败: {}", e);
            return Err(anyhow!("Request failed: {}", e));
        }
    }
}
