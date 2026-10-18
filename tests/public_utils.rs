//! 公开工具函数的集成测试。

mod common;

use std::fs;
use std::io::Write;

use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::utils::{
    ToCleanBotCommand, ToCleanBotName, create_timestamped_log_file, format_uptime,
    process_elapsed_seconds, read_last_lines, remove_dir_all_if_exists,
};

/// 名称规范化会去除首尾空白，并将 ASCII 字母转为小写。
#[test]
fn bot_name_trims_and_lowercases() {
    common::install_english();

    assert_eq!("  Duck_02 ".to_clean_bot_name(), Ok("duck_02".to_string()));
}

/// 连字符与下划线属于合法名称字符。
#[test]
fn bot_name_allows_hyphen_and_underscore() {
    assert_eq!("duck-02_x".to_clean_bot_name(), Ok("duck-02_x".to_string()));
}

/// 非法字符会被拒绝，错误信息必须指出第一个非法字符。
#[test]
fn bot_name_rejects_illegal_characters() {
    common::install_english();

    let illegal_inputs = [
        "中文名字",
        "duck 02",
        "duck.02",
        "../../etc/passwd",
        "duck;rm -rf /",
    ];

    for raw_name in illegal_inputs {
        let error = raw_name
            .to_clean_bot_name()
            .expect_err("illegal name must be rejected");
        let illegal_char = raw_name
            .trim()
            .chars()
            .find(|character| !is_bot_name_char(*character))
            .expect("test case must contain an illegal character");

        assert_eq!(
            error,
            text::get().bot_name_illegal_char(raw_name.trim(), illegal_char),
            "error must contain the offending character: {raw_name:?}"
        );
    }
}

/// 空字符串与纯空白名称会被拒绝，并返回“名称不能为空”的文案。
#[test]
fn bot_name_rejects_empty_input() {
    common::install_english();

    for raw_name in ["", "   "] {
        let error = raw_name
            .to_clean_bot_name()
            .expect_err("empty name must be rejected");
        assert_eq!(error, text::get().bot_name_empty());
    }
}

/// `str` 与 `String` 都通过同一 trait 实现调用名称规范化。
#[test]
fn bot_name_trait_accepts_str_and_string() {
    let owned_name = String::from(" Duck_02 ");

    assert_eq!(owned_name.to_clean_bot_name(), Ok("duck_02".to_string()));
    assert_eq!(
        owned_name.as_str().to_clean_bot_name(),
        Ok("duck_02".to_string())
    );
}

/// 多个指令片段以单空格连接；空片段列表返回错误。
#[test]
fn bot_command_joins_fragments_with_one_space() {
    common::install_english();

    assert_eq!(
        ["/home", "fishing"].to_clean_bot_command(),
        Ok("/home fishing".to_string())
    );

    let empty: &[&str] = &[];
    assert!(empty.to_clean_bot_command().is_err());
}

/// 单个指令片段按原样传递，不裁剪空白、不压缩连续空格、不剥离引号。
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
            Ok(raw.to_string()),
            "single fragment must be forwarded unchanged"
        );
    }
}

/// 运行时长按天、时、分格式化。
#[test]
fn uptime_formats_days_hours_and_minutes() {
    assert_eq!(format_uptime(86_400 + 3_600 + 5 * 60), "1天-1时-5分");
}

/// 不足一分钟的秒数被丢弃，不向上进位到分钟。
#[test]
fn uptime_drops_seconds() {
    assert_eq!(format_uptime(59), "0天-0时-0分");
    assert_eq!(format_uptime(61), "0天-0时-1分");
}

/// 当前测试进程的启动信息可由 `/proc` 读取。
#[cfg(target_os = "linux")]
#[test]
fn current_process_has_a_readable_uptime() {
    assert!(process_elapsed_seconds(std::process::id()).is_some());
}

/// 只返回文件末尾指定行数，并保持原有行顺序。
#[test]
fn read_last_lines_returns_only_the_tail() {
    let dir = common::temp_dir("read-last-lines");
    let path = dir.join("bot.log");
    fs::write(&path, "one\ntwo\nthree\n").expect("write test log");

    assert_eq!(
        read_last_lines(&path, 2).expect("read the last two lines"),
        vec!["two", "three"]
    );

    common::remove_dir(&dir);
}

/// 行数为 0 时返回错误，不按 1 行处理。
#[test]
fn read_last_lines_rejects_zero() {
    common::install_english();

    let dir = common::temp_dir("read-zero-lines");
    let path = dir.join("bot.log");
    fs::write(&path, "one\n").expect("write test log");

    let error = read_last_lines(&path, 0).expect_err("zero lines must be rejected");
    assert_eq!(error, text::get().bot_log_zero_lines());

    common::remove_dir(&dir);
}

/// 删除目录对已存在和不存在两种情况都成功。
#[test]
fn remove_dir_all_if_exists_is_idempotent() {
    let dir = common::temp_dir("remove-dir");
    fs::write(dir.join("file.log"), "").expect("write test file");

    remove_dir_all_if_exists(&dir).expect("remove an existing directory");
    assert!(!dir.exists());

    remove_dir_all_if_exists(&dir).expect("removing a missing directory is successful");
}

/// 创建的时间戳日志文件写入首行，返回句柄位于首行之后。
#[test]
fn create_timestamped_log_file_writes_the_first_line() {
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

/// 测试侧校验时间戳日志文件名：`YYYY-MM-DD-HH-MM-SS.log`。
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

/// 测试侧复刻名称允许字符集，用于计算预期错误中的非法字符。
fn is_bot_name_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}
