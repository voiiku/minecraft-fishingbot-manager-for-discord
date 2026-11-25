//! 公开文本与启动提示的集成测试。

mod common;

use std::time::Duration;

use minecraft_fishingbot_manager_for_discord::text::{
    self, CommandTextProblem, Language, bootstrap,
};

#[test]
fn configured_durations_are_reported_in_seconds_without_losing_precision() {
    let languages = text::all_languages();
    let bot_interval = Duration::from_millis(1);
    let batch_interval = Duration::from_millis(1_250);
    assert_eq!(
        languages[0].command_start_queue_summary(2, bot_interval, batch_interval),
        "启动队列设置: 每批最多启动 2 台，批内间隔 0.001 秒，批间隔 1.25 秒"
    );
    assert_eq!(
        languages[1].command_start_queue_summary(2, bot_interval, batch_interval),
        "Start queue: up to 2 per batch, 0.001 seconds between bots in a batch, 1.25 seconds between batches"
    );
    assert_eq!(
        languages[0].ping_timeout("example.com", 25565, Duration::from_millis(300)),
        "连接服务器 (example.com:25565) 超时(0.3 秒无响应)"
    );
    assert_eq!(
        languages[1].ping_timeout("example.com", 25565, Duration::from_secs(3)),
        "Timed out connecting to server (example.com:25565) (3 seconds without a response)"
    );

    for language in languages {
        let watchdog = language.watchdog_ready(
            Duration::from_millis(1_001),
            Duration::from_millis(333),
            Duration::from_millis(2_002),
        );
        for value in ["1.001", "0.333", "2.002"] {
            assert!(watchdog.contains(value), "{watchdog}");
        }
        let scheduled = language.reconnect_scheduled(1, bot_interval);
        let failed = language.reconnect_failed(1, bot_interval, "test");
        let result = language.start_result_timeout("duck", batch_interval);
        let result_log = language.start_result_timeout_log_message("duck", batch_interval);
        assert!(scheduled.contains("0.001"), "{scheduled}");
        assert!(failed.contains("0.001"), "{failed}");
        assert!(result.contains("1.25"), "{result}");
        assert!(result_log.contains("1.25"), "{result_log}");
    }

    let large = Duration::from_secs(u64::MAX);
    let summary = languages[1].command_start_queue_summary(1, large, Duration::ZERO);
    assert!(
        summary.contains("18446744073709551615 seconds"),
        "{summary}"
    );
    assert!(summary.contains("0 seconds"), "{summary}");
    let summary = languages[1].command_start_queue_summary(
        1,
        Duration::new(u64::MAX, 1),
        Duration::from_millis(1_005),
    );
    assert!(
        summary.contains("18446744073709551615.000000001 seconds"),
        "{summary}"
    );
    assert!(summary.contains("1.005 seconds"), "{summary}");
}

#[test]
fn restart_messages_distinguish_joined_and_not_joined() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].bot_restart_online_confirmed("duck"),
        "机器人 duck: 重启后已确认进服"
    );
    assert_eq!(
        languages[0].bot_restart_not_joined("duck"),
        "机器人 duck: 重启后会话已存在，当前未进服（可稍后使用 /show status 查看）"
    );
    assert_eq!(
        languages[1].bot_restart_online_confirmed("duck"),
        "Bot duck: server join confirmed after restart"
    );
    assert_eq!(
        languages[1].bot_restart_not_joined("duck"),
        "Bot duck: session exists after restart; not currently joined (check /show status later)"
    );
}

