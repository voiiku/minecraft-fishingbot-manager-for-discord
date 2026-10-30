//! 配置结构与路径展开的集成测试。

mod common;

use std::path::PathBuf;
use std::time::Duration;

use minecraft_fishingbot_manager_for_discord::config::{Admins, Config, Paths};
use minecraft_fishingbot_manager_for_discord::text;

/// 示例配置必须能被当前 schema 解析，且关键运行参数处于有效范围。
#[test]
fn example_config_matches_the_current_schema() {
    common::install_english();

    let config: Config =
        toml::from_str(common::example_config()).expect("example config must match the schema");

    assert!(!config.ping.default_ping_host.trim().is_empty());
    assert!(config.ping.default_ping_port > 0);
    assert_eq!(config.ping.timeout_ms, 3000);
    assert!(!config.runtime.screen_prefix.trim().is_empty());
    assert_eq!(config.anti_afk.command, "!/!!/!!ENTER!");
    assert_eq!(config.anti_afk.execution_interval_seconds, 180);
    assert_eq!(config.anti_afk.input_delay_seconds, 0.05);
    assert_eq!(config.timing.watchdog_window_ms, 30000);
    assert_eq!(config.timing.check_grace_ms, 5000);
    assert_eq!(config.timing.stop_grace_ms, 3000);
    assert_eq!(config.timing.stop_settle_ms, 500);
    assert!(config.timing.probe_interval() > Duration::ZERO);
    assert!(config.timing.probe_interval() <= config.timing.watchdog_window());
    assert_eq!(
        config.timing.first_handshake_timeout(),
        config.timing.watchdog_window() * 2
    );

    assert!(config.startup_queue.max_batch_size > 0);
    assert_eq!(config.startup_queue.bot_login_interval_ms, 1000);
    assert_eq!(config.startup_queue.batch_login_interval_ms, 20000);
    assert!(config.startup_queue.max_pending_requests > 0);
    assert!(!config.command_echo.pause_echo.trim().is_empty());
    assert!(!config.command_echo.unpause_echo.trim().is_empty());
    assert_eq!(config.command_echo.wait_ms, 300);
    assert!(config.command_echo.scan_lines > 0);
    assert!(!config.text.help.trim().is_empty());
}

#[test]
fn example_help_fits_and_includes_batch_commands() {
    let config: Config = toml::from_str(common::example_config()).unwrap();
    assert!(config.text.help.contains("/anti-afk"));
    assert!(config.text.help.contains("/commandall"));

    for language in text::all_languages() {
        let reply = format!(
            "{}:\n=======================\n\n{}:\n{}",
            language.help_title(),
            language.report_content_label(),
            config.text.help
        );
        assert!(
            reply.encode_utf16().count() <= config.runtime.max_reply_chars,
            "example help must fit without truncation"
        );
    }
}

#[test]
fn timing_config_accepts_seconds_with_millisecond_precision() {
    let example: toml::Value = toml::from_str(common::example_config()).unwrap();
    type TimingField = (&'static str, &'static str, fn(&Config) -> u64);
    let fields: [TimingField; 8] = [
        ("command_echo", "wait", |config| config.command_echo.wait_ms),
        ("ping", "timeout", |config| config.ping.timeout_ms),
        ("startup_queue", "bot_login_interval", |config| {
            config.startup_queue.bot_login_interval_ms
        }),
        ("startup_queue", "batch_login_interval", |config| {
            config.startup_queue.batch_login_interval_ms
        }),
        ("timing", "check_grace", |config| {
            config.timing.check_grace_ms
        }),
        ("timing", "stop_grace", |config| config.timing.stop_grace_ms),
        ("timing", "stop_settle", |config| {
            config.timing.stop_settle_ms
        }),
        ("timing", "watchdog_window", |config| {
            config.timing.watchdog_window_ms
        }),
    ];

    for (table, field, get_value) in fields {
        for (value, expected) in [("0.001", 1), ("0.3", 300), ("1.001", 1001), ("20", 20000)] {
            let mut input = example.clone();
            let parsed: toml::Value = toml::from_str(&format!("value = {value}")).unwrap();
            input[table][field] = parsed["value"].clone();
            let config: Config = input.try_into().expect("valid seconds must parse");
            assert_eq!(get_value(&config), expected, "{table}.{field} = {value}");
        }

        for value in [
            "-1",
            "-0.1",
            "nan",
            "inf",
            "0.0005",
            "0.0000000001",
            "1e100",
        ] {
            let mut input = example.clone();
            let parsed: toml::Value = toml::from_str(&format!("value = {value}")).unwrap();
            input[table][field] = parsed["value"].clone();
            let result: Result<Config, _> = input.try_into();
            assert!(result.is_err(), "{table}.{field} = {value} must fail");
        }

        let mut legacy = example.clone();
        let fields = legacy[table].as_table_mut().unwrap();
        let value = fields.remove(field).unwrap();
        fields.insert(format!("{field}_ms"), value);
        let result: Result<Config, _> = legacy.try_into();
        assert!(result.is_err(), "legacy {table}.{field}_ms must fail");
    }
}

