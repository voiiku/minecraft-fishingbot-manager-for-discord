//! 配置结构与路径展开的集成测试。

mod common;

use std::path::PathBuf;
use std::time::Duration;

use minecraft_fishingbot_manager_for_discord::config::{Admins, Config, Paths};

/// 示例配置必须能被当前 schema 解析，且关键运行参数处于有效范围。
#[test]
fn example_config_matches_the_current_schema() {
    common::install_english();

    let config: Config =
        toml::from_str(common::example_config()).expect("example config must match the schema");

    assert!(!config.ping.default_ping_host.trim().is_empty());
    assert!(config.ping.default_ping_port > 0);
    assert!(config.ping.timeout_ms > 0);
    assert!(!config.runtime.screen_prefix.trim().is_empty());
    assert!(config.timing.watchdog_window_ms > 0);
    assert!(config.timing.check_grace_ms > 0);
    assert!(config.timing.probe_interval() > Duration::ZERO);
    assert!(config.timing.probe_interval() <= config.timing.watchdog_window());
    assert_eq!(
        config.timing.first_handshake_timeout(),
        config.timing.watchdog_window() * 2
    );

    assert!(config.startup_queue.max_batch_size > 0);
    assert!(config.startup_queue.bot_login_interval_ms > 0);
    assert!(config.startup_queue.max_pending_requests > 0);
    assert!(!config.command_echo.pause_echo.trim().is_empty());
    assert!(!config.command_echo.unpause_echo.trim().is_empty());
    assert!(config.command_echo.wait_ms > 0);
    assert!(config.command_echo.scan_lines > 0);
    assert!(!config.text.help.trim().is_empty());
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
