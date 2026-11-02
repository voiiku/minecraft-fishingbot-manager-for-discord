//! screen 查询、登录会话状态与反 AFK 的回归测试。

mod common;

use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, AntiAfkAction, BotControlAction, BotManageAction, ShowAction,
    check_sync_commands_permission, config, handle_all_bots_control, handle_anti_afk,
    handle_bot_control, handle_bot_log, handle_bot_login, handle_bot_login_await,
    handle_bot_manage, handle_checkserver, handle_fishingbot_config, handle_show,
    handle_sync_commands, regenerate_fishingbot_config, text,
};

#[test]
fn screen_queries_login_and_anti_afk() {
    const CHILD: &str = "FISHBOT_QUERY_FAILURE_TEST_CHILD";
    const ADMIN: u64 = 123_456_789_012_345_678;

    if let Some(mode) = std::env::var_os(CHILD) {
        config::Config::get_or_init();
        let i18n = text::get();
        match mode.to_str().unwrap() {
            "game_config" => {
                let base_dir = &config::Config::get().paths.base_dir;
                let legacy_path = base_dir.join("conf.json");
                std::fs::write(&legacy_path, "legacy config").unwrap();
                regenerate_fishingbot_config().expect("generate game config");

                let view = handle_fishingbot_config(ADMIN).expect("read game config");
                assert_eq!(view.file_name, "config.json");
                assert_eq!(
                    view.content,
                    std::fs::read_to_string(base_dir.join("config.json")).unwrap()
                );
                let parsed: serde_json::Value = serde_json::from_str(&view.content).unwrap();
                assert!(parsed["server"].is_object());
                assert_eq!(
                    std::fs::read_to_string(legacy_path).unwrap(),
                    "legacy config"
                );
            }
            "checkserver" => {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let reply = runtime.block_on(handle_checkserver(ADMIN));
                assert!(reply.contains(i18n.checkserver_title()), "{reply}");
                assert!(reply.contains("CPU usage:"), "{reply}");
                assert!(reply.contains("Memory used/total:"), "{reply}");
                assert!(reply.contains("Disk usage:"), "{reply}");
                assert!(reply.contains("Host uptime:"), "{reply}");
            }
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
                let mut canonical =
                    minecraft_fishingbot_manager_for_discord::utils::token_ready_bot_names_with_screen_sessions(
                    )
                    .unwrap();
                let mut compatible =
                    minecraft_fishingbot_manager_for_discord::utils::all_running_bot_names()
                        .unwrap();
                canonical.sort();
                compatible.sort();
                assert_eq!(canonical, ["duck", "goose"]);
                assert_eq!(compatible, canonical);
            }
            "startall_seconds" => {
                let reply = handle_all_bots_control(AllBotsControlAction::StartAll, ADMIN);
                assert!(
                    reply.contains(
                        "1 seconds between bots in a batch, 0.001 seconds between batches"
                    ),
                    "{reply}"
                );
            }
            "start_and_status_online" => {
                let reply = handle_bot_control(BotControlAction::Start, "duck", ADMIN);
                assert_eq!(reply, i18n.start_command_sent_confirmed("duck"));

                let status = handle_show(ShowAction::Status, ADMIN);
                assert!(status.contains(i18n.bot_status_online()), "{status}");
            }
            "status_log_transitions" => {
                let logs_dir = config::Config::get()
                    .paths
                    .bots_dir
                    .join(ADMIN.to_string())
                    .join("duck")
                    .join(&config::Config::get().runtime.bot_logs_dir);
                std::fs::create_dir_all(&logs_dir).unwrap();
                let log_path = logs_dir.join("9999-12-31-23-59-59.log");
                let online = &config::Config::get().keywords.online;
                let reconnect = format!(
                    "{}5{}",
                    i18n.jar_reconnect_prefix(),
                    i18n.jar_reconnect_suffix()
                );
                for (content, expected) in [
                    (
                        "Loading configuration".to_string(),
                        i18n.bot_status_initializing(),
                    ),
                    (
                        format!("Loading configuration\n{online}\n"),
                        i18n.bot_status_online(),
                    ),
                    (
                        format!("{online}\n{reconnect}\n"),
                        i18n.bot_status_reconnecting(),
                    ),
                    (format!("{reconnect}\n{online}\n"), i18n.bot_status_online()),
                ] {
                    std::fs::write(&log_path, content).unwrap();
                    let status = handle_show(ShowAction::Status, ADMIN);
                    assert!(status.contains(expected), "{status}");
                }
                std::fs::remove_file(log_path).unwrap();
            }
            "pauseall" | "pauseall_zh" => {
                let owner = config::Config::get().paths.bots_dir.join(ADMIN.to_string());
                for (name, contents) in [
                    ("duck", "initial\n"),
                    ("owl", "Fishing paused\n"),
                    ("raven", "initial\n"),
                    ("swan", "initial\n"),
                ] {
                    let bot = owner.join(name);
                    let logs = bot.join(&config::Config::get().runtime.bot_logs_dir);
                    std::fs::create_dir_all(&logs).unwrap();
                    std::fs::write(bot.join("refreshToken"), "test token").unwrap();
                    std::fs::write(logs.join("9999-12-31-23-59-59.log"), contents).unwrap();
                }
                let falcon = owner.join("falcon");
                std::fs::create_dir_all(&falcon).unwrap();
                std::fs::write(falcon.join("refreshToken"), "test token").unwrap();
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();
                for action in [
                    AllBotsControlAction::PauseAll,
                    AllBotsControlAction::UnpauseAll,
                ] {
                    assert_eq!(
                        handle_all_bots_control(action, 987_654_322),
                        i18n.admin_required(987_654_322)
                    );
                }
                assert!(!std::path::Path::new(&counter).exists());

                for (action, command, title, confirmed, status_label, other_status) in [
                    (
                        AllBotsControlAction::PauseAll,
                        "/pause",
                        i18n.pauseall_title(),
                        i18n.bot_pause_confirmed("duck"),
                        i18n.bot_pause_paused(),
                        i18n.bot_pause_fishing(),
                    ),
                    (
                        AllBotsControlAction::UnpauseAll,
                        "/unpause",
                        i18n.unpauseall_title(),
                        i18n.bot_unpause_confirmed("duck"),
                        i18n.bot_pause_fishing(),
                        i18n.bot_pause_paused(),
                    ),
                ] {
                    let reply = handle_all_bots_control(action, ADMIN);
                    let unconfirmed = i18n.bot_pause_unconfirmed("owl", command);
                    let missing_log = i18n.bot_status_log_not_found("falcon");
                    let send_failed = i18n.send_command_failed("goose", None);
                    let superseded =
                        i18n.bot_line("raven", &i18n.bot_pause_superseded(command, other_status));
                    assert_eq!(
                        reply,
                        format!(
                            "{title}:\n=======================\n- {}\n- {}\n- {}\n\n{}:\n- {confirmed}\n- {unconfirmed}\n\n{}:\n- {missing_log}\n- {send_failed}\n- {superseded}",
                            i18n.count_line(i18n.count_running_accounts(), 5),
                            i18n.count_line(i18n.count_processed_successfully(), 2),
                            i18n.count_line(i18n.count_processing_failed(), 3),
                            i18n.report_succeeded_label(),
                            i18n.report_failed_label()
                        )
                    );

                    let status = handle_show(ShowAction::Status, ADMIN);
                    let duck = status.lines().find(|line| line.contains("duck |")).unwrap();
                    let owl = status.lines().find(|line| line.contains("owl |")).unwrap();
                    let raven = status
                        .lines()
                        .find(|line| line.contains("raven |"))
                        .unwrap();
                    assert!(duck.contains(status_label), "{duck}");
                    assert!(owl.contains(i18n.bot_pause_fishing()), "{owl}");
                    assert!(raven.contains(other_status), "{raven}");
                }
                assert_eq!(
                    std::fs::read_to_string(counter).unwrap(),
                    "lookup_duck:/pause\nlookup_falcon:/pause\nlookup_goose:/pause\nlookup_owl:/pause\nlookup_raven:/pause\n\
                     lookup_duck:/unpause\nlookup_falcon:/unpause\nlookup_goose:/unpause\nlookup_owl:/unpause\nlookup_raven:/unpause\n"
                );
                let duck_log = owner
                    .join("duck")
                    .join("logs")
                    .join("9999-12-31-23-59-59.log");
                std::fs::write(&duck_log, "old log ".repeat(100)).unwrap();
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                std::fs::write(format!("{counter}.rotate"), "").unwrap();
                assert_eq!(
                    handle_bot_control(BotControlAction::Pause, "duck", ADMIN),
                    i18n.bot_pause_confirmed("duck")
                );
                std::fs::remove_dir_all(owner.join("owl")).unwrap();
                std::fs::remove_dir_all(owner.join("raven")).unwrap();
                std::fs::remove_dir_all(owner.join("falcon")).unwrap();
                std::fs::remove_dir_all(owner.join("swan")).unwrap();
                std::fs::remove_file(duck_log).unwrap();
            }
            "pauseall_empty" => {
                for action in [
                    AllBotsControlAction::PauseAll,
                    AllBotsControlAction::UnpauseAll,
                ] {
                    let reply = handle_all_bots_control(action, ADMIN);
                    assert!(
                        reply.contains(&i18n.count_line(i18n.count_running_accounts(), 0)),
                        "{reply}"
                    );
                    assert!(
                        reply.contains(&i18n.count_line(i18n.count_processing_failed(), 0)),
                        "{reply}"
                    );
                }
            }
            "pauseall_recheck_error" => {
                let reply = handle_all_bots_control(AllBotsControlAction::PauseAll, ADMIN);
                assert!(
                    reply.contains(&i18n.count_line(i18n.count_running_accounts(), 1)),
                    "{reply}"
                );
                let line = reply
                    .lines()
                    .find(|line| line.contains("simulated recheck error"))
                    .expect("recheck error must be reported");
                assert!(line.starts_with("- Bot duck:"), "{line}");
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
                    handle_all_bots_control(AllBotsControlAction::PauseAll, ADMIN),
                    handle_all_bots_control(AllBotsControlAction::UnpauseAll, ADMIN),
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
            "restart_no_token" => {
                let reply = handle_bot_control(BotControlAction::Restart, "crane", ADMIN);
                assert_eq!(reply, i18n.restart_skipped_missing_token("crane"));
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(!std::path::Path::new(&format!("{counter}.unexpected")).exists());
            }
            "restart_success"
            | "restart_success_zh"
            | "restart_unconfirmed"
            | "restart_unconfirmed_zh"
            | "restartall_success"
            | "restartall_unconfirmed" => {
                let is_batch = mode == "restartall_success" || mode == "restartall_unconfirmed";
                let confirmed = mode == "restart_success"
                    || mode == "restart_success_zh"
                    || mode == "restartall_success";
                let reply = if is_batch {
                    handle_all_bots_control(AllBotsControlAction::RestartAll, ADMIN)
                } else {
                    handle_bot_control(BotControlAction::Restart, "duck", ADMIN)
                };
                if is_batch {
                    assert!(reply.contains(i18n.restartall_title()), "{reply}");
                    let expected = if confirmed {
                        i18n.start_command_sent_confirmed("duck")
                    } else {
                        i18n.start_command_sent_unconfirmed("duck")
                    };
                    assert!(reply.contains(&expected), "{reply}");
                } else {
                    let expected = if confirmed {
                        i18n.bot_restart_online_confirmed("duck")
                    } else {
                        i18n.bot_restart_session_unconfirmed("duck")
                    };
                    assert_eq!(reply, expected);
                }
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(std::path::Path::new(&format!("{counter}.stopping")).exists());
                assert!(std::path::Path::new(&format!("{counter}.launched")).exists());
            }
            "restart_start_failed" | "start_dead" | "restart_dead" | "restartall_dead" => {
                let reply = match mode.to_str().unwrap() {
                    "start_dead" => handle_bot_control(BotControlAction::Start, "duck", ADMIN),
                    "restartall_dead" => {
                        handle_all_bots_control(AllBotsControlAction::RestartAll, ADMIN)
                    }
                    _ => handle_bot_control(BotControlAction::Restart, "duck", ADMIN),
                };
                assert!(reply.contains(&i18n.start_failed("duck")), "{reply}");
                assert!(
                    !reply.contains(&i18n.bot_restart_online_confirmed("duck")),
                    "{reply}"
                );
                if mode == "restartall_dead" {
                    assert!(reply.contains(i18n.restartall_title()), "{reply}");
                    assert!(
                        reply.contains(&i18n.count_line(i18n.count_restart_failed(), 1)),
                        "{reply}"
                    );
                }
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(std::path::Path::new(&format!("{counter}.launched")).exists());
                if mode != "start_dead" {
                    assert!(std::path::Path::new(&format!("{counter}.stopping")).exists());
                }
            }
            "routing_permissions" => {
                const OWNER: u64 = 987_654_321;
                const OTHER: u64 = 987_654_322;
                for check_permission in [check_sync_commands_permission, handle_sync_commands] {
                    assert_eq!(check_permission(OTHER), Err(i18n.admin_required(OTHER)));
                    assert_eq!(check_permission(OWNER), Err(i18n.admin_required(OWNER)));
                    assert_eq!(check_permission(ADMIN), Ok(()));
                }
                let bot = config::Config::get()
                    .paths
                    .bots_dir
                    .join(OWNER.to_string())
                    .join("owl");
                std::fs::create_dir_all(&bot).unwrap();
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();

                for action in [
                    AllBotsControlAction::StartAll,
                    AllBotsControlAction::StopAll,
                    AllBotsControlAction::RestartAll,
                    AllBotsControlAction::PauseAll,
                    AllBotsControlAction::UnpauseAll,
                    AllBotsControlAction::CommandAll(&["/help"]),
                ] {
                    assert_eq!(
                        handle_all_bots_control(action, OTHER),
                        i18n.admin_required(OTHER)
                    );
                }
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    for action in [AntiAfkAction::On, AntiAfkAction::Off] {
                        assert_eq!(
                            handle_anti_afk(action, OTHER).await,
                            i18n.admin_required(OTHER)
                        );
                    }
                });
                assert!(
                    !std::path::Path::new(&counter).exists(),
                    "unauthorized actions must not query screen"
                );

                let not_found = i18n.bot_dir_not_found("owl");
                assert_eq!(
                    handle_bot_control(BotControlAction::Start, "owl", OTHER),
                    not_found
                );
                assert_eq!(
                    handle_bot_manage(BotManageAction::Delete, "owl", OTHER),
                    not_found
                );
                assert_eq!(handle_bot_log(OTHER, "owl", 1).unwrap_err(), not_found);
                assert_eq!(handle_bot_login("owl", OTHER).unwrap_err(), not_found);
                assert!(bot.exists(), "unauthorized delete must not remove the bot");

                for caller in [OWNER, ADMIN] {
                    assert_eq!(
                        handle_bot_control(BotControlAction::Start, "owl", caller),
                        i18n.bot_start_missing_token("owl")
                    );
                    assert_eq!(
                        handle_bot_manage(BotManageAction::Delete, "owl", caller),
                        i18n.refusal_bot_running("owl")
                    );
                    assert_eq!(
                        handle_bot_log(caller, "owl", 1).unwrap_err(),
                        i18n.bot_log_missing("owl")
                    );
                    assert_eq!(
                        handle_bot_login("owl", caller).unwrap_err(),
                        i18n.bot_login_already_running("owl")
                    );
                }
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

    let mut value: toml::Value = toml::from_str(common::example_config()).unwrap();
    value["paths"]["base_dir"] = toml::Value::String(data.to_str().unwrap().to_string());
    value["runtime"]["screen_prefix"] = toml::Value::String("lookup_".to_string());
    value["startup_queue"]["bot_login_interval"] = toml::Value::Integer(1);
    for (table, field) in [
        ("startup_queue", "batch_login_interval"),
        ("timing", "check_grace"),
        ("timing", "stop_grace"),
        ("timing", "stop_settle"),
    ] {
        value[table][field] = toml::Value::Float(0.001);
    }
    let jar = value["paths"]["jar"].as_str().unwrap();
    std::fs::write(data.join(jar), b"test jar").unwrap();
    std::fs::write(
        root.join(config::CONFIG_FILE_NAME),
        toml::to_string(&value).unwrap(),
    )
    .unwrap();

    let screen = bin.join("screen");
    std::fs::write(
        &screen,
        r#"#!/bin/sh
case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
  status)
    printf x >> "$FISHBOT_SCREEN_COUNTER"
    printf 'There are screens on:\n\t123.lookup_duck\t(Detached)\n\t456.lookup_goose\t(Dead ???)\n\t789.lookup_crane\t(Detached)\n'
    exit 0 ;;
  startall_seconds)
    if [ "$1" = "-dmS" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    printf 'There are screens on:\n\t456.lookup_goose\t(Detached)\n'
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf '\t123.lookup_duck\t(Detached)\n'
    fi
    exit 0 ;;
  start_and_status_online)
    if [ "$1" = "-dmS" ]; then
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        printf 'Welcome to the xxx server\n' >> "$file"
      done
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
      exit 0
    fi
    printf 'No Sockets found in /tmp/test.\n'
    exit 1 ;;
  status_log_transitions)
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  pauseall|pauseall_zh)
    case "$1" in
      -list)
        printf 'There are screens on:\n\t123.lookup_duck\t(Detached)\n\t222.lookup_falcon\t(Detached)\n\t456.lookup_goose\t(Dead ???)\n\t789.lookup_crane\t(Detached)\n\t999.lookup_owl\t(Detached)\n\t888.lookup_raven\t(Detached)\n'
        exit 0 ;;
      -S)
        if [ "$3" != "-X" ] || [ "$4" != "stuff" ]; then exit 2; fi
        printf '%s:%s' "$2" "$5" >> "$FISHBOT_SCREEN_COUNTER"
        case "$2" in
          lookup_duck)
            case "$5" in
              /pause*) echo_line='Fishing paused' ;;
              /unpause*) echo_line='Fishing resumed' ;;
              *) exit 2 ;;
            esac
            if [ -f "$FISHBOT_SCREEN_COUNTER.rotate" ]; then
              printf '%s\n' "$echo_line" > "$FISHBOT_BOT_PATH/logs/9999-12-31-23-59-59.log"
            else
              printf '%s\n' "$echo_line" >> "$FISHBOT_BOT_PATH/logs/9999-12-31-23-59-59.log"
            fi ;;
          lookup_goose) exit 1 ;;
          lookup_falcon|lookup_owl) ;;
          lookup_raven)
            case "$5" in
              /pause*) echo_line='Fishing resumed' ;;
              /unpause*) echo_line='Fishing paused' ;;
              *) exit 2 ;;
            esac
            printf '%s\n' "$echo_line" >> "$FISHBOT_BOT_PATH/../raven/logs/9999-12-31-23-59-59.log" ;;
          *) exit 2 ;;
        esac
        exit 0 ;;
      *) exit 2 ;;
    esac ;;
  pauseall_empty) printf 'No Sockets found in /tmp/test.\n'; exit 1 ;;
  pauseall_recheck_error)
    if [ "$1" != "-list" ]; then exit 2; fi
    if [ -f "$FISHBOT_SCREEN_COUNTER" ]; then
      printf 'simulated recheck error\n' >&2
      exit 1
    fi
    : > "$FISHBOT_SCREEN_COUNTER"
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  routing_permissions)
    printf x >> "$FISHBOT_SCREEN_COUNTER"
    printf 'There is a screen on:\n\t123.lookup_owl\t(Detached)\n'
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
  restart_no_token)
    if [ "$1" != "-list" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.unexpected"
      exit 2
    fi
    printf 'There is a screen on:\n\t123.lookup_crane\t(Detached)\n'
    exit 0 ;;
  restart_success|restart_success_zh|restartall_success|restart_unconfirmed|restart_unconfirmed_zh|restartall_unconfirmed)
    if [ "$1" = "-dmS" ]; then
      case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
        *unconfirmed*)
          for file in "$FISHBOT_BOT_PATH"/logs/*.log; do printf 'initializing\n' > "$file"; done ;;
        *)
          for file in "$FISHBOT_BOT_PATH"/logs/*.log; do printf 'Welcome to the xxx server\n' >> "$file"; done ;;
      esac
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ "$1" = "-X" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.stopping"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ] && [ ! -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf 'No Sockets found in /tmp/test.\n'
      exit 1
    fi
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  restart_start_failed)
    if [ "$1" = "-dmS" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ "$1" = "-X" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.stopping"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ]; then
      printf 'No Sockets found in /tmp/test.\n'
      exit 1
    fi
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    exit 0 ;;
  start_dead|restart_dead|restartall_dead)
    if [ "$1" = "-dmS" ]; then
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        printf 'Welcome to the xxx server\n' >> "$file"
      done
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ "$1" = "-X" ]; then
      : > "$FISHBOT_SCREEN_COUNTER.stopping"
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      if [ -f "$FISHBOT_SCREEN_COUNTER.verified" ]; then
        printf 'There is a screen on:\n\t123.lookup_duck\t(Dead ???)\n'
      else
        : > "$FISHBOT_SCREEN_COUNTER.verified"
        printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
      fi
      exit 0
    fi
    if [ "$FISHBOT_QUERY_FAILURE_TEST_CHILD" = "start_dead" ] || [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ]; then
      printf 'No Sockets found in /tmp/test.\n'
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
        "game_config",
        "checkserver",
        "status",
        "startall_seconds",
        "start_and_status_online",
        "status_log_transitions",
        "pauseall",
        "pauseall_zh",
        "pauseall_empty",
        "pauseall_recheck_error",
        "anti_afk_timestamp",
        "failure",
        "empty",
        "missing",
        "race",
        "verify_failure",
        "stop_failure",
        "restart_failure",
        "restart_no_token",
        "restart_success",
        "restart_success_zh",
        "restart_unconfirmed",
        "restart_unconfirmed_zh",
        "restartall_success",
        "restartall_unconfirmed",
        "restart_start_failed",
        "start_dead",
        "restart_dead",
        "restartall_dead",
        "login_failure",
        "login_await_failure",
        "login_recover",
        "routing_permissions",
    ] {
        if mode.ends_with("_zh") {
            value["i18n"]["language"] = toml::Value::String("CHINESE_SIMPLIFIED".into());
            std::fs::write(
                root.join(config::CONFIG_FILE_NAME),
                toml::to_string(&value).unwrap(),
            )
            .unwrap();
        }
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
        if mode.ends_with("_zh") {
            value["i18n"]["language"] = toml::Value::String("ENGLISH".into());
            std::fs::write(
                root.join(config::CONFIG_FILE_NAME),
                toml::to_string(&value).unwrap(),
            )
            .unwrap();
        }
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
