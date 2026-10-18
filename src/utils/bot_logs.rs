//! 机器人日志文件：创建、挑选、读取（`<机器人目录>/<runtime.bot_logs_dir>` 下的 `时间戳.log`）。
//! 默认丢弃 FishingBot.jar 的原始日志。
//! "最新日志"的判定（[`latest_log_path`]）：在合法（以时间戳格式命名）的文件中按文件名取最大者。
//! 程序日志（`<部署目录>/logs/`）用同一套命名逻辑，但属于另一模块，见 `src/process_log.rs`。

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::config;
use crate::text;

/// 日志文件名里的时间戳格式：`2026-09-28-01-47-01`（机器本地时间）。
const LOG_NAME_TIME_FORMAT: &str = "%Y-%m-%d-%H-%M-%S";

/// 拼接出一个"预期目录"的路径；不检查它是否存在。
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

/// 创建以时间戳命名的日志文件，写入首行 `first_line`，返回路径与可写句柄。
/// 机器人日志（`create_bot_log_file`）与程序日志（`src/process_log.rs`）共用此函数，命名规则同此。
/// 返回句柄的写入位置在首行之后；父目录不存在时一并创建（`create_dir_all`）。
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

/// 由机器人文件夹路径获取机器人日志文件夹路径（目录名只在此处从配置读取一次）。
pub(crate) fn bot_logs_dir(bot_dir: &Path) -> PathBuf {
    target_logs_dir(bot_dir, &config::Config::get().runtime.bot_logs_dir)
}

/// 每次机器人运行流程开始前新建日志文件：`<机器人目录>/<runtime.bot_logs_dir>/时间戳.log`。
/// 它是启动流程的第一步（见 [`super::startup::StartupQueue`]）：先创建日志文件再启动会话，会话向该文件追加内容。
pub fn create_bot_log_file(bot_dir: &Path) -> Result<PathBuf, String> {
    let first_line = text::get().log_text(text::LogText::BotLogFirstLine);

    create_timestamped_log_file(&bot_logs_dir(bot_dir), &first_line)
        .map(|(log_path, _file)| log_path)
}

/// 挑选机器人最新一份日志文件的路径（`logs/` 下文件名最大者）。
/// 文件名最大即最新启动对应的日志；
/// 不符合本项目命名规则的文件不参与挑选。
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

/// 从一批候选文件名中挑出最新一份日志的文件名；`None` 表示没有任何合格候选。
fn latest_log_file_name(candidate_file_names: &[String]) -> Option<&str> {
    candidate_file_names
        .iter()
        .filter(|file_name| is_bot_log_file(file_name))
        .max_by_key(|file_name| file_name.as_str())
        .map(String::as_str)
}

// FishingBot 原始日志目录的准备（启动流程的第二步）。
//
// 整个目录删掉重建 `<机器人目录>/<runtime.bot_logs_dir>/raw`，并把它的原始日志指向丢弃路径。
// 清理前先报出"意料之外的文件"：静默删除会掩盖 FishingBot.jar 更名日志规则这一情况。
// 目录名与丢弃目标由调用方给出（[`reset_raw_logs_dir`] 不读配置，便于单测），
// 读取配置的是 [`reset_fishingbot_logs_dir`]。

use crate::utils::fs::remove_dir_all_if_exists;

/// 准备 FishingBot.jar 的日志目录（启动流程的第二步）：整个目录删掉重建，并把原始日志指向 `discard_path`。
/// 清理前先报出"意料之外的文件"，以免静默删除掩盖 FishingBot.jar 更名日志规则。
/// 意外只警告、不打断启动；最坏情况是 FishingBot.jar 因建不出日志文件而拒绝启动。
pub fn reset_fishingbot_logs_dir(bot_dir: &Path) -> PathBuf {
    let raw_game_logs = &config::Config::get().raw_game_logs;

    reset_raw_logs_dir(
        &target_logs_dir(&bot_logs_dir(bot_dir), &raw_game_logs.dir_name),
        &raw_game_logs.file_name,
        &raw_game_logs.discard_path,
    )
}

/// [`reset_fishingbot_logs_dir`] 的实现：`raw_logs_dir` 为 raw 目录本身，
/// 目录名与丢弃目标由调用方给出（拆出来是为了不读配置即可单测；读配置在 [`reset_fishingbot_logs_dir`] 中）。
fn reset_raw_logs_dir(
    raw_logs_dir: &Path,
    game_log_file_name: &str,
    discard_path: &str,
) -> PathBuf {
    // 第一步：先报出上一轮留下的"意料之外的文件"；随后整个目录会被清空。
    for file_name in unexpected_game_log_file_names(raw_logs_dir, discard_path) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::UnexpectedGameLogFile {
                dir: &raw_logs_dir.display().to_string(),
                file_name: &file_name,
            })
        );
    }

    // 第二步：整个目录删掉重建（目录原本不存在的情况由 `remove_dir_all_if_exists` 视为成功，无需另行判断存在性）。
    if let Err(e) = remove_dir_all_if_exists(raw_logs_dir) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogsDirRemoveFailed {
                dir: &raw_logs_dir.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }
    if let Err(e) = fs::create_dir_all(raw_logs_dir) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogsDirCreateFailed {
                dir: &raw_logs_dir.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }

    // 第三步：将预期的原始日志指向丢弃路径：FishingBot.jar 建不出日志文件会拒绝启动，因此给它一个"能写、但什么都不留"的目标。
    let game_log_file = raw_logs_dir.join(game_log_file_name);
    if let Err(e) = std::os::unix::fs::symlink(discard_path, &game_log_file) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogDiscardFailed {
                file: &game_log_file.display().to_string(),
                device: discard_path,
                reason: &e.to_string(),
            })
        );
    }

    raw_logs_dir.to_path_buf()
}

