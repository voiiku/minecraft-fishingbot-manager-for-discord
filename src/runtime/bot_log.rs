//! 创建、选择机器人日志并准备 FishingBot 原始日志目录。
use crate::config;
use crate::runtime::log_files::create_timestamped_log_file;
#[cfg(test)]
use crate::runtime::utils::bot_log::bot_log_time;
use crate::runtime::utils::bot_log::latest_log_file_name;
use crate::utils::fs::remove_dir_all_if_exists;
use std::fs;
use std::path::{Path, PathBuf};

/// 根据配置获取机器人日志目录。
pub(crate) fn bot_logs_dir(bot_dir: &Path) -> PathBuf {
    bot_dir.join(&config::Config::get().runtime.bot_logs_dir)
}

/// 创建机器人日志文件；启动会话前调用，会话向该文件追加内容。
pub(crate) fn create_bot_log_file(bot_dir: &Path) -> Result<PathBuf, String> {
    let first_line = crate::config::Config::get().text().bot_log_first_line();

    create_timestamped_log_file(&bot_logs_dir(bot_dir), &first_line)
        .map(|(log_path, _file)| log_path)
}

/// 重建原始日志目录并指向丢弃路径；清理前警告意外文件。
pub(crate) fn reset_fishingbot_raw_logs_dir(bot_dir: &Path) -> PathBuf {
    let raw_game_logs = &config::Config::get().raw_game_logs;

    reset_raw_logs_dir(
        &bot_logs_dir(bot_dir).join(&raw_game_logs.dir_name),
        &raw_game_logs.file_name,
        &raw_game_logs.discard_path,
    )
}

fn reset_raw_logs_dir(
    raw_logs_dir: &Path,
    game_log_file_name: &str,
    discard_path: &str,
) -> PathBuf {
    for file_name in unexpected_game_log_file_names(raw_logs_dir, discard_path) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .unexpected_game_log_file(&raw_logs_dir.display().to_string(), &file_name)
        );
    }

    if let Err(e) = remove_dir_all_if_exists(raw_logs_dir) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .game_logs_dir_remove_failed(&raw_logs_dir.display().to_string(), &e.to_string())
        );
    }
    if let Err(e) = fs::create_dir_all(raw_logs_dir) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .game_logs_dir_create_failed(&raw_logs_dir.display().to_string(), &e.to_string())
        );
    }

    let game_log_file = raw_logs_dir.join(game_log_file_name);
    if let Err(e) = std::os::unix::fs::symlink(discard_path, &game_log_file) {
        log::warn!(
            "{}",
            crate::config::Config::get().text().game_log_discard_failed(
                &game_log_file.display().to_string(),
                discard_path,
                &e.to_string()
            )
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

/// 返回合法日志中时间戳最新的文件路径。
pub fn latest_log_path(bot_dir: &Path) -> Option<PathBuf> {
    let logs_dir = bot_logs_dir(bot_dir);

    let candidate_file_names: Vec<String> = fs::read_dir(&logs_dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();

    let latest_file_name = latest_log_file_name(
        &candidate_file_names,
        config::Config::get().constants.log_filename_time_format,
    )?;

    Some(logs_dir.join(latest_file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_test_dir;

    #[test]
    fn only_timestamped_bot_log_names_are_recognized() {
        let format = "%Y-%m-%d-%H-%M-%S";
        assert!(bot_log_time("2026-09-28-01-47-01.log", format).is_some());
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
            assert!(bot_log_time(name, format).is_none(), "{name}");
        }
    }

    /// 按时间戳选择最新日志，跨天、跨月、跨年均适用。
    #[test]
    fn latest_log_is_picked_by_name_order() {
        let file_names = vec![
            "2026-09-28-01-47-01.log".to_string(),
            "2026-09-30-15-30-12.log".to_string(),
            "2026-09-29-23-59-59.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names, "%Y-%m-%d-%H-%M-%S"),
            Some("2026-09-30-15-30-12.log")
        );

        // 跨月（09 → 10）与跨年（2026 → 2027）同样成立。
        let file_names = vec![
            "2026-09-30-23-59-59.log".to_string(),
            "2026-10-01-00-00-00.log".to_string(),
            "2027-01-01-00-00-00.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names, "%Y-%m-%d-%H-%M-%S"),
            Some("2027-01-01-00-00-00.log")
        );
    }

    #[test]
    fn non_own_log_names_are_never_picked() {
        assert_eq!(latest_log_file_name(&[], "%Y-%m-%d-%H-%M-%S"), None);
        let file_names = vec![
            "log0.log".to_string(),
            "log1.log".to_string(),
            "备份.log".to_string(),
            "2026-09-30-15-30-12.txt".to_string(),
        ];
        assert_eq!(latest_log_file_name(&file_names, "%Y-%m-%d-%H-%M-%S"), None);

        let file_names = vec![
            "log9.log".to_string(),
            "2026-01-01-00-00-00.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&file_names, "%Y-%m-%d-%H-%M-%S"),
            Some("2026-01-01-00-00-00.log"),
            "合法文件名必须优先于 log9.log"
        );
    }

    #[test]
    fn latest_log_uses_the_configured_timestamp_format() {
        let names = vec![
            "31-12-2025_23-59-59.log".to_string(),
            "01-01-2026_00-00-00.log".to_string(),
        ];
        assert_eq!(
            latest_log_file_name(&names, "%d-%m-%Y_%H-%M-%S"),
            Some("01-01-2026_00-00-00.log")
        );
    }

    #[test]
    fn game_logs_dir_is_prepared() {
        let bot_logs_dir = temp_test_dir("prepare-game-logs");
        let raw_logs_dir = bot_logs_dir.join("raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");

        assert_eq!(game_logs_dir, raw_logs_dir);
        assert!(game_logs_dir.is_dir());
        assert!(game_logs_dir.join("log0.log").exists(), "日志文件要能写");

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    #[test]
    fn unexpected_game_logs_are_reported_before_cleanup() {
        let bot_logs_dir = temp_test_dir("unexpected-game-logs");
        let raw_logs_dir = bot_logs_dir.join("raw");

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
        let raw_logs_dir = bot_logs_dir.join("raw");
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
        let raw_logs_dir = bot_logs_dir.join("raw");
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
