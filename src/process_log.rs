//! 程序与框架日志同时写入终端和 `<部署目录>/logs/时间戳.log`。
//! 日志文件不可用时将只写终端；级别由 `RUST_LOG` 或 `[log] debug` 决定。

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::config;
use crate::text;
use crate::utils;

/// 程序日志目录名。
const LOG_DIR_NAME: &str = "logs";

/// 日志时间戳格式。
const LOG_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// 进程级 logger。
static PROCESS_LOGGER: OnceLock<ProcessLog> = OnceLock::new();

/// 初始化程序日志，由[`main`]调用。
pub fn init_process_log(debug: bool) {
    // 日志文件创建失败不影响程序运行，告警等 logger 安装后输出。
    let mut warnings: Vec<String> = Vec::new();
    let (log_path, log_file) = match create_process_log_file() {
        Ok((log_path, log_file)) => (Some(log_path), Some(log_file)),
        Err(message) => {
            warnings.push(message);
            (None, None)
        }
    };

    // 安装 logger 并设置级别。
    let rust_log = std::env::var("RUST_LOG").ok();
    let (levels, level_warning) = effective_log_levels(debug, rust_log.as_deref());
    warnings.extend(level_warning);
    install_logger(log_file, levels);

    // 输出初始化结果。
    for warning in warnings {
        log::warn!("{warning}");
        // 全局级别过滤 WARN 时仍将文件创建告警写到终端。
        if log::max_level() < LevelFilter::Warn {
            eprintln!("{warning}");
        }
    }
    if let Some(log_path) = log_path {
        log::info!(
            "{}",
            text::get().log_text(text::LogText::ProcessLogPath {
                path: &log_path.display().to_string(),
            })
        );
    }

    // 记录配置路径与进程号。
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
}

/// 创建程序日志文件。
fn create_process_log_file() -> Result<(PathBuf, File), String> {
    let process_logs_dir = config::Config::install_dir()?.join(LOG_DIR_NAME);
    // 首行使用当前语言。
    let first_line = text::get().log_text(text::LogText::ProcessLogFirstLine);

    utils::create_timestamped_log_file(&process_logs_dir, &first_line)
}

/// 安装 logger 并设置级别，重复安装失败时输出告警。
fn install_logger(log_file: Option<File>, levels: LogLevels) {
    let logger = PROCESS_LOGGER.get_or_init(|| ProcessLog {
        levels,
        file: log_file.map(Mutex::new),
    });

    match log::set_logger(logger) {
        // 全局上限：两个 target 前缀各有各的上限，取更宽的那个才不会把任何一方的记录压掉。
        Ok(()) => log::set_max_level(levels.own.max(levels.dependencies)),
        Err(e) => println!(
            "{}",
            text::get().log_text(text::LogText::ProcessLogInstallFailed {
                reason: &e.to_string(),
            })
        ),
    }
}

/// 本项目与第三方依赖的日志级别上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogLevels {
    /// 本项目（`minecraft_fishingbot_manager_for_discord`）记录的上限。
    own: LevelFilter,
    /// 第三方 crate 记录的上限。
    dependencies: LevelFilter,
}

impl LogLevels {
    /// 根据日志 target 选择级别上限。
    fn ceiling_for(&self, target: &str) -> LevelFilter {
        if target.starts_with(OWN_TARGET_PREFIX) {
            self.own
        } else {
            self.dependencies
        }
    }
}

/// 本 crate 的日志 target 前缀。
const OWN_TARGET_PREFIX: &str = env!("CARGO_CRATE_NAME");

/// 按 `RUST_LOG` 或 `[log] debug` 计算日志级别。
fn effective_log_levels(debug: bool, rust_log: Option<&str>) -> (LogLevels, Option<String>) {
    let own_fallback = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let dependency_fallback = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    };
    let by_config = LogLevels {
        own: own_fallback,
        dependencies: dependency_fallback,
    };

    let Some(raw) = rust_log.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return (by_config, None);
    };

    match raw.parse::<LevelFilter>() {
        Ok(level) => (
            LogLevels {
                own: level,
                dependencies: level,
            },
            None,
        ),
        Err(_) => {
            let warning = text::get().log_text(text::LogText::RustLogInvalidLevel {
                raw,
                fallback: &own_fallback.to_string(),
            });

            (by_config, Some(warning))
        }
    }
}

/// 日志续行缩进。
const CONTINUATION_INDENT: &str = "  ";

/// 渲染一条日志记录；续行缩进两个空格。
fn format_record(timestamp: &str, level: Level, message: &str) -> String {
    let prefix = format!("[{timestamp}] [{level}] ");
    let mut rendered = String::new();

    if message.is_empty() {
        // 空消息仍保留时间与级别。
        rendered.push_str(&prefix);
    } else {
        for (index, chunk) in message.lines().enumerate() {
            if index > 0 {
                rendered.push('\n');
            }
            if index == 0 {
                rendered.push_str(&prefix);
            } else {
                rendered.push_str(CONTINUATION_INDENT);
            }
            rendered.push_str(chunk);
        }
    }
    rendered.push('\n');

    rendered
}

/// 将日志写入终端与文件。
struct ProcessLog {
    /// 日志级别上限。
    levels: LogLevels,
    /// 日志文件句柄。
    file: Option<Mutex<File>>,
}