#[test]
fn batch_pause_reports_do_not_claim_unconfirmed_pauses() {
    let languages = text::all_languages();
    let (chinese, english) = (languages[0], languages[1]);
    assert_eq!(chinese.pauseall_title(), "批量暂停钓鱼结果");
    assert_eq!(chinese.unpauseall_title(), "批量恢复钓鱼结果");
    assert_eq!(chinese.count_processed_successfully(), "处理完成");
    assert_eq!(chinese.report_succeeded_label(), "[完成明细]");
    assert_eq!(chinese.report_failed_label(), "[异常明细]");
    assert_eq!(
        chinese.bot_pause_query_unavailable("duck", "/pause"),
        "机器人 'duck': 已发送 /pause，状态不可用（会话异常）"
    );
    assert_eq!(english.pauseall_title(), "Pause-all result");
    assert_eq!(english.unpauseall_title(), "Unpause-all result");
    assert_eq!(english.count_processed_successfully(), "completed");
    assert_eq!(english.report_succeeded_label(), "[Completed]");
    assert_eq!(english.report_failed_label(), "[Issues]");
    assert_eq!(
        english.bot_pause_query_unavailable("duck", "/pause"),
        "Bot 'duck': /pause sent; status unavailable (session abnormal)"
    );
}

#[test]
fn login_and_timeout_messages_do_not_assert_unverified_outcomes() {
    let languages = text::all_languages();
    for language in languages {
        let prompt = language.bot_login_prompt("duck", "https://example.test", "ABCD", 7);
        assert!(prompt.contains("7"), "{prompt}");
        assert!(prompt.contains("ABCD"), "{prompt}");
        assert!(prompt.contains("https://example.test"), "{prompt}");
        assert!(
            language
                .bot_login_succeeded("duck")
                .contains("/show status")
        );
        assert!(language.bot_login_timed_out("duck", 10).contains("10"));
    }
    assert!(
        !languages[0]
            .bot_login_timed_out("duck", 10)
            .contains("已取消")
    );
    assert!(
        !languages[1]
            .bot_login_timed_out("duck", 10)
            .contains("cancelled")
    );
    assert!(!languages[0].bot_login_succeeded("duck").contains("删除"));
    assert!(!languages[1].bot_login_succeeded("duck").contains("removed"));
}

#[test]
fn startup_and_sync_labels_match_the_reported_outcomes() {
    let languages = text::all_languages();
    assert_eq!(languages[0].count_valid_accounts(), "已配置 Token");
    assert_eq!(languages[0].count_start_verified(), "启动复核通过");
    assert_eq!(languages[0].count_start_failed(), "启动异常");
    assert_eq!(languages[0].count_running_accounts(), "目标机器人");
    assert_eq!(languages[0].count_skipped_running(), "跳过（会话已存在）");
    assert!(
        languages[0]
            .bot_start_already_running("duck")
            .contains("screen 会话已存在")
    );
    assert!(
        languages[0]
            .start_result_timeout("duck", Duration::from_secs(5))
            .contains("状态未确认")
    );
    assert_eq!(
        languages[0].command_sync_guild_summary(1, 1, 0),
        "目标服务器 1 个；更新成功 1 个，更新失败 0 个"
    );
    assert_eq!(languages[1].count_start_failed(), "start issues");
    assert_eq!(
        languages[1].count_skipped_running(),
        "skipped (session exists)"
    );
    assert!(
        languages[1]
            .start_result_timeout("duck", Duration::from_secs(5))
            .contains("unconfirmed")
    );
    assert_eq!(
        languages[1].command_sync_guild_summary(1, 1, 0),
        "Target servers: 1; updated: 1, update failures: 0"
    );
}

#[test]
fn guild_sync_errors_include_the_server_and_reason() {
    for language in text::all_languages() {
        let error = language.command_sync_guild_failed("guild-12", "detail");
        assert!(error.contains("guild-12"), "{error}");
        assert!(error.contains("detail"), "{error}");
        assert!(
            language
                .command_sync_cleanup_skipped()
                .contains("/sync-commands")
        );
    }
}

#[test]
fn command_text_validation_errors_are_localized() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].command_text_invalid("start name", CommandTextProblem::Missing),
        "指令文案 start name 无效: 缺少说明"
    );
    assert_eq!(
        languages[1].command_text_invalid("start name", CommandTextProblem::Missing),
        "Invalid command text start name: description is missing"
    );
    for language in languages {
        let error = language.command_text_invalid(
            "start name",
            CommandTextProblem::InvalidLength {
                length: 102,
                max: 100,
            },
        );
        assert!(error.contains("start name"), "{error}");
        assert!(error.contains("102"), "{error}");
    }
}

