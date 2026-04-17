use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;

// 可配置的日志文件大小限制 (10MB，前端日志通常较小)
const MAX_WEBLOG_SIZE_MB: u64 = 10;
const MAX_WEBLOG_SIZE_BYTES: u64 = MAX_WEBLOG_SIZE_MB * 1024 * 1024;

// Web日志文件名
const WEBLOG_FILE_NAME: &str = "auto-cursor-web.log";

// 全局Web日志器实例
static WEBLOGGER: Mutex<Option<WebLogger>> = Mutex::new(None);

#[derive(Debug, Serialize, Deserialize)]
pub struct WebLogEntry {
    pub level: String,
    pub message: String,
    pub url: Option<String>,
    pub user_agent: Option<String>,
    pub stack: Option<String>,
    pub timestamp: String,
}

pub struct WebLogger {
    log_file_path: PathBuf,
}

impl WebLogger {
    /// 初始化Web日志器
    pub fn init() -> Result<()> {
        // 延迟初始化，只设置标记，实际初始化在第一次写日志时进行
        Ok(())
    }

    /// 延迟初始化Web日志器
    fn lazy_init() -> Result<()> {
        let log_file_path = Self::get_weblog_file_path()?;
        let logger = WebLogger { log_file_path };

        // 确保日志目录存在
        if let Some(parent) = logger.log_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut global_logger = WEBLOGGER.lock().unwrap();
        *global_logger = Some(logger);

        Ok(())
    }

    /// 获取Web日志文件路径
    fn get_weblog_file_path() -> Result<PathBuf> {
        // 优先尝试使用应用目录，如果失败则使用系统临时目录
        let log_dir = match crate::get_app_dir() {
            Ok(app_dir) => app_dir.join("logs"),
            Err(_) => {
                // 如果应用目录不可用，使用系统临时目录
                std::env::temp_dir().join("auto-cursor-logs")
            }
        };

        Ok(log_dir.join(WEBLOG_FILE_NAME))
    }

    /// 写入Web日志
    pub fn write_weblog(entry: &WebLogEntry) {
        // 尝试获取已初始化的logger
        if let Ok(logger_guard) = WEBLOGGER.lock() {
            if let Some(logger) = logger_guard.as_ref() {
                if let Err(e) = logger.write_weblog_internal(entry) {
                    eprintln!("Failed to write weblog: {}", e);
                }
                return;
            }
        }

        // 如果logger未初始化，尝试延迟初始化
        if let Err(_) = Self::lazy_init() {
            // 初始化失败，回退到控制台输出
            println!(
                "[WEBLOG] [{}] [{}] {}",
                entry.timestamp, entry.level, entry.message
            );
            return;
        }

        // 初始化成功后再次尝试写入
        if let Ok(logger_guard) = WEBLOGGER.lock() {
            if let Some(logger) = logger_guard.as_ref() {
                if let Err(e) = logger.write_weblog_internal(entry) {
                    eprintln!("Failed to write weblog: {}", e);
                }
            }
        }
    }

    /// 便捷方法：写入简单的Web日志
    pub fn write_simple_weblog(level: &str, message: &str, url: Option<String>) {
        let entry = WebLogEntry {
            level: level.to_string(),
            message: message.to_string(),
            url,
            user_agent: None,
            stack: None,
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        };
        Self::write_weblog(&entry);
    }

    /// 内部写入Web日志方法
    fn write_weblog_internal(&self, entry: &WebLogEntry) -> Result<()> {
        // 检查文件大小并清理
        self.check_and_cleanup_weblog_file()?;

        // 格式化日志条目
        let log_line = if let Some(stack) = &entry.stack {
            format!(
                "[{}] [{}] {} | URL: {} | Stack: {}",
                entry.timestamp,
                entry.level,
                entry.message,
                entry.url.as_deref().unwrap_or("N/A"),
                stack
            )
        } else {
            format!(
                "[{}] [{}] {} | URL: {}",
                entry.timestamp,
                entry.level,
                entry.message,
                entry.url.as_deref().unwrap_or("N/A")
            )
        };

        // 同时输出到控制台和文件
        println!("[WEBLOG] {}", log_line);

        // 追加写入日志文件
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)?;