impl ProcessLog {
    /// 写出一行,失败不影响其他。
    fn write_line(&self, line: &str) {
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(line.as_bytes());
        let _ = stderr.flush();

        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            // 每行写入后立即刷新。
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

impl Log for ProcessLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.levels.ceiling_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let line = format_record(
            &Local::now().format(LOG_TIME_FORMAT).to_string(),
            record.level(),
            &record.args().to_string(),
        );
        self.write_line(&line);
    }

    fn flush(&self) {
        if let Some(file) = &self.file
            && let Ok(mut file) = file.lock()
        {
            let _ = file.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单行记录：`[时间] [级别] 正文` + 一个换行。
    #[test]
    fn single_line_record_gets_one_prefix() {
        assert_eq!(
            format_record("2026-09-30 15:30:12", Level::Info, "已加载配置"),
            "[2026-09-30 15:30:12] [INFO] 已加载配置\n"
        );
    }

    /// 多行记录（`log::info!("{}", report)` 打一整块 `Report`）：每行都在，
    /// 续行缩进两格，这样第二行起不会"裸奔"，翻日志时仍能看出时间与级别。
    #[test]
    fn multi_line_record_indents_continuation_lines() {
        assert_eq!(
            format_record("T", Level::Warn, "标题:\n- 一行\n- 二行"),
            "[T] [WARN] 标题:\n  - 一行\n  - 二行\n"
        );
    }

    /// 空消息也要留住"时间 + 级别"，否则日志里会出现来历不明的空行。
    #[test]
    fn empty_record_keeps_the_prefix() {
        assert_eq!(format_record("T", Level::Error, ""), "[T] [ERROR] \n");
    }

    /// 正文里的 CRLF 不该把 `\r` 带进日志文件（`lines()` 已经帮我们去掉了）。
    #[test]
    fn carriage_returns_are_dropped() {
        assert_eq!(
            format_record("T", Level::Info, "a\r\nb"),
            "[T] [INFO] a\n  b\n"
        );
    }

    /// 没给 `RUST_LOG` 时：本项目自己的记录按配置放，第三方 crate 只放到 `WARN`
    /// （否则 serenity 的 `recv; do_heartbeat;` 会把本项目的关键几行淹没）。
    #[test]
    fn third_party_records_are_capped_to_warn_by_default() {
        let (levels, warning) = effective_log_levels(false, None);

        assert_eq!(warning, None);
        assert_eq!(levels.own, LevelFilter::Info);
        assert_eq!(levels.dependencies, LevelFilter::Warn);

        // "是不是自己的记录"只看 target 前缀：lib 与 bin 都是 `minecraft_fishingbot_manager_for_discord`。
        assert_eq!(
            levels.ceiling_for("minecraft_fishingbot_manager_for_discord"),
            LevelFilter::Info
        );
        assert_eq!(
            levels.ceiling_for("minecraft_fishingbot_manager_for_discord::utils::bot_status"),
            LevelFilter::Info
        );
        assert_eq!(
            levels.ceiling_for("serenity::gateway::shard"),
            LevelFilter::Warn
        );
        assert_eq!(levels.ceiling_for("poise::framework"), LevelFilter::Warn);
    }

    /// `debug = true` 时两档都放到 `DEBUG`：本项目自己的记录 + 第三方 crate 的
    ///（"打开 debug 就是全都要"）。默认（`false`）只放宽自己那一档，第三方仍只到 `WARN`。
    #[test]
    fn debug_widens_both_tiers() {
        let (levels, _) = effective_log_levels(true, None);

        assert_eq!(levels.own, LevelFilter::Debug);
        assert_eq!(levels.dependencies, LevelFilter::Debug);
        assert_eq!(
            levels.ceiling_for("serenity::gateway::shard"),
            LevelFilter::Debug
        );
        assert_eq!(levels.ceiling_for("poise::framework"), LevelFilter::Debug);
    }

    /// 显式给了 `RUST_LOG` 就两档照办，排查框架问题时要能看见 serenity 的话。
    #[test]
    fn rust_log_overrides_both_levels() {
        let (levels, warning) = effective_log_levels(false, Some("trace"));

        assert_eq!(warning, None);
        assert_eq!(levels.own, LevelFilter::Trace);
        assert_eq!(levels.dependencies, LevelFilter::Trace);

        // 只有空白的值等同于没设置。
        let (levels, warning) = effective_log_levels(false, Some("   "));
        assert_eq!(warning, None);
        assert_eq!(levels.own, LevelFilter::Info);
        assert_eq!(levels.dependencies, LevelFilter::Warn);
    }

    /// `RUST_LOG` 写错时回落成配置那档，并把告警带出去（`init_process_log` 会写进日志）。
    #[test]
    fn invalid_rust_log_falls_back_with_a_warning() {
        let (levels, warning) = effective_log_levels(false, Some("verbose"));

        assert_eq!(levels.own, LevelFilter::Info);
        assert_eq!(levels.dependencies, LevelFilter::Warn);

        let warning = warning.expect("写错时应当有告警");
        assert!(warning.contains("verbose"), "告警应带上写错的值: {warning}");
    }
}