#[test]
fn checkserver_query_errors_are_localized() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].checkserver_system_failed("detail"),
        "获取系统信息失败: detail"
    );
    assert_eq!(
        languages[1].checkserver_system_failed("detail"),
        "Failed to initialize system information: detail"
    );
    assert_eq!(
        languages[0].checkserver_disks_failed("detail"),
        "获取磁盘信息失败: detail"
    );
    assert_eq!(
        languages[1].checkserver_disks_failed("detail"),
        "Failed to query disk information: detail"
    );
    assert_eq!(
        languages[0].checkserver_uptime_failed("detail"),
        "获取系统运行时间失败: detail"
    );
    assert_eq!(
        languages[1].checkserver_uptime_failed("detail"),
        "Failed to query system uptime: detail"
    );
}

#[test]
fn screen_lookup_failure_is_localized_and_includes_reason() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].screen_list_failed("example"),
        "查询 screen 会话失败: example"
    );
    assert_eq!(
        languages[1].screen_list_failed("example"),
        "Failed to list screen sessions: example"
    );
}

#[test]
fn runtime_reasons_and_command_logs_are_localized() {
    let languages = text::all_languages();
    assert_eq!(languages[0].ping_no_address_reason(), "域名解析未返回地址");
    assert_eq!(languages[1].ping_no_address_reason(), "no address returned");
    assert_eq!(languages[0].ping_connection_failed_reason(), "连接失败");
    assert_eq!(
        languages[1].ping_connection_failed_reason(),
        "connection failed"
    );
    assert_eq!(
        languages[0].screen_unrecognized_output("example"),
        "无法识别的输出: example"
    );
    assert_eq!(
        languages[1].screen_unrecognized_output("example"),
        "unrecognized output: example"
    );

    for language in languages {
        let lookup = language.autocomplete_lookup_failed("detail");
        let task = language.autocomplete_task_failed("detail");
        assert_ne!(lookup, task);
        assert!(lookup.contains("detail"));
        assert!(task.contains("detail"));
        let fallback = language.command_fallback_reply_failed(123, "start", "detail");
        assert!(fallback.contains("123"), "{fallback}");
        assert!(fallback.contains("/start"), "{fallback}");
        assert!(fallback.contains("detail"), "{fallback}");
    }
    assert_eq!(
        languages[1].autocomplete_lookup_failed("detail"),
        "Autocomplete candidate lookup failed: detail"
    );
    assert_eq!(
        languages[1].autocomplete_task_failed("detail"),
        "Autocomplete candidate lookup task failed: detail"
    );
    assert_eq!(
        languages[1].command_fallback_reply_failed(123, "start", "detail"),
        "Failed to send the fallback response for /start to Discord user 123: detail"
    );
}

#[test]
fn queue_full_reason_is_localized() {
    let languages = text::all_languages();
    assert_eq!(languages[0].start_queue_full_reason(), "启动队列已满");
    assert_eq!(
        languages[1].start_queue_full_reason(),
        "start queue is full (max_pending_requests)"
    );
}

#[test]
fn anti_afk_messages_are_localized_and_preserve_error_details() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].anti_afk_stop_not_running(),
        "错误: 反 AFK 功能未在运行，无需停止"
    );
    assert_eq!(
        languages[1].anti_afk_stop_not_running(),
        "Error: Anti-AFK is not running; no need to stop"
    );
    for language in languages {
        assert!(!language.anti_afk_started().is_empty());
        assert!(!language.anti_afk_already_running().is_empty());
        assert!(!language.anti_afk_stopped().is_empty());
        assert_ne!(
            language.anti_afk_status(true),
            language.anti_afk_status(false)
        );
        for message in [
            language.anti_afk_lock_failed("test reason"),
            language.anti_afk_config_failed("test reason"),
            language.anti_afk_worker_failed("test reason"),
            language.anti_afk_start_failed("test reason"),
            language.anti_afk_stop_failed("test reason"),
        ] {
            assert!(message.contains("test reason"), "{message}");
            assert!(
                message.contains("反 AFK")
                    || message.contains("Anti-AFK")
                    || message.contains("anti-AFK")
            );
        }
    }
}