/// 列出 `raw/` 中不应存在的文件名（已排序）；目录读取失败时返回空数组。
/// 预期只有一个文件：`game_log_file_name` 指向 `discard_path` 的符号链接；
/// 其余文件均视为意外。[`reset_raw_logs_dir`] 在清空整个目录前先将其报出。
fn unexpected_game_log_file_names(raw_logs_dir: &Path, discard_path: &str) -> Vec<String> {
    let Ok(dir_reader) = fs::read_dir(raw_logs_dir) else {
        return Vec::new();
    };

    let mut unexpected_names: Vec<String> = dir_reader
        .flatten()
        .filter_map(|entry| {
            // 只有指向 `discard_path` 的符号链接才算预期文件。
            let is_discard_link =
                fs::read_link(entry.path()).is_ok_and(|target| target == Path::new(discard_path));
            if is_discard_link {
                return None;
            }

            Some(entry.file_name().to_str()?.to_string())
        })
        .collect();
    unexpected_names.sort();

    unexpected_names
}

#[cfg(test)]
mod raw_logs_dir_tests {
    use super::*;

    /// FishingBot 启动前必须准备好 raw 日志目录与日志文件；缺文件会拒绝启动。
    #[test]
    fn game_logs_dir_is_prepared() {
        let bot_logs_dir = temp_test_dir("prepare-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");

        assert_eq!(game_logs_dir, raw_logs_dir);
        assert!(game_logs_dir.is_dir());
        assert!(game_logs_dir.join("log0.log").exists(), "日志文件要能写");

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 启动清理必须报告 raw 目录中的非预期文件，随后清空目录。
    /// 正常情况下仅存在指向丢弃目标的 `log0.log`，不应产生告警。
    #[test]
    fn unexpected_game_logs_are_reported_before_cleanup() {
        let bot_logs_dir = temp_test_dir("unexpected-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null").is_empty(),
            "正常情况下 raw/ 里只有指向 /dev/null 的 log0.log"
        );

        // 模拟 FishingBot 更换日志文件名：创建一个有内容的普通文件。
        fs::write(game_logs_dir.join("log1.log"), "未过滤的刷屏行\n").expect("创建非预期文件");

        assert_eq!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null"),
            vec!["log1.log".to_string()]
        );

        // 再次执行启动清理：该文件必须被删除，仅保留过滤后的日志。
        let game_logs_dir_again = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            !game_logs_dir_again.join("log1.log").exists(),
            "启动清理必须删除非预期文件"
        );

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// FishingBot 日志文件是指向 `/dev/null` 的符号链接，写入内容必须被丢弃。
    /// 磁盘上仅保留本项目过滤后的日志。
    #[test]
    fn game_log_file_discards_writes() {
        let bot_logs_dir = temp_test_dir("discard-game-log");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_log_file =
            reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null").join("log0.log");
        assert_eq!(
            fs::read_link(&game_log_file).expect("应当是符号链接"),
            Path::new("/dev/null")
        );

        fs::write(&game_log_file, "未过滤的刷屏行\n").expect("写 FishingBot 的日志");
        assert_eq!(fs::metadata(&game_log_file).expect("看文件大小").len(), 0);

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 启动前清空并重建 raw 日志目录，不保留上一次运行的任何文件。
    #[test]
    fn resetting_game_logs_dir_clears_previous_run() {
        let bot_logs_dir = temp_test_dir("reset-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        fs::write(game_logs_dir.join("log1.log"), "上次运行残留\n").expect("创建残留文件");

        let game_logs_dir_again = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");

        assert_eq!(game_logs_dir_again, game_logs_dir);
        assert!(
            !game_logs_dir.join("log1.log").exists(),
            "上一次运行的文件不得保留"
        );
        assert!(game_logs_dir.join("log0.log").exists());

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 测试使用独立临时目录，避免并发冲突。
    use crate::test_support::temp_test_dir;
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

    /// `create_timestamped_log_file` 按 `LOG_NAME_TIME_FORMAT` 生成的文件名必须能被识别。
    #[test]
    fn own_log_names_are_recognized() {
        assert!(is_bot_log_file("2026-09-28-01-47-01.log"));
    }

    /// FishingBot 原始日志（例如 Java FileHandler 的 `log0.log`）不得被识别为本项目日志，
    /// 否则 `/log` 会读取未过滤的原始输出。
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

    /// 时间戳形状但格式不合法的文件名必须被拒绝。
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

    /// 不符合命名规则的文件不得参与“最新日志”选择。
    /// 例如 `log9.log` 的字典序高于 `2026-…`，若不过滤会被错误选中。
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
            "合法文件名必须优先于 log9.log"
        );
    }

    /// 没有候选文件时返回 `None`。
    #[test]
    fn no_candidate_means_no_latest_log() {
        assert_eq!(latest_log_file_name(&[]), None);
    }
}
