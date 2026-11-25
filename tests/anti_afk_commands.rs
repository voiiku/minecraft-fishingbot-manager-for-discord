//! 反 AFK 指令的权限、运行状态与停止流程。

use minecraft_fishingbot_manager_for_discord::operations;
use minecraft_fishingbot_manager_for_discord::text::report::IntoReplyText;
mod common;

use minecraft_fishingbot_manager_for_discord::config;

#[test]
#[ignore = "requires GNU screen"]
fn anti_afk_commands_and_status() {
    const CHILD: &str = "ANTI_AFK_COMMAND_TEST_CHILD";
    const ADMIN: u64 = 123_456_789_012_345_678;
    const USER: u64 = 987_654_321;

    if std::env::var_os(CHILD).is_some() {
        config::Config::load().expect("load test configuration");
        let runtime = tokio::runtime::Runtime::new().expect("create runtime");
        runtime.block_on(async {
            let i18n = minecraft_fishingbot_manager_for_discord::config::Config::get().text();
            let help = operations::queries::help();
            assert!(help.contains("/anti-afk"), "{help}");
            assert!(help.contains("/commandall"), "{help}");
            assert_eq!(
                operations::set_anti_afk(true, USER).await.to_reply_text(),
                i18n.admin_required(USER)
            );
            assert!(
                operations::queries::show_status()
                    .to_reply_text()
                    .contains(i18n.anti_afk_status(false))
            );
            assert_eq!(
                operations::set_anti_afk(true, ADMIN).await.to_reply_text(),
                i18n.anti_afk_started()
            );
            assert_eq!(
                operations::set_anti_afk(true, ADMIN).await.to_reply_text(),
                i18n.anti_afk_already_running()
            );
            assert_eq!(
                operations::set_anti_afk(false, USER).await.to_reply_text(),
                i18n.admin_required(USER)
            );
            assert!(
                operations::queries::show_status()
                    .to_reply_text()
                    .contains(i18n.anti_afk_status(true))
            );
            assert_eq!(
                operations::set_anti_afk(false, ADMIN).await.to_reply_text(),
                i18n.anti_afk_stopped()
            );
            assert_eq!(
                operations::set_anti_afk(false, ADMIN).await.to_reply_text(),
                i18n.anti_afk_stop_not_running()
            );
            assert!(
                operations::queries::show_status()
                    .to_reply_text()
                    .contains(i18n.anti_afk_status(false))
            );
        });
        return;
    }

    let root = common::temp_dir("anti-afk-commands");
    let data = root.join("data");
    std::fs::create_dir_all(data.join("bots")).expect("create test bots directory");
    let mut config_text = common::example_config().to_string();
    let base_setting = "base_dir = \"/absolute/path/yourfolder/fishingbots\"";
    let prefix_setting = "screen_prefix = \"fb1_\"";
    assert!(
        config_text.contains(base_setting),
        "example base_dir changed"
    );
    assert!(
        config_text.contains(prefix_setting),
        "example screen_prefix changed"
    );
    let prefix = format!(
        "anti_afk_test_{}_{:x}_",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    );
    config_text = config_text.replace(
        base_setting,
        &format!("base_dir = {:?}", data.to_str().expect("UTF-8 test path")),
    );
    config_text = config_text.replace(prefix_setting, &format!("screen_prefix = \"{prefix}\""));
    let value: toml::Value = toml::from_str(&config_text).expect("parse test config");
    assert_eq!(
        value["runtime"]["screen_prefix"].as_str(),
        Some(prefix.as_str())
    );
    let sessions = std::process::Command::new("screen")
        .arg("-ls")
        .output()
        .expect("inspect screen sessions");
    assert!(
        matches!(sessions.status.code(), Some(0 | 1)),
        "screen -ls failed: {}",
        String::from_utf8_lossy(&sessions.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&sessions.stdout).contains(&prefix),
        "test prefix must not match an existing screen session"
    );
    let jar = value["paths"]["jar"].as_str().expect("jar file name");
    std::fs::write(data.join(jar), b"test jar").expect("create test jar");
    std::fs::write(root.join(common::TEST_CONFIG_FILE_NAME), config_text).expect("write config");

    let executable = root.join("anti-afk-command-test");
    std::fs::copy(
        std::env::current_exe().expect("test executable"),
        &executable,
    )
    .expect("copy test executable");
    let output = std::process::Command::new(executable)
        .args([
            "--exact",
            "anti_afk_commands_and_status",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("run isolated test process");
    common::remove_dir(&root);
    assert!(
        output.status.success(),
        "isolated test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
