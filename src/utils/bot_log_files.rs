//! 机器人日志文件：创建、挑选、读取（`<机器人目录>/<runtime.logs_dir>` 下的 `时间戳.log`）。
//! 对 FishingBot.jar 自己那份原始日志的处置（`<机器人目录>/<runtime.logs_dir>/raw/`，内容指向 `/dev/null`）在 [`super::startup`] ，因为那是启动流程的一步。
//!
//! "最新日志"的标准（[`latest_log_path`]）：在合法（以时间戳格式命名）的文件里按文件名取最大。
//! 程序日志（`<部署目录>/logs/`）用同一套命名逻辑，但属于另一模块，见 `src/process_log.rs`。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::config;
use crate::text;

/// 日志文件名里的时间戳格式：`2026-09-28-01-47-01`（机器本地时间）。
const LOG_NAME_TIME_FORMAT: &str = "%Y-%m-%d-%H-%M-%S";

/// 通用的拼路径工具，拼出一个"预期文件夹"的路径
pub fn target_logs_dir(base_dir: &Path, dir_name: &str) -> PathBuf {
    base_dir.join(dir_name)
}

/// 判断一个文件名是不是本项目写的机器人日志（形如 `2026-09-28-01-47-01.log`）。
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

    // 多出其他段（例如 `2026-09-28-01-47-01-99.log`）同样非法。
    parts.next().is_none()
}

/// 本项目内日志命名的唯一规范：在 `dir` 下新建一份时间戳命名（本地时间 `年-月-日-时-分-秒.log`）的日志文件，并写入首行 `first_line`，返回路径与可写句柄。
/// 机器人日志（[`create_bot_log_file`]）与程序日志（`src/process_log.rs`）共用它，
/// 句柄的写入位置在首行之后，拿到的调用方可以接着往后写（程序日志就是这么用的）；
/// 机器人日志由会话里的 `tee -a` 往里追加，用不到句柄，直接丢弃。
/// 父目录不存在会一并创建（`create_dir_all`）。
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

/// 由机器人文件夹路径，获取机器人日志文件夹路径（日志目录名只在这里从配置读一次）。
pub(crate) fn bot_logs_dir(bot_dir: &Path) -> PathBuf {
    target_logs_dir(bot_dir, &config::get().runtime.logs_dir)
}

/// 在开始一次机器人运行流程前新建日志文件：`<机器人目录>/<runtime.logs_dir>/时间戳.log`。
/// 这是启动流程的第一步（见 [`super::startup::start_bot_sessions`]）：先建立日志文件再拉起会话，会话只负责向日志内追加内容。
pub fn create_bot_log_file(bot_dir: &Path) -> Result<PathBuf, String> {
    let first_line = text::get().log_text(text::LogText::BotLogFirstLine);

    create_timestamped_log_file(&bot_logs_dir(bot_dir), &first_line)
        .map(|(log_path, _file)| log_path)
}

/// 挑选机器人最新的一份日志文件的路径（`logs/` 下名字最大的那份日志）。
/// "名字最大"= 最新启动对应的日志；
/// 文件名不符合本项目命名规则的文件不参与挑选。
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

/// 从一批候选文件名里挑出最新的一份日志的文件名；`None` 表示一个合格的候选文件都没有。
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

    /// 日志目录就在机器人目录下的 `logs_dir` 那一层（名字由配置给，这里直接传参对照）。
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

    /// 本项目写的日志（[`create_timestamped_log_file`] 按 `LOG_NAME_TIME_FORMAT` 命名）要能认出来。
    #[test]
    fn own_log_names_are_recognized() {
        assert!(is_bot_log_file("2026-09-28-01-47-01.log"));
    }

    /// FishingBot 自己写的原始日志（Java FileHandler 的 `log0.log` 等）不能被当成我们的日志：
    /// 认错了 `/log` 就会把没过滤的内容（`+24.50XP` 这类刷屏行）读回来。
    #[test]
    fn game_log_names_are_rejected() {
        let names = [
            "log0.log",
            "log1.log",
            "log.log",
            "logs.log",
            "FishingBot.log",
        ];

        for name in names {
            assert!(!is_bot_log_file(name), "{name} 不该被当成我们的日志");
        }
    }

    /// 长得像时间戳但格式不对（长度、分隔符位置、后缀有出入）的也要拒绝。
    #[test]
    fn malformed_timestamp_names_are_rejected() {
        let names = [
            "2026-09-28-01-47-01.log.bak",
            "2026-09-28-01-47-01.logs",
            "2026_09_28-01-47-01.log",
            "2026-09-28-01-47-1.log",
            "20260928-01-47-01.log",
            "2026-09-28-01-47-.log",
        ];

        for name in names {
            assert!(!is_bot_log_file(name), "{name} 不该被当成我们的日志");
        }
    }

    /// 建出来的文件必须能被 [`is_bot_log_file`] 认出来，而且首行与句柄都能用。
    ///
    /// 命名这件事现在由 Rust 生成、又被 `/log` 的挑选口径校验，两边任一处改了写法，
    /// `/log` 就会突然报"还没有日志"：用这个测试把两者钉在一起。
    #[test]
    fn created_log_file_matches_own_name_rule() {
        let dir = temp_test_dir("create-log-file");
        let first_line = "--- 测试首行 ---";

        let (log_path, mut file) =
            create_timestamped_log_file(&dir, first_line).expect("应当能建出日志文件");

        let file_name = log_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("日志文件名应当能转成字符串");
        assert!(is_bot_log_file(file_name), "{file_name} 应当被认出来");
        assert_eq!(
            fs::read_to_string(&log_path).expect("读回日志"),
            format!("{first_line}\n")
        );

        // 句柄的写入位置就在首行之后：接着写就是追加，不会截掉首行。
        writeln!(file, "测试追加").expect("写日志");
        assert_eq!(
            fs::read_to_string(&log_path).expect("读回日志"),
            format!("{first_line}\n测试追加\n")
        );

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 每个测试用一个独立的临时目录（带进程号，避免并发跑测试时互相踩）。
    use crate::test_support::temp_test_dir;

    /// 挑"最新一份日志"按**名字**取最大：名字是定宽时间戳，字典序就是时间序（跨天、跨月、跨年都对）。
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

    /// 不在命名规则里的文件**永远不参与挑选** —— 这一步不能省：
    /// `log9.log` 在字典序里排在 `2026-…` 前面（`'l' > '2'`），没有过滤它就会"赢"。
    #[test]
    fn non_own_log_names_are_never_picked() {
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
            "合格的名字必须赢过 log9.log"
        );
    }

    /// 一个候选都没有时返回 `None`（`/log` 会回「还没有日志」）。
    #[test]
    fn no_candidate_means_no_latest_log() {
        assert_eq!(latest_log_file_name(&[]), None);
    }
}