        file.write_all(format!("{}\n", log_line).as_bytes())?;
        file.flush()?;

        Ok(())
    }

    /// 检查并清理Web日志文件
    fn check_and_cleanup_weblog_file(&self) -> Result<()> {
        if !self.log_file_path.exists() {
            return Ok(());
        }

        let file_size = std::fs::metadata(&self.log_file_path)?.len();

        if file_size > MAX_WEBLOG_SIZE_BYTES {
            self.trim_weblog_file()?;
        }

        Ok(())
    }

    /// 裁剪Web日志文件，保留后半部分
    fn trim_weblog_file(&self) -> Result<()> {
        let temp_path = self.log_file_path.with_extension("tmp");

        {
            let input_file = File::open(&self.log_file_path)?;
            let reader = BufReader::new(input_file);
            let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

            // 保留后70%的日志行（前端日志相对重要）
            let keep_lines = (lines.len() as f64 * 0.7) as usize;
            let lines_to_keep = if keep_lines > 0 && keep_lines < lines.len() {
                &lines[lines.len() - keep_lines..]
            } else {
                &lines[lines.len() / 2..] // 至少保留一半
            };

            let mut temp_file = File::create(&temp_path)?;

            // 写入保留的行
            for line in lines_to_keep {
                writeln!(temp_file, "{}", line)?;
            }

            temp_file.flush()?;
        }

        // 替换原文件
        std::fs::rename(&temp_path, &self.log_file_path)?;

        // 记录清理操作
        let cleanup_entry = WebLogEntry {
            level: "INFO".to_string(),
            message: format!(
                "Web log file trimmed, kept approximately {}% of original content",
                70
            ),
            url: None,
            user_agent: None,
            stack: None,
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        };
        self.write_weblog_internal(&cleanup_entry)?;

        Ok(())
    }

    /// 获取Web日志文件路径（用于外部访问）
    pub fn get_weblog_path() -> Option<PathBuf> {
        if let Ok(logger_guard) = WEBLOGGER.lock() {
            if let Some(logger) = logger_guard.as_ref() {
                return Some(logger.log_file_path.clone());
            }
        }
        None
    }

    /// 读取最近的Web日志条目
    pub fn get_recent_weblogs(limit: usize) -> Result<Vec<String>> {
        let log_path =
            Self::get_weblog_path().ok_or_else(|| anyhow::anyhow!("Web logger not initialized"))?;

        if !log_path.exists() {
            return Ok(vec![]);
        }

        let file = File::open(&log_path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

        let start_index = if lines.len() > limit {
            lines.len() - limit
        } else {
            0
        };

        Ok(lines[start_index..].to_vec())
    }
}

// 便捷的Web日志宏
#[macro_export]
macro_rules! weblog_info {
    ($($arg:tt)*) => {
        crate::weblog::WebLogger::write_simple_weblog("INFO", &format!($($arg)*), None)
    };
}

#[macro_export]
macro_rules! weblog_debug {
    ($($arg:tt)*) => {
        crate::weblog::WebLogger::write_simple_weblog("DEBUG", &format!($($arg)*), None)
    };
}

#[macro_export]
macro_rules! weblog_warn {
    ($($arg:tt)*) => {
        crate::weblog::WebLogger::write_simple_weblog("WARN", &format!($($arg)*), None)
    };
}

#[macro_export]
macro_rules! weblog_error {
    ($($arg:tt)*) => {
        crate::weblog::WebLogger::write_simple_weblog("ERROR", &format!($($arg)*), None)
    };
}

// 获取Web日志配置
pub fn get_weblog_config() -> (u64, &'static str) {
    (MAX_WEBLOG_SIZE_MB, WEBLOG_FILE_NAME)
}
