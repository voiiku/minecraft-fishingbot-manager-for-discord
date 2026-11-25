//! screen 查询、登录会话状态与反 AFK 的回归测试。

use minecraft_fishingbot_manager_for_discord::operations;
use minecraft_fishingbot_manager_for_discord::text::report::IntoReplyText;
mod common;

use minecraft_fishingbot_manager_for_discord::runtime::bot_instances::bot_sessions::token_ready_bot_names_with_screen_sessions;
use minecraft_fishingbot_manager_for_discord::runtime::bot_instances::screen::is_bot_screen_running;
use minecraft_fishingbot_manager_for_discord::runtime::shutdown;
use minecraft_fishingbot_manager_for_discord::{config, regenerate_fishingbot_config, text};

#[test]
fn screen_queries_login_and_anti_afk() {
    const CHILD: &str = "FISHBOT_QUERY_FAILURE_TEST_CHILD";
    const ADMIN: u64 = 123_456_789_012_345_678;

    if let Some(mode) = std::env::var_os(CHILD) {
        config::Config::load().expect("load test configuration");
        if mode != "startup_boundary_zh" {
            minecraft_fishingbot_manager_for_discord::complete_startup();
        }
        let i18n = minecraft_fishingbot_manager_for_discord::config::Config::get().text();
        match mode.to_str().unwrap() {
            "signal_without_context" => {
                minecraft_fishingbot_manager_for_discord::ProcessLog::get_or_init();
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(shutdown::exit_on_shutdown_signal(None));
            }
            "permanent_failure" | "permanent_failure_cleanup_error" => {
                shutdown::exit_on_permanent_failure("test permanent failure");
            }
            "permanent_failure_logged" => {
                minecraft_fishingbot_manager_for_discord::ProcessLog::get_or_init();
                shutdown::exit_on_permanent_failure("test permanent failure");
            }
            "permanent_failure_logger_disabled" => {
                minecraft_fishingbot_manager_for_discord::ProcessLog::get_or_init();
                log::set_max_level(log::LevelFilter::Off);
                shutdown::exit_on_permanent_failure("test permanent failure");
            }
            "startup_boundary_zh" => {
                let english = text::all_languages()[1];
                assert_eq!(
                    i18n.discord_connected(1),
                    text::all_languages()[0].discord_connected(1)
                );
                let failure = "Unauthorized";
                assert_eq!(
                    minecraft_fishingbot_manager_for_discord::bootstrap_error_text()
                        .discord_init_failed_bad_token(failure),
                    english.discord_init_failed_bad_token(failure)
                );
                let error = is_bot_screen_running("duck").expect_err("screen must fail");
                assert!(
                    error.starts_with("Failed to list screen sessions:"),
                    "{error}"
                );

                minecraft_fishingbot_manager_for_discord::complete_startup();
                assert_eq!(
                    minecraft_fishingbot_manager_for_discord::bootstrap_error_text()
                        .discord_init_failed_bad_token(failure),
                    i18n.discord_init_failed_bad_token(failure)
                );
                let error = is_bot_screen_running("duck").expect_err("screen must fail");
                assert!(error.starts_with("查询 screen 会话失败:"), "{error}");

                minecraft_fishingbot_manager_for_discord::complete_startup();
                assert_eq!(
                    minecraft_fishingbot_manager_for_discord::bootstrap_error_text()
                        .discord_init_failed_bad_token(failure),
                    i18n.discord_init_failed_bad_token(failure)
                );
            }
            "game_config" => {
                let base_dir = &config::Config::get().paths.base_dir;
                let legacy_path = base_dir.join("conf.json");
                std::fs::write(&legacy_path, "legacy config").unwrap();
                regenerate_fishingbot_config().expect("generate game config");

                let view = operations::queries::show_bot_config().expect("read game config");
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
                let reply = runtime
                    .block_on(operations::queries::check_server())
                    .to_reply_text();
                assert!(reply.contains(i18n.checkserver_title()), "{reply}");
                assert!(reply.contains("CPU usage:"), "{reply}");
                assert!(reply.contains("Memory used/total:"), "{reply}");
                assert!(reply.contains("Disk usage:"), "{reply}");
                assert!(reply.contains("Host uptime:"), "{reply}");
            }
            "status" => {
                let reply = operations::queries::show_status().to_reply_text();
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
                let mut names = token_ready_bot_names_with_screen_sessions().unwrap();
                names.sort();
                assert_eq!(names, ["duck", "goose"]);
            }
            "startall_seconds" => {
                let reply = operations::all_bots_control::start_all_bots(ADMIN).to_reply_text();
                assert!(
                    reply.contains(
                        "1 seconds between bots in a batch, 0.001 seconds between batches"
                    ),
                    "{reply}"
                );
            }
            "start_and_status_online" => {
                let reply =
                    operations::single_bot_control::start_single_bot("duck", ADMIN).to_reply_text();
                assert_eq!(reply, i18n.start_join_confirmed("duck"));

                let status = operations::queries::show_status().to_reply_text();
                assert!(status.contains(i18n.bot_status_joined()), "{status}");
            }
            "start_status_unavailable" | "start_status_timeout" => {
                let reply =
                    operations::single_bot_control::start_single_bot("duck", ADMIN).to_reply_text();
                let expected = if mode == "start_status_unavailable" {
                    i18n.bot_status_unavailable().to_string()
                } else {
                    i18n.bot_status_query_timeout("duck")
                };
                assert!(reply.contains(&expected), "{reply}");
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(std::path::Path::new(&format!("{counter}.launched")).exists());
                assert!(!std::path::Path::new(&format!("{counter}.stopping")).exists());
            }
            "status_transitions" => {
                let logs_dir = config::Config::get()
                    .paths
                    .bots_dir
                    .join(ADMIN.to_string())
                    .join("duck")
                    .join(&config::Config::get().runtime.bot_logs_dir);
                std::fs::create_dir_all(&logs_dir).unwrap();
                let log_path = logs_dir.join("9999-12-31-23-59-59.log");
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                for (fields, expected_connection, expected_pause, no_rod, bobber, last_fish) in [
                    (
                        "false true true false 123",
                        i18n.bot_status_not_joined(),
                        i18n.bot_pause_paused(),
                        "true",
                        "false",
                        "123",
                    ),
                    (
                        "true false false true 456",
                        i18n.bot_status_joined(),
                        i18n.bot_pause_unpaused(),
                        "false",
                        "true",
                        "456",
                    ),
                ] {
                    std::fs::write(&log_path, "old log\n").unwrap();
                    std::fs::write(format!("{counter}.state"), fields).unwrap();
                    let status = operations::queries::show_status().to_reply_text();
                    let row = status
                        .lines()
                        .find(|line| line.starts_with("- duck |"))
                        .unwrap();
                    let columns: Vec<_> = row.split(" | ").collect();
                    assert_eq!(columns.len(), 7, "{row}");
                    assert_eq!(
                        &columns[1..6],
                        [
                            expected_connection,
                            expected_pause,
                            no_rod,
                            bobber,
                            last_fish
                        ]
                    );
                }
                std::fs::write(format!("{counter}.state"), "unavailable").unwrap();
                let status = operations::queries::show_status().to_reply_text();
                assert!(status.contains(i18n.bot_status_unavailable()), "{status}");
                std::fs::remove_file(log_path).unwrap();
            }
            "pauseall" | "pauseall_zh" => {
                let owner = config::Config::get().paths.bots_dir.join(ADMIN.to_string());
                for (name, contents) in [
                    ("duck", "initial\n"),
                    (
                        "owl",
                        "[2026] [INFO   ] Fishing status: paused=true joined=true no_rod_available=false has_bobber=false last_fish=0\n",
                    ),
                    ("goose", "initial\n"),
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
                    operations::all_bots_control::pause_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::unpause_all_bots
                        as fn(u64) -> Result<String, String>,
                ] {
                    assert_eq!(
                        action(987_654_322).to_reply_text(),
                        i18n.admin_required(987_654_322)
                    );
                }
                assert!(!std::path::Path::new(&counter).exists());

                for (action, command, title, confirmed, status_label, other_status) in [
                    (
                        operations::all_bots_control::pause_all_bots
                            as fn(u64) -> Result<String, String>,
                        "/pause",
                        i18n.pauseall_title(),
                        i18n.bot_pause_confirmed("duck"),
                        i18n.bot_pause_paused(),
                        i18n.bot_pause_unpaused(),
                    ),
                    (
                        operations::all_bots_control::unpause_all_bots
                            as fn(u64) -> Result<String, String>,
                        "/unpause",
                        i18n.unpauseall_title(),
                        i18n.bot_unpause_confirmed("duck"),
                        i18n.bot_pause_unpaused(),
                        i18n.bot_pause_paused(),
                    ),
                ] {
                    let reply = action(ADMIN).to_reply_text();
                    let timeout = i18n.bot_status_query_after_command_failed(
                        command,
                        &i18n.bot_status_query_timeout("owl"),
                    );
                    let missing_log = i18n.bot_status_log_not_found("falcon");
                    let send_failed = i18n.send_command_failed("goose", None);
                    let superseded = i18n.bot_line(
                        "raven",
                        &i18n.bot_pause_status_mismatch(command, other_status),
                    );
                    assert_eq!(
                        reply,
                        format!(
                            "{title}:\n=======================\n- {}\n- {}\n- {}\n\n{}:\n- {confirmed}\n\n{}:\n- {missing_log}\n- {send_failed}\n- {timeout}\n- {superseded}",
                            i18n.count_line(i18n.count_running_accounts(), 5),
                            i18n.count_line(i18n.count_processed_successfully(), 1),
                            i18n.count_line(i18n.count_processing_failed(), 4),
                            i18n.report_succeeded_label(),
                            i18n.report_failed_label()
                        )
                    );

                    let status = operations::queries::show_status().to_reply_text();
                    let duck = status.lines().find(|line| line.contains("duck |")).unwrap();
                    let owl = status.lines().find(|line| line.contains("owl |")).unwrap();
                    let raven = status
                        .lines()
                        .find(|line| line.contains("raven |"))
                        .unwrap();
                    assert!(duck.contains(status_label), "{duck}");
                    assert!(owl.contains(i18n.bot_status_field_unknown()), "{owl}");
                    assert!(raven.contains(other_status), "{raven}");
                }
                let sent = std::fs::read_to_string(&counter).unwrap();
                for command in ["/pause", "/unpause"] {
                    for name in ["duck", "goose", "owl", "raven"] {
                        assert!(
                            sent.contains(&format!("lookup_{name}:{command}\n")),
                            "{sent}"
                        );
                    }
                }
                std::fs::write(format!("{}.duck", counter.to_string_lossy()), "true\n").unwrap();
                std::fs::write(format!("{}.owl_unavailable", counter.to_string_lossy()), "")
                    .unwrap();
                let refreshed = operations::queries::show_status().to_reply_text();
                let duck = refreshed
                    .lines()
                    .find(|line| line.contains("duck |"))
                    .unwrap();
                let owl = refreshed
                    .lines()
                    .find(|line| line.contains("owl |"))
                    .unwrap();
                assert!(duck.contains(i18n.bot_pause_paused()), "{duck}");
                assert!(owl.contains(i18n.bot_status_field_unavailable()), "{owl}");
                assert_eq!(
                    operations::single_bot_control::pause_single_bot("owl", ADMIN).to_reply_text(),
                    i18n.bot_pause_query_unavailable("owl", "/pause")
                );
                let duck_log = owner
                    .join("duck")
                    .join("logs")
                    .join("9999-12-31-23-59-59.log");
                std::fs::write(&duck_log, "old log ".repeat(100)).unwrap();
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                std::fs::write(format!("{counter}.rotate"), "").unwrap();
                let result =
                    operations::single_bot_control::pause_single_bot("duck", ADMIN).to_reply_text();
                assert!(
                    result.contains(&i18n.bot_status_log_changed("duck")),
                    "{result}"
                );
                std::fs::remove_dir_all(owner.join("owl")).unwrap();
                std::fs::remove_dir_all(owner.join("raven")).unwrap();
                std::fs::remove_dir_all(owner.join("falcon")).unwrap();
                std::fs::remove_dir_all(owner.join("swan")).unwrap();
                std::fs::remove_dir_all(owner.join("goose").join("logs")).unwrap();
                std::fs::remove_file(duck_log).unwrap();
            }
            "pauseall_empty" => {
                for action in [
                    operations::all_bots_control::pause_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::unpause_all_bots
                        as fn(u64) -> Result<String, String>,
                ] {
                    let reply = action(ADMIN).to_reply_text();
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
                let reply = operations::all_bots_control::pause_all_bots(ADMIN).to_reply_text();
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
                    operations::queries::show_status().to_reply_text(),
                    operations::single_bot_control::start_single_bot("duck", ADMIN).to_reply_text(),
                    operations::single_bot_control::stop_single_bot("duck", ADMIN).to_reply_text(),
                    operations::single_bot_control::restart_single_bot("duck", ADMIN)
                        .to_reply_text(),
                    operations::single_bot_control::send_single_bot_command(
                        "duck",
                        &["/help"],
                        ADMIN,
                    )
                    .to_reply_text(),
                    operations::accounts::delete_bot("duck", ADMIN).to_reply_text(),
                    operations::all_bots_control::start_all_bots(ADMIN).to_reply_text(),
                    operations::all_bots_control::stop_all_bots(ADMIN).to_reply_text(),
                    operations::all_bots_control::restart_all_bots(ADMIN).to_reply_text(),
                    operations::all_bots_control::pause_all_bots(ADMIN).to_reply_text(),
                    operations::all_bots_control::unpause_all_bots(ADMIN).to_reply_text(),
                    operations::all_bots_control::send_all_bots_command(&["/help"], ADMIN)
                        .to_reply_text(),
                    operations::accounts::begin_bot_login("duck", ADMIN).unwrap_err(),
                ];
                for reply in replies {
                    assert!(reply.contains(expected), "{reply}");
                }
            }
            "empty" => {
                assert!(!is_bot_screen_running("duck").expect("no sessions is not an error"));
            }
            "missing" => {
                let error =
                    is_bot_screen_running("duck").expect_err("missing screen must be reported");
                assert!(error.contains("screen"), "{error}");
            }
            "anti_afk_timestamp" => {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    assert_eq!(
                        operations::set_anti_afk(true, ADMIN).await.to_reply_text(),
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
                        operations::set_anti_afk(false, ADMIN).await.to_reply_text(),
                        i18n.anti_afk_stopped()
                    );
                });
            }
            "race" => {
                assert_eq!(
                    operations::accounts::begin_bot_login("duck", ADMIN).unwrap_err(),
                    i18n.refusal_bot_running("duck")
                );
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();
                assert_eq!(std::fs::read_to_string(counter).unwrap(), "x");
            }
            "verify_failure" => {
                let reply =
                    operations::single_bot_control::start_single_bot("duck", ADMIN).to_reply_text();
                assert!(reply.contains("verification screen error"), "{reply}");
            }
            "stop_failure" => {
                let reply =
                    operations::single_bot_control::stop_single_bot("duck", ADMIN).to_reply_text();
                assert!(reply.contains("stop screen error"), "{reply}");
            }
            "restart_failure" => {
                let reply = operations::single_bot_control::restart_single_bot("duck", ADMIN)
                    .to_reply_text();
                assert!(reply.contains("stop screen error"), "{reply}");
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(
                    !std::path::Path::new(&format!("{counter}.launched")).exists(),
                    "restart must not start a bot after the stop cannot be verified"
                );
            }
            "restart_no_token" => {
                let reply = operations::single_bot_control::restart_single_bot("crane", ADMIN)
                    .to_reply_text();
                assert_eq!(reply, i18n.restart_skipped_missing_token("crane"));
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(!std::path::Path::new(&format!("{counter}.unexpected")).exists());
            }
            "restartall_already_running" => {
                let reply = operations::all_bots_control::restart_all_bots(ADMIN).to_reply_text();
                assert!(
                    reply.contains(&i18n.count_line(i18n.count_restart_verified(), 0)),
                    "{reply}"
                );
                assert!(
                    reply.contains(&i18n.count_line(i18n.count_restart_failed(), 1)),
                    "{reply}"
                );
                assert!(
                    reply.contains(&i18n.startall_skipped_running("duck")),
                    "{reply}"
                );
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
                    operations::all_bots_control::restart_all_bots(ADMIN).to_reply_text()
                } else {
                    operations::single_bot_control::restart_single_bot("duck", ADMIN)
                        .to_reply_text()
                };
                if is_batch {
                    assert!(reply.contains(i18n.restartall_title()), "{reply}");
                    let expected = if confirmed {
                        i18n.start_join_confirmed("duck")
                    } else {
                        i18n.start_not_joined("duck")
                    };
                    assert!(reply.contains(&expected), "{reply}");
                } else {
                    let expected = if confirmed {
                        i18n.bot_restart_online_confirmed("duck")
                    } else {
                        i18n.bot_restart_not_joined("duck")
                    };
                    assert_eq!(reply, expected);
                }
                let counter = std::env::var("FISHBOT_SCREEN_COUNTER").unwrap();
                assert!(std::path::Path::new(&format!("{counter}.stopping")).exists());
                assert!(std::path::Path::new(&format!("{counter}.launched")).exists());
            }
            "restart_start_failed" | "start_dead" | "restart_dead" | "restartall_dead" => {
                let reply = match mode.to_str().unwrap() {
                    "start_dead" => operations::single_bot_control::start_single_bot("duck", ADMIN)
                        .to_reply_text(),
                    "restartall_dead" => {
                        operations::all_bots_control::restart_all_bots(ADMIN).to_reply_text()
                    }
                    _ => operations::single_bot_control::restart_single_bot("duck", ADMIN)
                        .to_reply_text(),
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
                let sync = operations::begin_command_sync(ADMIN).expect("admin may synchronize");
                assert_eq!(
                    operations::begin_command_sync(OTHER).unwrap_err(),
                    i18n.admin_required(OTHER)
                );
                assert_eq!(
                    operations::begin_command_sync(ADMIN).unwrap_err(),
                    i18n.command_sync_in_progress()
                );
                drop(sync);
                let bot = config::Config::get()
                    .paths
                    .bots_dir
                    .join(OWNER.to_string())
                    .join("owl");
                std::fs::create_dir_all(&bot).unwrap();
                let counter = std::env::var_os("FISHBOT_SCREEN_COUNTER").unwrap();

                for action in [
                    operations::all_bots_control::start_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::stop_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::restart_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::pause_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::unpause_all_bots
                        as fn(u64) -> Result<String, String>,
                    |caller| {
                        operations::all_bots_control::send_all_bots_command(&["/help"], caller)
                    },
                ] {
                    assert_eq!(action(OTHER).to_reply_text(), i18n.admin_required(OTHER));
                }
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    for enabled in [true, false] {
                        assert_eq!(
                            operations::set_anti_afk(enabled, OTHER)
                                .await
                                .to_reply_text(),
                            i18n.admin_required(OTHER)
                        );
                    }
                });
                assert!(
                    !std::path::Path::new(&counter).exists(),
                    "unauthorized actions must not query screen"
                );

                let not_found = i18n.bot_dir_not_found("owl");
                for operation in [
                    operations::single_bot_control::start_single_bot,
                    operations::single_bot_control::stop_single_bot,
                    operations::single_bot_control::restart_single_bot,
                    operations::single_bot_control::pause_single_bot,
                    operations::single_bot_control::unpause_single_bot,
                ] {
                    assert_eq!(operation("owl", OTHER).to_reply_text(), not_found);
                }
                assert_eq!(
                    operations::single_bot_control::send_single_bot_command(
                        "owl",
                        &["/help"],
                        OTHER
                    )
                    .to_reply_text(),
                    not_found
                );
                assert_eq!(
                    operations::accounts::delete_bot("owl", OTHER).to_reply_text(),
                    not_found
                );
                assert_eq!(
                    operations::queries::read_bot_log("owl", Some(-1), OTHER).unwrap_err(),
                    not_found
                );
                assert_eq!(
                    operations::accounts::begin_bot_login("owl", OTHER).unwrap_err(),
                    not_found
                );
                assert_eq!(
                    operations::accounts::rename_bot("owl", "other", OTHER).to_reply_text(),
                    not_found
                );
                assert_eq!(
                    operations::accounts::update_bot_token("owl", "test".into(), OTHER)
                        .to_reply_text(),
                    not_found
                );
                assert!(!std::path::Path::new(&counter).exists());
                assert!(bot.exists(), "unauthorized delete must not remove the bot");

                for caller in [OWNER, ADMIN] {
                    assert_eq!(
                        operations::single_bot_control::start_single_bot("owl", caller)
                            .to_reply_text(),
                        i18n.bot_start_missing_token("owl")
                    );
                    assert_eq!(
                        operations::accounts::delete_bot("owl", caller).to_reply_text(),
                        i18n.refusal_bot_running("owl")
                    );
                    assert_eq!(
                        operations::queries::read_bot_log("owl", Some(1), caller).unwrap_err(),
                        i18n.bot_log_missing("owl")
                    );
                    assert_eq!(
                        operations::accounts::begin_bot_login("owl", caller).unwrap_err(),
                        i18n.bot_login_already_running("owl")
                    );
                }
            }
            "login_failure" | "login_await_failure" | "login_recover" => {
                let result = operations::accounts::begin_bot_login("duck", ADMIN);
                let reply = if mode == "login_await_failure" {
                    operations::accounts::finish_bot_login(
                        result.expect("device code should be found"),
                    )
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
        root.join(common::TEST_CONFIG_FILE_NAME),
        toml::to_string(&value).unwrap(),
    )
    .unwrap();

    let screen = bin.join("screen");
    std::fs::write(
        &screen,
        r#"#!/bin/sh
case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
  status)
    if [ "$1" = "-S" ]; then
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        [ -f "$file" ] && printf '[2026] [INFO   ] Fishing status: unavailable\n' >> "$file"
      done
      exit 0
    fi
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
  start_and_status_online|start_status_unavailable|start_status_timeout)
    if [ "$1" = "-dmS" ]; then
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        printf 'Welcome to the xxx server\n' >> "$file"
      done
      : > "$FISHBOT_SCREEN_COUNTER.launched"
      exit 0
    fi
    if [ "$1" = "-S" ]; then
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        [ -f "$file" ] || continue
        case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
          start_status_unavailable) printf '[2026] [INFO   ] Fishing status: unavailable\n' >> "$file" ;;
          start_status_timeout) : ;;
          *) printf '[2026] [INFO   ] Fishing status: paused=false joined=true no_rod_available=false has_bobber=true last_fish=42\n' >> "$file" ;;
        esac
      done
      exit 0
    fi
    if [ -f "$FISHBOT_SCREEN_COUNTER.launched" ]; then
      printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
      exit 0
    fi
    printf 'No Sockets found in /tmp/test.\n'
    exit 1 ;;
  status_transitions)
    if [ "$1" = "-S" ]; then
      read -r joined paused no_rod bobber last_fish < "$FISHBOT_SCREEN_COUNTER.state"
      if [ "$joined" = unavailable ]; then
        printf '[2026] [INFO   ] Fishing status: unavailable\n' >> "$FISHBOT_BOT_PATH/logs/9999-12-31-23-59-59.log"
      else
        printf '[2026] [INFO   ] Fishing status: paused=%s joined=%s no_rod_available=%s has_bobber=%s last_fish=%s\n' "$paused" "$joined" "$no_rod" "$bobber" "$last_fish" >> "$FISHBOT_BOT_PATH/logs/9999-12-31-23-59-59.log"
      fi
      exit 0
    fi
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
          lookup_goose) exit 1 ;;
          lookup_falcon) ;;
          lookup_owl)
            if [ -f "$FISHBOT_SCREEN_COUNTER.owl_unavailable" ]; then
              case "$5" in
                /status*)
                  if [ "$FISHBOT_QUERY_FAILURE_TEST_CHILD" = pauseall_zh ]; then
                    echo_line='钓鱼状态：不可用'
                  else
                    echo_line='Fishing status: unavailable'
                  fi
                  printf '[2026] [INFO   ] %s\n' "$echo_line" >> "$FISHBOT_BOT_PATH/../owl/logs/9999-12-31-23-59-59.log" ;;
              esac
            fi ;;
          lookup_duck|lookup_raven)
            name="${2#lookup_}"
            case "$5" in
              /pause*)
                if [ "$name" = duck ]; then state=true; else state=false; fi
                printf '%s\n' "$state" > "$FISHBOT_SCREEN_COUNTER.$name" ;;
              /unpause*)
                if [ "$name" = duck ]; then state=false; else state=true; fi
                printf '%s\n' "$state" > "$FISHBOT_SCREEN_COUNTER.$name" ;;
              /status*)
                state=false
                if [ -f "$FISHBOT_SCREEN_COUNTER.$name" ]; then
                  read -r state < "$FISHBOT_SCREEN_COUNTER.$name"
                fi
                if [ "$FISHBOT_QUERY_FAILURE_TEST_CHILD" = pauseall_zh ]; then
                  prefix='钓鱼状态：'
                else
                  prefix='Fishing status: '
                fi
                log="$FISHBOT_BOT_PATH/../$name/logs/9999-12-31-23-59-59.log"
                if [ -f "$FISHBOT_SCREEN_COUNTER.rotate" ]; then
                  : > "$log"
                fi
                printf '[2026] [INFO   ] %spaused=%s joined=true no_rod_available=false has_bobber=true last_fish=0\n' "$prefix" "$state" >> "$log" ;;
              *) exit 2 ;;
            esac ;;
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
  permanent_failure|permanent_failure_logged|permanent_failure_logger_disabled|signal_without_context)
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
  permanent_failure_cleanup_error) printf 'simulated cleanup failure\n' >&2; exit 1 ;;
  failure|startup_boundary_zh) printf 'simulated screen query error\n' >&2; exit 1 ;;
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
  restartall_already_running)
    case "$1" in
      -X) : > "$FISHBOT_SCREEN_COUNTER.stopping"; exit 0 ;;
      -list)
        if [ -f "$FISHBOT_SCREEN_COUNTER.stopping" ] && [ ! -f "$FISHBOT_SCREEN_COUNTER.stopped" ]; then
          if [ -f "$FISHBOT_SCREEN_COUNTER.grace" ]; then
            : > "$FISHBOT_SCREEN_COUNTER.stopped"
          else
            : > "$FISHBOT_SCREEN_COUNTER.grace"
          fi
          printf 'No Sockets found in /tmp/test.\n'; exit 1
        fi
        printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'; exit 0 ;;
      *) exit 2 ;;
    esac ;;
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
    if [ "$1" = "-S" ]; then
      case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
        *unconfirmed*) joined=false ;;
        *) joined=true ;;
      esac
      case "$FISHBOT_QUERY_FAILURE_TEST_CHILD" in
        *_zh) prefix='钓鱼状态：' ;;
        *) prefix='Fishing status: ' ;;
      esac
      for file in "$FISHBOT_BOT_PATH"/logs/*.log; do
        [ -f "$file" ] && printf '[2026] [INFO   ] %spaused=false joined=%s no_rod_available=false has_bobber=false last_fish=0\n' "$prefix" "$joined" >> "$file"
      done
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
        "start_status_unavailable",
        "start_status_timeout",
        "status_transitions",
        "pauseall",
        "pauseall_zh",
        "pauseall_empty",
        "pauseall_recheck_error",
        "anti_afk_timestamp",
        "failure",
        "startup_boundary_zh",
        "empty",
        "missing",
        "race",
        "verify_failure",
        "stop_failure",
        "restart_failure",
        "restart_no_token",
        "restartall_already_running",
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
        "permanent_failure",
        "permanent_failure_cleanup_error",
        "permanent_failure_logged",
        "permanent_failure_logger_disabled",
        "signal_without_context",
    ] {
        if mode.ends_with("_zh") {
            value["i18n"]["language"] = toml::Value::String("CHINESE_SIMPLIFIED".into());
            std::fs::write(
                root.join(common::TEST_CONFIG_FILE_NAME),
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
                root.join(common::TEST_CONFIG_FILE_NAME),
                toml::to_string(&value).unwrap(),
            )
            .unwrap();
        }
        let success = if mode.starts_with("permanent_failure") {
            let report_written = mode != "permanent_failure_logged"
                || std::fs::read_dir(root.join("logs"))
                    .unwrap()
                    .flatten()
                    .any(|entry| {
                        std::fs::read_to_string(entry.path())
                            .is_ok_and(|content| content.contains("test permanent failure"))
                    });
            let cleanup_observed = if mode == "permanent_failure_cleanup_error" {
                String::from_utf8_lossy(&output.stderr).contains("simulated cleanup failure")
            } else {
                root.join(format!("counter-{mode}.stopping")).is_file()
            };
            output.status.code() == Some(1)
                && String::from_utf8_lossy(&output.stderr).contains("test permanent failure")
                && cleanup_observed
                && report_written
        } else if mode == "signal_without_context" {
            let text = text::Language::English.implementation();
            let stderr = String::from_utf8_lossy(&output.stderr);
            output.status.code() == Some(0)
                && root.join(format!("counter-{mode}.stopping")).is_file()
                && stderr.contains(&text.shutdown_signal_report_title())
                && stderr.contains(&text.shutdown_report_no_context())
        } else {
            output.status.success()
        };
        if !success {
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