#[test]
fn anti_afk_table_and_all_fields_are_required() {
    let example: toml::Value = toml::from_str(common::example_config()).unwrap();
    let mut missing_table = example.clone();
    missing_table.as_table_mut().unwrap().remove("anti_afk");
    let result: Result<Config, _> = missing_table.try_into();
    let error = result.unwrap_err();
    assert!(error.to_string().contains("anti_afk"), "{error}");

    for field in ["command", "execution_interval", "input_delay"] {
        let mut missing_field = example.clone();
        missing_field["anti_afk"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        let result: Result<Config, _> = missing_field.try_into();
        let error = result.unwrap_err();
        assert!(error.to_string().contains(field), "{error}");
    }
}

#[test]
fn anti_afk_time_keys_no_longer_accept_unit_suffixes() {
    let example: toml::Value = toml::from_str(common::example_config()).unwrap();
    for (field, old_field) in [
        ("execution_interval", "execution_interval_seconds"),
        ("input_delay", "input_delay_seconds"),
    ] {
        let mut input = example.clone();
        let fields = input["anti_afk"].as_table_mut().unwrap();
        let value = fields.remove(field).unwrap();
        fields.insert(old_field.to_string(), value);
        let result: Result<Config, _> = input.try_into();
        assert!(result.is_err(), "legacy anti_afk.{old_field} must fail");
    }
}

/// 缺少必填表时配置解析失败。
#[test]
fn missing_required_table_is_rejected() {
    let text = r#"
[ping]
host = "example.com"
port = 25565
"#;

    assert!(toml::from_str::<Config>(text).is_err());
}

/// 配置中出现未知字段时解析失败。
#[test]
fn unknown_field_is_rejected() {
    let text = common::example_config().replace("[runtime]", "[runtime]\nunknown_field = 1");

    assert!(toml::from_str::<Config>(&text).is_err());
}

/// 示例配置中的每个键都必须在配置文档中以行内代码形式出现。
#[test]
fn every_example_key_is_documented() {
    let docs = include_str!("../docs/mods/config/config-zh_cn.md");
    let keys = example_keys();

    assert!(
        keys.contains(&"max_reply_chars".to_string()),
        "the example config key extractor produced no known key: {keys:?}"
    );

    let undocumented_keys: Vec<String> = keys
        .into_iter()
        .filter(|key| !docs.contains(&format!("`{key}`")))
        .collect();

    assert!(
        undocumented_keys.is_empty(),
        "configuration keys missing from docs/mods/config/config-zh_cn.md: {undocumented_keys:?}"
    );
}

/// 提取示例配置中的裸键名；跳过注释、表头、引号键与多行字符串内容。
fn example_keys() -> Vec<String> {
    let text = common::example_config();
    let mut keys = Vec::new();
    let mut in_multiline_string = false;

    for line in text.lines() {
        let trimmed = line.trim();
        let triple_quote_count = trimmed.matches("\"\"\"").count();

        if in_multiline_string {
            if triple_quote_count % 2 == 1 {
                in_multiline_string = false;
            }
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }

        let Some((key, _)) = trimmed.split_once(" = ") else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        keys.push(key.to_string());

        if triple_quote_count % 2 == 1 {
            in_multiline_string = true;
        }
    }

    keys
}

/// 相对路径以 `base_dir` 为基准展开，`bots_dir` 由代码派生。
#[test]
fn paths_expand_relative_to_base_dir() {
    let base_dir = absolute("srv/fishbot/data");
    let paths = read_paths(
        &base_dir.display().to_string(),
        "FishingBot.jar",
        "conf.json",
    )
    .expect("valid paths must parse");

    assert_eq!(paths.base_dir, base_dir);
    assert_eq!(paths.bots_dir, base_dir.join("bots"));
    assert_eq!(paths.jar, base_dir.join("FishingBot.jar"));
    assert_eq!(paths.fishingbot_config, base_dir.join("conf.json"));
}

/// 管理员名单可按 Discord 用户 ID 查询，并保留显示名称。
#[test]
fn admins_lookup_by_user_id() {
    let admins: Admins =
        toml::from_str("\"2878855255\" = \"SlanceLenk\"").expect("parse administrator list");

    assert!(admins.is_admin(2878855255));
    assert!(!admins.is_admin(123456));
    assert!(!admins.is_empty());
    assert!(
        admins
            .iter()
            .any(|(user_id, name)| user_id == "2878855255" && name == "SlanceLenk")
    );
}

/// `base_dir` 必须是绝对路径。
#[test]
fn relative_base_dir_is_rejected() {
    common::install_english();

    let error = read_paths("fishingbots", "FishingBot.jar", "conf.json")
        .expect_err("relative base_dir must be rejected");
    assert!(error.contains("absolute path"), "{error}");
}

/// `jar` 与 `fishingbot_config_file_name` 必须是相对路径。
#[test]
fn absolute_jar_or_config_file_name_is_rejected() {
    common::install_english();

    let base_dir = absolute("srv/fishbot/data").display().to_string();

    let error = read_paths(
        &base_dir,
        &absolute("opt/FishingBot.jar").display().to_string(),
        "conf.json",
    )
    .expect_err("absolute jar must be rejected");
    assert!(error.contains("relative path"), "{error}");

    let error = read_paths(
        &base_dir,
        "FishingBot.jar",
        &absolute("etc/conf.json").display().to_string(),
    )
    .expect_err("absolute config file name must be rejected");
    assert!(error.contains("fishingbot_config_file_name"), "{error}");
}

/// 构造测试用的绝对路径。
fn absolute(path: &str) -> PathBuf {
    PathBuf::from(format!("/{path}"))
}

/// 以配置文件的 `[paths]` 表格式反序列化路径。
fn read_paths(
    base_dir: &str,
    jar: &str,
    fishingbot_config_file_name: &str,
) -> Result<Paths, String> {
    let text = format!(
        "base_dir = {base_dir:?}\njar = {jar:?}\nfishingbot_config_file_name = {fishingbot_config_file_name:?}\n"
    );

    toml::from_str(&text).map_err(|error| error.to_string())
}
