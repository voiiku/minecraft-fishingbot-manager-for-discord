//! 程序与框架日志同时写入终端和 `<部署目录>/logs/时间戳.log`。
//! 日志文件不可用时将只写终端；级别由配置里的 `[log] debug` 决定。
//! 级别判定在 `process_log/levels.rs`，记录渲染在 `process_log/format.rs`（都是纯函数，各自带测试）。

mod format;
mod levels;

use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use log::{Log, Metadata, Record};

use crate::config;
use crate::text;
use crate::utils;

use format::{format_record, timestamp};
use levels::LogLevels;

/// 程序日志目录名。
const LOG_DIR_NAME: &str = "logs";

/// 进程级 logger。
static PROCESS_LOGGER: OnceLock<ProcessLog> = OnceLock::new();

/// 程序日志：终端（stderr）与文件各一份，`impl log::Log`。
///
/// 进程内唯一的那一份由 [`ProcessLog::get_or_init`] 取（首次调用建日志文件并把它装上）。
pub struct ProcessLog {
    /// 日志级别上限。
    levels: LogLevels,
    /// 日志文件句柄（建不出来时为 `None`，那时只写终端）。
    file: Option<Mutex<File>>,
}

impl ProcessLog {
    /// 获取进程的 logger
    /// 当首次调用时就创建本次的日志文件，安装到 `log` 框架上，设置级别，记录启动信息。
    ///
    /// 必须在配置文件加载之后调用（需要读debug字段）
    /// 失败（磁盘写满、文件被删）时不该演变成游戏账号上不了线。
    pub fn get_or_init() -> &'static Self {
        // 如果已经存在就获取。
        if let Some(logger) = PROCESS_LOGGER.get() {
            println!(
                "{}",
                text::get().log_text(text::LogText::ProcessLogAlreadyInstalled)
            );
            return logger;
        }
        // 否则就创建文件
        // 创建失败不影响程序运行，告警等 logger 装好后输出。
        let first_line = text::get().log_text(text::LogText::ProcessLogFirstLine);
        let mut file_warning: Option<String> = None;
        let (log_path, log_file) = match config::Config::install_dir().and_then(|dir| {
            utils::create_timestamped_log_file(&dir.join(LOG_DIR_NAME), &first_line)
        }) {
            Ok((log_path, log_file)) => (Some(log_path), Some(log_file)),
            Err(message) => {
                file_warning = Some(message);
                (None, None)
            }
        };

        // 安装到 `log` 框架，设置全局级别：级别只由配置里的 `[log] debug` 决定。
        let levels = LogLevels::from_config();
        // 初始化全局的PROCESS_LOGGER
        let logger = PROCESS_LOGGER.get_or_init(|| Self {
            levels,
            file: log_file.map(Mutex::new),
        });

        match log::set_logger(logger) {
            // 设置日志等级上限，取两者（本程序自身日志和框架日志）最宽的。
            Ok(()) => log::set_max_level(levels.own.max(levels.dependencies)),
            // 走到这里说明 `log` 的全局位置已经存在了，是代码逻辑错误。
            Err(e) => println!(
                "{}",
                text::get().log_text(text::LogText::ProcessLogInstallFailed {
                    reason: &e.to_string(),
                })
            ),
        }

        // 输出结果：警告、日志位置，配置已加载文本和进程ID。
        if let Some(warning) = file_warning {
            log::warn!("{warning}");
        }
        if let Some(log_path) = log_path {
            log::info!(
                "{}",
                text::get().log_text(text::LogText::ProcessLogPath {
                    path: &log_path.display().to_string(),
                })
            );
        }
        log::info!(
            "{}",
            text::get().log_text(text::LogText::ConfigLoaded {
                path: &config::Config::get().config_file_path.display().to_string(),
            })
        );
        log::info!(
            "{}",
            text::get().log_text(text::LogText::ProcessId {
                pid: std::process::id(),
            })
        );

        logger
    }
}

impl Log for ProcessLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.levels.ceiling_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        // 如果自身没启用就直接返回
        if !self.enabled(record.metadata()) {
            return;
        }
        // 格式化单行日志；空消息被丢弃（`format_record` 返回 `None`），不写终端也不落文件。
        let Some(line) = format_record(&timestamp(), record.level(), &record.args().to_string())
        else {
            return;
        };

        // 终端与文件各一份，失败不影响其他。
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(line.as_bytes());
        let _ = stderr.flush();
        // 如果已经持有句柄，且能够持有日志文件的锁，就写入
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            // 每行写入后立即刷新。
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            let _ = file.flush();
        }
    }
}
