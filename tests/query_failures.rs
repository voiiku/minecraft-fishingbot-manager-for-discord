//! screen 查询、登录会话状态与反 AFK 的回归测试。

mod common;

use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, AntiAfkAction, BotControlAction, BotManageAction, ShowAction, config,
    handle_all_bots_control, handle_anti_afk, handle_bot_control, handle_bot_login,
    handle_bot_login_await, handle_bot_manage, handle_show, text,
};

#[test]
fn screen_queries_login_and_anti_afk() {
    const CHILD: &str = "FISHBOT_QUERY_FAILURE_TEST_CHILD";
    const ADMIN: u64 = 123_456_789_012_345_678;

    if let Some(mode) = std::env::var_os(CHILD) {
        config::Config::get_or_init();
        let i18n = text::get();
        match mode.to_str().unwrap() {
            "status" => {
                let reply = handle_show(ShowAction::Status, ADMIN);
                assert!(reply.contains("duck"), "{reply}");
                assert!(reply.contains("goose"), "{reply}");
                assert!(reply.contains(i18n.bot_status_stopped()), "{reply}");
                assert!(!reply.contains("crane"), "{reply}");
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();
                assert_eq!(
                    std::fs::read_to_string(counter).unwrap(),
                    "x",
                    "status must use one screen snapshot"
                );
            }
            "failure" => {
                let expected = "simulated screen query error";
                let replies = [
                    handle_show(ShowAction::Status, ADMIN),
                    handle_bot_control(BotControlAction::Start, "duck", ADMIN),
                    handle_bot_control(BotControlAction::Stop, "duck", ADMIN),
                    handle_bot_control(BotControlAction::Restart, "duck", ADMIN),
                    handle_bot_control(BotControlAction::Command(&["/help"]), "duck", ADMIN),
                    handle_bot_manage(BotManageAction::Delete, "duck", ADMIN),
                    handle_all_bots_control(AllBotsControlAction::StartAll, ADMIN),
                    handle_all_bots_control(AllBotsControlAction::StopAll, ADMIN),
                    handle_all_bots_control(AllBotsControlAction::RestartAll, ADMIN),
                    handle_all_bots_control(AllBotsControlAction::CommandAll(&["/help"]), ADMIN),
                    handle_bot_login("duck", ADMIN).unwrap_err(),
                ];
                for reply in replies {
                    assert!(reply.contains(expected), "{reply}");
                }
            }
            "empty" => {
                assert!(
                    !minecraft_fishingbot_manager_for_discord::utils::is_bot_screen_running("duck")
                        .expect("no sessions is not an error")
                );
            }
            "missing" => {
                let error =
                    minecraft_fishingbot_manager_for_discord::utils::is_bot_screen_running("duck")
                        .expect_err("missing screen must be reported");
                assert!(error.contains("screen"), "{error}");
            }
            "anti_afk_timestamp" => {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    assert_eq!(
                        handle_anti_afk(AntiAfkAction::On, ADMIN).await,
                        i18n.anti_afk_started()
                    );
                    let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();
                    tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        loop {
                            if std::fs::read_to_string(&counter).is_ok_and(|value| value.len() >= 3)
                            {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .expect("all input steps must use the correct session ID");
                    assert_eq!(std::fs::read_to_string(counter).unwrap(), "//\r");
                    assert_eq!(
                        handle_anti_afk(AntiAfkAction::Off, ADMIN).await,
                        i18n.anti_afk_stopped()
                    );
                });
            }
            "race" => {
                assert_eq!(
                    handle_bot_login("duck", ADMIN).unwrap_err(),
                    i18n.refusal_bot_running("duck")
                );
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();
                assert_eq!(std::fs::read_to_string(counter).unwrap(), "x");
            }
            "verify_failure" => {
                let reply = handle_bot_control(BotControlAction::Start, "duck", ADMIN);
                assert!(reply.contains("verification screen error"), "{reply}");
            }
            "stop_failure" => {
                let reply = handle_bot_control(BotControlAction::Stop, "duck", ADMIN);
                assert!(reply.contains("stop screen error"), "{reply}");
            }
            "restart_failure" => {
                let reply = handle_bot_control(BotControlAction::Restart, "duck", ADMIN);
                assert!(reply.contains("stop screen error"), "{reply}");
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(
                    !std::path::Path::new(&format!("{counter}.launched")).exists(),
                    "restart must not start a bot after the stop cannot be verified"
                );
            }
            "login_failure" | "login_await_failure" | "login_recover" => {
                let result = handle_bot_login("duck", ADMIN);
                let reply = if mode == "login_await_failure" {
                    handle_bot_login_await(result.expect("device code should be found"))
                } else {
                    result.unwrap_err()
                };
                assert!(reply.contains("login screen error"), "{reply}");
                let bot = &config::Config::get()
                    .paths
                    .bots_dir
                    .join(ADMIN.to_string())
                    .join("duck");
                let token = if mode == "login_recover" {
                    assert!(!bot.join("refreshToken.renewing").exists());
                    bot.join("refreshToken")
                } else {
                    assert!(!bot.join("refreshToken").exists());
                    bot.join("refreshToken.renewing")
                };
                assert_eq!(std::fs::read_to_string(token).unwrap(), "test token");
            }
            other => panic!("unexpected test mode {other}"),
        }
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let root = common::temp_dir("screen-query");
    let data = root.join("data");
    let bot = data.join("bots").join(ADMIN.to_string()).join("duck");
    let bin = root.join("bin");
    std::fs::create_dir_all(&bot).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bot.join("refreshToken"), "test token").unwrap();
    let owner = bot.parent().unwrap();
    std::fs::create_dir_all(owner.join("goose")).unwrap();
    std::fs::write(owner.join("goose").join("refreshToken"), "test token").unwrap();
    std::fs::create_dir_all(owner.join("crane")).unwrap();

    let mut config_text = common::example_config().to_string();
    let base_setting = "base_dir = \"/absolute/path/yourfolder/fishingbots\"";
    let prefix_setting = "screen_prefix = \"fb1_\"";
    assert!(config_text.contains(base_setting));
    assert!(config_text.contains(prefix_setting));
    config_text = config_text.replace(
        base_setting,
        &format!("base_dir = {:?}", data.to_str().unwrap()),
    );
    config_text = config_text.replace(prefix_setting, "screen_prefix = \"lookup_\"");
    for (before, after) in [
        ("batch_login_interval = 20", "batch_login_interval = 0.001"),
        ("check_grace = 5", "check_grace = 0.001"),
        ("stop_grace = 3", "stop_grace = 0.001"),
        ("stop_settle = 0.5", "stop_settle = 0.001"),
    ] {
        assert!(config_text.contains(before));
        config_text = config_text.replace(before, after);
    }
    let value: toml::Value = toml::from_str(&config_text).unwrap();
    let jar = value["paths"]["jar"].as_str().unwrap();
    std::fs::write(data.join(jar), b"test jar").unwrap();
    std::fs::write(root.join(config::CONFIG_FILE_NAME), config_text).unwrap();

    let screen = bin.join("screen");
    std::fs::write(
        &screen,
        r#"#!/bin/sh
case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
  status)
    printf x >> "$FISHBOT_SCREEN_COUNTER"
    printf 'There are screens on:\n\t123.lookup_duck\t(Detached)\n\t456.lookup_goose\t(Dead ???)\n\t789.lookup_crane\t(Detached)\n'
    exit 0 ;;
  anti_afk_timestamp)
    case "$1" in
      -version) printf 'Screen version 5.0.2\n'; exit 0 ;;
      -list) printf 'There is a screen on:\n\t123.lookup_duck\t(10/29/26 21:40:59)\t(Detached)\n'; exit 0 ;;
      -S)
        if [ "$2" != "123.lookup_duck" ] || [ "$3" != "-X" ] || [ "$4" != "stuff" ]; then
          printf 'unexpected screen invocation: %s %s %s %s\n' "$1" "$2" "$3" "$4" >&2
          exit 1
        fi
        printf '%s' "$5" >> "$FISHBOT_SCREEN_COUNTER"
        exit 0 ;;
      *) exit 2 ;;
    esac ;;
  failure) printf 'simulated screen query error\n' >&2; exit 1 ;;
  empty) printf 'No Sockets found in /tmp/test.\n'; exit 1 ;;
  race)
    if [ -f "$FISHBOT_SCREEN_COUNTER" ]; then
      printf x >> "$FISHBOT_SCREEN_COUNTER"
      printf 'No Sockets found in /tmp/test.\n'
      exit 1
    fi
    printf x > "$FISHBOT_SCREEN_COUNTER"
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  verify_failure)
    if [ "$1" = "-dmS" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf 'verification screen error\n' >&2
      exit 1
    fi
    printf 'No Sockets found in /tmp/test.\n'
    exit 1 ;;
  stop_failure|restart_failure)
    if [ "$1" = "-dmS" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ "$1" = "-X" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.stopping"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ]; then
      printf 'stop screen error\n' >&2
      exit 1
    fi
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  login_failure|login_await_failure|login_recover)
    if [ "$1" = "-dmS" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      if [ "$FISHBOT_QUERY_FAILURE_TEST_CHILD" = "login_await_failure" ]; then
        for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
          printf 'Please log in using the following code ABCD1234 at https://www.microsoft.com/link.\n' >> "$file"
        done
      fi
      exit 0
    fi
    if [ "$1" = "-X" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.stopping"
      exit 0
    fi
    if [ "$FISHBOT_QUERY_FAILURE_TEST_CHILD" = "login_recover" ] && [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ]; then
      printf 'No Sockets found in /tmp/test.\n'
      exit 1
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf 'login screen error\n' >&2
      exit 1
    fi
    printf 'No Sockets found in /tmp/test.\n'
    exit 1 ;;
  *) exit 2 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&screen, std::fs::Permissions::from_mode(0o700)).unwrap();
    let executable = root.join("query-failures-test");
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();

    let mut failures = Vec::new();
    for mode in [
        "status",
        "anti_afk_timestamp",
        "failure",
        "empty",
        "missing",
        "race",
        "verify_failure",
        "stop_failure",
        "restart_failure",
        "login_failure",
        "login_await_failure",
        "login_recover",
    ] {
        if mode.starts_with("login_") {
            let backup = bot.join("refreshToken.renewing");
            if backup.exists() {
                std::fs::remove_file(backup).unwrap();
            }
            std::fs::write(bot.join("refreshToken"), "test token").unwrap();
        }
        let output = std::process::Command::new(&executable)
            .args([
                "--exact",
                "screen_queries_login_and_anti_afk",
                "--nocapture",
            ])
            .env(CHILD, mode)
            .env(
                "FISHBOT_SCREEN_COUNTER",
                root.join(format!("counter-{mode}")),
            )
            .env("FISHBOT_BOT_PATH", &bot)
            .env(
                "PATH",
                if mode == "missing" {
                    root.join("missing-bin")
                } else {
                    bin.clone()
                },
            )
            .output()
            .expect("run isolated test process");
        if !output.status.success() {
            failures.push(format!(
                "{mode}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    common::remove_dir(&root);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