/// 合法语言名去除两侧空白后可解析；未知取值的错误包含输入值与全部合法取值。
#[test]
fn language_names_are_parsed_with_supported_values() {
    assert_eq!(
        Language::from_name("CHINESE_SIMPLIFIED"),
        Ok(Language::ChineseSimplified)
    );
    assert_eq!(Language::from_name(" ENGLISH "), Ok(Language::English));

    let error = Language::from_name("FRENCH").expect_err("FRENCH is not implemented");
    assert!(
        error.contains("FRENCH"),
        "error contains the input: {error}"
    );
    assert!(
        error.contains("ENGLISH") && error.contains("CHINESE_SIMPLIFIED"),
        "error lists the supported values: {error}"
    );
    assert!(
        error.contains("is not a supported language"),
        "bootstrap wording changed: {error}"
    );
}

/// `supported_names()` 列出的每个名称都能解析，并能通过 `name()` 返回原值。
#[test]
fn every_supported_language_name_round_trips() {
    for name in Language::supported_names() {
        let language = Language::from_name(name).expect("supported name must parse");
        assert_eq!(language.name(), *name);
    }
}

/// 每种语言都提供非空指令文案，且同一语言内的 `name_path` 不重复。
#[test]
fn every_language_exposes_unique_command_texts() {
    for language in text::all_languages() {
        let command_texts = language.command_texts();
        assert!(!command_texts.is_empty());

        let mut name_paths: Vec<&str> = command_texts.iter().map(|text| text.name_path).collect();
        let count = name_paths.len();
        name_paths.sort_unstable();
        name_paths.dedup();
        assert_eq!(name_paths.len(), count, "command name paths must be unique");
    }
}

/// 引导期错误文案包含出错值，便于部署者定位配置项。
#[test]
fn bootstrap_messages_include_the_problematic_value() {
    let unknown_language = bootstrap::language_unknown("GERMAN");
    assert!(unknown_language.contains("GERMAN"), "{unknown_language}");
    assert!(unknown_language.contains("ENGLISH"), "{unknown_language}");
    assert!(
        unknown_language.contains("CHINESE_SIMPLIFIED"),
        "{unknown_language}"
    );

    let parse_error = bootstrap::config_parse_failed("/etc/x.conf.toml", "unexpected character");
    assert!(parse_error.contains("/etc/x.conf.toml"), "{parse_error}");
    assert!(
        parse_error.contains("unexpected character"),
        "{parse_error}"
    );
}

/// 首次生成模板与读取已有配置的提示包含文件路径及后续操作。
#[test]
fn bootstrap_messages_describe_the_next_step() {
    let created = bootstrap::config_template_created("/opt/fishing/x.conf.toml");
    assert!(created.contains("/opt/fishing/x.conf.toml"), "{created}");
    assert!(created.contains("restart the program"), "{created}");

    let found = bootstrap::config_file_found("/opt/fishing/x.conf.toml");
    assert!(found.contains("/opt/fishing/x.conf.toml"), "{found}");
}

