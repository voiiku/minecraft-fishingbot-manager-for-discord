//! 公开运行时工具函数的集成测试。

mod common;

use std::fs;
use std::io::Write;

use minecraft_fishingbot_manager_for_discord::runtime::{
    bot_instances::bot_dirs::BotDirEntry,
    log_files::{create_timestamped_log_file, read_last_lines},
};
use minecraft_fishingbot_manager_for_discord::utils::{
    bot_name::{BotNameError, ToCleanBotName},
    console_command::ToCleanBotCommand,
    fs::remove_dir_all_if_exists,
    uptime::{format_uptime, process_elapsed_seconds},
};

#[test]
fn bot_names_are_normalized_and_accept_supported_characters() {
    for (input, expected) in [("  Duck_02 ", "duck_02"), ("duck-02_x", "duck-02_x")] {
        assert_eq!(input.to_clean_bot_name(), Ok(expected.to_string()));
        assert_eq!(
            input.to_string().to_clean_bot_name(),
            Ok(expected.to_string())
        );
    }
}

#[test]
fn bot_name_rejects_illegal_characters() {
    if !common::in_configured_child("bot_name_rejects_illegal_characters") {
        return;
    }
    for (raw_name, illegal_char) in [
        ("中文名字", '中'),
        ("duck 02", ' '),
        ("duck.02", '.'),
        ("../../etc/passwd", '.'),
        ("duck;rm -rf /", ';'),
    ] {
        let error = raw_name
            .to_clean_bot_name()
            .expect_err("illegal name must be rejected");
        assert_eq!(
            error,
            BotNameError::InvalidCharacter {
                name: raw_name.trim(),
                character: illegal_char,
            }
        );
        assert_eq!(
            BotDirEntry::new(raw_name, 1).unwrap_err(),
            minecraft_fishingbot_manager_for_discord::config::Config::get()
                .text()
                .bot_name_illegal_char(raw_name.trim(), illegal_char)
        );
    }
}

#[test]
fn bot_name_rejects_empty_input() {
    if !common::in_configured_child("bot_name_rejects_empty_input") {
        return;
    }

    for raw_name in ["", "   "] {
        let error = raw_name
            .to_clean_bot_name()
            .expect_err("empty name must be rejected");
        assert_eq!(error, BotNameError::Empty);
        assert_eq!(
            BotDirEntry::new(raw_name, 1).unwrap_err(),
            minecraft_fishingbot_manager_for_discord::config::Config::get()
                .text()
                .bot_name_empty()
        );
    }
}

#[test]
fn bot_command_joins_fragments_with_one_space() {
    assert_eq!(
        ["/home", "fishing"].to_clean_bot_command(),
        Some("/home fishing".to_string())
    );

    let empty: &[&str] = &[];
    assert!(empty.to_clean_bot_command().is_none());
}

#[test]
fn bot_command_preserves_user_input() {
    for raw in [
        "/home fishing",
        "/msg  two  spaces",
        "\"/home fishing\"",
        " say hello ",
    ] {
        assert_eq!(
            [raw].to_clean_bot_command(),
            Some(raw.to_string()),
            "single fragment must be forwarded unchanged"
        );
    }
}

#[test]
fn uptime_formats_days_hours_and_minutes_without_rounding() {
    assert_eq!(format_uptime(86_400 + 3_600 + 5 * 60), "1d-1h-5m");
    assert_eq!(format_uptime(59), "0d-0h-0m");
    assert_eq!(format_uptime(61), "0d-0h-1m");
}

#[cfg(target_os = "linux")]
#[test]
fn current_process_has_a_readable_uptime() {
    assert!(process_elapsed_seconds(std::process::id()).is_some());
}

#[test]
fn read_last_lines_returns_only_the_tail() {
    if !common::in_configured_child("read_last_lines_returns_only_the_tail") {
        return;
    }
    let dir = common::temp_dir("read-last-lines");
    let path = dir.join("bot.log");
    fs::write(&path, "one\ntwo\nthree\n").expect("write test log");

    assert_eq!(
        read_last_lines(&path, 2).expect("read the last two lines"),
        vec!["two", "three"]
    );

    common::remove_dir(&dir);
}

#[test]
fn read_last_lines_rejects_zero() {
    if !common::in_configured_child("read_last_lines_rejects_zero") {
        return;
    }

    let dir = common::temp_dir("read-zero-lines");
    let path = dir.join("bot.log");
    fs::write(&path, "one\n").expect("write test log");

    let error = read_last_lines(&path, 0).expect_err("zero lines must be rejected");
    assert_eq!(
        error,
        minecraft_fishingbot_manager_for_discord::config::Config::get()
            .text()
            .bot_log_zero_lines()
    );

    common::remove_dir(&dir);
}

#[test]
fn remove_dir_all_if_exists_is_idempotent() {
    let dir = common::temp_dir("remove-dir");
    fs::write(dir.join("file.log"), "").expect("write test file");

    remove_dir_all_if_exists(&dir).expect("remove an existing directory");
    assert!(!dir.exists());

    remove_dir_all_if_exists(&dir).expect("removing a missing directory is successful");
}

#[test]
fn create_timestamped_log_file_writes_the_first_line() {
    if !common::in_configured_child("create_timestamped_log_file_writes_the_first_line") {
        return;
    }
    let dir = common::temp_dir("create-log-file");
    let first_line = "--- test first line ---";

    let (log_path, mut file) =
        create_timestamped_log_file(&dir, first_line).expect("create timestamped log file");

    let file_name = log_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("log file name must be valid UTF-8");
    assert!(is_timestamped_log_name(file_name), "{file_name}");
    assert_eq!(
        fs::read_to_string(&log_path).expect("read log file"),
        format!("{first_line}\n")
    );

    writeln!(file, "appended line").expect("append log line");
    assert_eq!(
        fs::read_to_string(&log_path).expect("read log file"),
        format!("{first_line}\nappended line\n")
    );

    common::remove_dir(&dir);
}

fn is_timestamped_log_name(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(".log") else {
        return false;
    };
    let parts: Vec<&str> = stem.split('-').collect();

    parts.len() == 6
        && parts.iter().zip([4, 2, 2, 2, 2, 2]).all(|(part, width)| {
            part.len() == width && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}
