//! 启动前准备 FishingBot 原始日志目录。

use std::fs;
use std::path::{Path, PathBuf};

use super::bot_logs::{bot_logs_dir, target_logs_dir};
use super::fs::remove_dir_all_if_exists;
use crate::config;
use crate::text;

/// 重建原始日志目录并指向丢弃路径；清理前警告意外文件。
pub(crate) fn reset_fishingbot_raw_logs_dir(bot_dir: &Path) -> PathBuf {
    let raw_game_logs = &config::Config::get().raw_game_logs;

    reset_raw_logs_dir(
        &target_logs_dir(&bot_logs_dir(bot_dir), &raw_game_logs.dir_name),
        &raw_game_logs.file_name,
        &raw_game_logs.discard_path,
    )
}

fn reset_raw_logs_dir(
    raw_logs_dir: &Path,
    game_log_file_name: &str,
    discard_path: &str,
) -> PathBuf {
    // 清空目录前记录意外文件，避免掩盖 JAR 日志命名规则的变化。
    for file_name in unexpected_game_log_file_names(raw_logs_dir, discard_path) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::UnexpectedGameLogFile {
                dir: &raw_logs_dir.display().to_string(),
                file_name: &file_name,
            })
        );
    }

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

    // JAR 需要可写的日志目标，符号链接将原始日志指向丢弃路径。
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

/// 返回非丢弃路径符号链接的文件名并排序；目录读取失败时返回空列表。
fn unexpected_game_log_file_names(raw_logs_dir: &Path, discard_path: &str) -> Vec<String> {
    let Ok(dir_reader) = fs::read_dir(raw_logs_dir) else {
        return Vec::new();
    };

    let mut unexpected_names: Vec<String> = dir_reader
        .flatten()
        .filter_map(|entry| {
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
mod tests {
    use super::*;
    use crate::test_support::temp_test_dir;

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

    #[test]
    fn unexpected_game_logs_are_reported_before_cleanup() {
        let bot_logs_dir = temp_test_dir("unexpected-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null").is_empty(),
            "正常情况下 raw/ 里只有指向 /dev/null 的 log0.log"
        );
        fs::write(game_logs_dir.join("log1.log"), "未过滤的刷屏行\n").expect("创建非预期文件");
        assert_eq!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null"),
            vec!["log1.log".to_string()]
        );

        let game_logs_dir_again = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            !game_logs_dir_again.join("log1.log").exists(),
            "启动清理必须删除非预期文件"
        );
        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

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
}