#[test]
fn chinese_config_errors_are_english_and_success_uses_configured_language() {
    const CHILD: &str = "FISHBOT_ENGLISH_CONFIG_ERROR_CHILD";
    if let Ok(mode) = std::env::var(CHILD) {
        if mode == "returned_error" {
            let error = minecraft_fishingbot_manager_for_discord::config::Config::load()
                .expect_err("invalid configuration must return an error");
            assert!(
                error.contains("config item anti_afk.input_delay"),
                "{error}"
            );
            assert!(
                std::panic::catch_unwind(
                    minecraft_fishingbot_manager_for_discord::config::Config::get
                )
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(|| {
                    minecraft_fishingbot_manager_for_discord::config::Config::get().text()
                })
                .is_err()
            );
            return;
        }
        minecraft_fishingbot_manager_for_discord::config::Config::load().unwrap_or_else(|message| {
            minecraft_fishingbot_manager_for_discord::runtime::shutdown::exit_on_permanent_failure(
                &message,
            )
        });
        assert_eq!(
            mode, "valid",
            "invalid configuration must terminate the process"
        );
        assert_eq!(
            minecraft_fishingbot_manager_for_discord::config::Config::get()
                .text()
                .report_succeeded_label(),
            "[完成明细]"
        );
        return;
    }

    let root = common::temp_dir("english-config-error");
    let data = root.join("data");
    std::fs::create_dir_all(&data).expect("create base directory");
    let mut config: toml::Value = toml::from_str(common::example_config()).expect("parse example");
    config["paths"]["base_dir"] = toml::Value::String(data.to_string_lossy().into_owned());
    config["i18n"]["language"] = toml::Value::String("CHINESE_SIMPLIFIED".into());
    config["anti_afk"]["input_delay"] = toml::Value::Float(-1.0);
    std::fs::write(data.join(config["paths"]["jar"].as_str().unwrap()), b"jar")
        .expect("create test jar");
    std::fs::write(
        root.join(common::TEST_CONFIG_FILE_NAME),
        toml::to_string(&config).expect("serialize config"),
    )
    .expect("write config");
    let executable = root.join("bootstrap-text-test");
    std::fs::copy(
        std::env::current_exe().expect("test executable"),
        &executable,
    )
    .expect("copy test executable");

    let returned_error = std::process::Command::new(&executable)
        .args([
            "--exact",
            "chinese_config_errors_are_english_and_success_uses_configured_language",
            "--nocapture",
        ])
        .env(CHILD, "returned_error")
        .output()
        .expect("run isolated config error test");
    assert!(
        returned_error.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&returned_error.stdout),
        String::from_utf8_lossy(&returned_error.stderr)
    );

    let output = std::process::Command::new(&executable)
        .args([
            "--exact",
            "chinese_config_errors_are_english_and_success_uses_configured_language",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .expect("run isolated config validation");
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("config item anti_afk.input_delay"),
        "{error}"
    );
    assert!(error.contains("non-negative"), "{error}");
    assert!(!error.contains("配置项"), "{error}");
    assert!(!error.contains("config not loaded yet"), "{error}");
    assert!(!error.contains("panicked"), "{error}");

    config["anti_afk"]["input_delay"] = toml::Value::Float(0.05);
    std::fs::write(
        root.join(common::TEST_CONFIG_FILE_NAME),
        toml::to_string(&config).expect("serialize valid config"),
    )
    .expect("write valid config");
    let output = std::process::Command::new(&executable)
        .args([
            "--exact",
            "chinese_config_errors_are_english_and_success_uses_configured_language",
            "--nocapture",
        ])
        .env(CHILD, "valid")
        .output()
        .expect("run isolated valid config");
    common::remove_dir(&root);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 内部不变量消息指出正确的配置初始化入口。
#[test]
fn bootstrap_invariants_name_the_entry_point() {
    assert!(
        bootstrap::CONFIG_NOT_LOADED.contains("config::Config::load()"),
        "{}",
        bootstrap::CONFIG_NOT_LOADED
    );
    assert!(
        bootstrap::CONFIG_NOT_LOADED.contains("process entry point"),
        "{}",
        bootstrap::CONFIG_NOT_LOADED
    );
}

/// 回复日志同时包含调用者、指令路径与回复正文。
#[test]
fn replied_log_line_contains_caller_command_and_reply() {
    let line = Language::English
        .implementation()
        .command_replied(42, "bot add", "created");

    assert!(line.contains("/bot add"), "{line}");
    assert!(line.contains("42"), "{line}");
    assert!(line.contains("created"), "{line}");
}
