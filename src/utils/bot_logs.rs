//! 机器人日志文件的创建、选择和读取；程序日志使用相同的时间戳命名规则。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::config;
use crate::text;

/// 日志文件名的本地时间格式。
const LOG_NAME_TIME_FORMAT: &str = "%Y-%m-%d-%H-%M-%S";

/// 构造日志目录路径，不检查目录是否存在。
pub fn target_logs_dir(base_dir: &Path, dir_name: &str) -> PathBuf {
    base_dir.join(dir_name)
}

/// 检查文件名是否符合时间戳日志格式。
fn is_bot_log_file(file_name: &str) -> bool {
    let Some(time_part) = file_name.strip_suffix(".log") else {
        return false;
    };

    let mut parts = time_part.split('-');
    for part_width in [4usize, 2, 2, 2, 2, 2] {
        let Some(part) = parts.next() else {
            return false;
        };
        if part.len() != part_width || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }

    parts.next().is_none()
}

/// 创建时间戳日志文件并写入首行；返回路径及位于首行之后的可写句柄。
pub fn create_timestamped_log_file(
    dir: &Path,
    first_line: &str,
) -> Result<(PathBuf, File), String> {
    fs::create_dir_all(dir).map_err(|e| {
        text::get().log_text(text::LogText::CreateLogDirFailed {
            dir: &dir.display().to_string(),
            reason: &e.to_string(),
        })
    })?;

    let log_path = dir.join(format!("{}.log", Local::now().format(LOG_NAME_TIME_FORMAT)));

    let mut file = File::create(&log_path).map_err(|e| {
        text::get().log_text(text::LogText::CreateLogFileFailed {
            path: &log_path.display().to_string(),
            reason: &e.to_string(),
        })
    })?;
    writeln!(file, "{first_line}").map_err(|e| {
        text::get().log_text(text::LogText::WriteLogFirstLineFailed {
            path: &log_path.display().to_string(),
            reason: &e.to_string(),
        })
    })?;

    Ok((log_path, file))
}

/// 根据配置获取机器人日志目录。
pub(crate) fn bot_logs_dir(bot_dir: &Path) -> PathBuf {
    target_logs_dir(bot_dir, &config::Config::get().runtime.bot_logs_dir)
}

/// 创建机器人日志文件；启动会话前调用，会话向该文件追加内容。
pub fn create_bot_log_file(bot_dir: &Path) -> Result<PathBuf, String> {
    let first_line = text::get().log_text(text::LogText::BotLogFirstLine);

    create_timestamped_log_file(&bot_logs_dir(bot_dir), &first_line)
        .map(|(log_path, _file)| log_path)
}

/// 返回合法时间戳日志中名称最大的文件路径。
pub fn latest_log_path(bot_dir: &Path) -> Option<PathBuf> {
    let logs_dir = bot_logs_dir(bot_dir);

    let candidate_file_names: Vec<String> = fs::read_dir(&logs_dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();

    let latest_file_name = latest_log_file_name(&candidate_file_names)?;

    Some(logs_dir.join(latest_file_name))
}

/// 返回最新的合法日志文件名；无候选时返回 `None`。
fn latest_log_file_name(candidate_file_names: &[String]) -> Option<&str> {
    candidate_file_names
        .iter()
        .filter(|file_name| is_bot_log_file(file_name))
        .max_by_key(|file_name| file_name.as_str())
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 日志目录位于机器人目录下的 `bot_logs_dir` 层；目录名由配置提供，此处直接传参验证。
    #[test]
    fn target_logs_dir_uses_the_configured_name() {
        let bot_dir = Path::new("/fishingbots/bots/123456/duck");

        assert_eq!(
            target_logs_dir(bot_dir, "logs"),
            Path::new("/fishingbots/bots/123456/duck/logs")
        );
        assert_eq!(
            target_logs_dir(bot_dir, "log"),
            Path::new("/fishingbots/bots/123456/duck/log")
        );
    }

    #[test]
    fn only_timestamped_bot_log_names_are_recognized() {
        assert!(is_bot_log_file("2026-09-28-01-47-01.log"));
        for name in [
            "log0.log",
            "log1.log",
            "log.log",
            "logs.log",
            "FishingBot.log",
            "2026-09-28-01-47-01.log.bak",
            "2026-09-28-01-47-01.logs",
            "2026_09_28-01-47-01.log",
            "2026-09-28-01-47-1.log",
            "20260928-01-47-01.log",
            "2026-09-28-01-47-.log",
        ] {
            assert!(!is_bot_log_file(name), "{name}");
        }
    }

    /// 按文件名中的定宽时间戳取最大值；跨天、跨月、跨年均由字典序保证。
    #[test]
    fn latest_log_is_picked_by_name_order() {
        let file_names = vec![
            "2026-09-28-01-47-01.log".to_string(),
            "2026-09-30-15-30-12.log".to_string(),
            "2026-09-29-23-59-59.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names),
            Some("2026-09-30-15-30-12.log")
        );

        // 跨月（09 → 10）与跨年（2026 → 2027）同样成立
        let file_names = vec![
            "2026-09-30-23-59-59.log".to_string(),
            "2026-10-01-00-00-00.log".to_string(),
            "2027-01-01-00-00-00.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names),
            Some("2027-01-01-00-00-00.log")
        );
    }

    #[test]
    fn non_own_log_names_are_never_picked() {
        assert_eq!(latest_log_file_name(&[]), None);
        let file_names = vec![
            "log0.log".to_string(),
            "log1.log".to_string(),
            "备份.log".to_string(),
            "2026-09-30-15-30-12.txt".to_string(),
        ];
        assert_eq!(latest_log_file_name(&file_names), None);

        let file_names = vec![
            "log9.log".to_string(),
            "2026-01-01-00-00-00.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names),
            Some("2026-01-01-00-00-00.log"),
            "合法文件名必须优先于 log9.log"
        );
    }
}
