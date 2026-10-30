//! 公开文本与启动提示的集成测试。

mod common;

use minecraft_fishingbot_manager_for_discord::text::{
    self, CommandTextProblem, ConfigRequirement, Language, LogText, bootstrap,
    failure_is_permanent, setup_failure_log_text,
};

#[test]
fn guild_cleanup_notice_describes_the_scheduled_recheck() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].log_text(LogText::GuildCommandsCleanupWaiting { minutes: 52 }),
        "全局指令尚未满足传播稳定性检查条件；将在约 52 分钟后重新检查，并在条件满足时清理各服务器的遗留 guild 指令"
    );
    assert_eq!(
        languages[1].log_text(LogText::GuildCommandsCleanupWaiting { minutes: 52 }),
        "Global commands have not yet met the propagation stability criteria. A recheck is scheduled in approximately 52 minutes; legacy guild commands will be removed once the criteria are met"
    );
    for language in languages {
        let error = language.log_text(LogText::GlobalCommandsReadFailed { reason: "detail" });
        assert!(error.contains("detail"), "{error}");
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
            CommandTextProblem::InvalidLength { length: 102 },
        );
        assert!(error.contains("start name"), "{error}");
        assert!(error.contains("102"), "{error}");
    }
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
        let lookup = language.log_text(LogText::AutocompleteLookupFailed { reason: "detail" });
        let task = language.log_text(LogText::AutocompleteTaskFailed { reason: "detail" });
        assert_ne!(lookup, task);
        assert!(lookup.contains("detail"));
        assert!(task.contains("detail"));
        let fallback = language.log_text(LogText::CommandFallbackReplyFailed {
            caller_id: 123,
            command_path: "start",
            reason: "detail",
        });
        assert!(fallback.contains("123"), "{fallback}");
        assert!(fallback.contains("/start"), "{fallback}");
        assert!(fallback.contains("detail"), "{fallback}");
    }
    assert_eq!(
        languages[1].log_text(LogText::AutocompleteLookupFailed { reason: "detail" }),
        "Autocomplete candidate lookup failed: detail"
    );
    assert_eq!(
        languages[1].log_text(LogText::AutocompleteTaskFailed { reason: "detail" }),
        "Autocomplete candidate lookup task failed: detail"
    );
    assert_eq!(
        languages[1].log_text(LogText::CommandFallbackReplyFailed {
            caller_id: 123,
            command_path: "start",
            reason: "detail",
        }),
        "Failed to send the fallback response for /start to Discord user 123: detail"
    );
}

#[test]
fn queue_full_reason_is_localized() {
    let languages = text::all_languages();
    assert_eq!(
        languages[0].start_queue_full_reason(),
        "启动队列已满（max_pending_requests）"
    );
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
            language.anti_afk_status_failed("test reason"),
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

#[test]
fn anti_afk_validation_errors_are_localized() {
    let languages = text::all_languages();
    for (language, expected) in [(languages[0], "非负"), (languages[1], "non-negative")] {
        let message = language.log_text(LogText::ConfigFieldInvalid {
            field: "anti_afk.input_delay",
            requirement: ConfigRequirement::NonNegativeDuration,
        });
        assert!(message.contains("anti_afk.input_delay"), "{message}");
        assert!(message.contains(expected), "{message}");
    }

    for language in languages {
        let message = language.log_text(LogText::ConfigFieldInvalid {
            field: "anti_afk.command",
            requirement: ConfigRequirement::InputPulseCommand {
                reason: "invalid token",
            },
        });
        assert!(message.contains("anti_afk.command"), "{message}");
        assert!(message.contains("invalid token"), "{message}");
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

/// 初始化错误按成因分类：缺少邀请、Token 无效与普通连接失败。
#[test]
fn setup_failures_are_classified_by_cause() {
    for message in ["Missing Access", "Unknown Guild"] {
        assert!(
            matches!(
                setup_failure_log_text(message),
                LogText::DiscordInitFailedNotInvited { .. }
            ),
            "{message} must be classified as a missing invitation"
        );
    }

    for message in ["Unauthorized", "Invalid Token", "HTTP 401"] {
        assert!(
            matches!(
                setup_failure_log_text(message),
                LogText::DiscordInitFailedBadToken { .. }
            ),
            "{message} must be classified as a bad token"
        );
    }

    assert!(matches!(
        setup_failure_log_text("connection refused"),
        LogText::DiscordInitFailed { .. }
    ));
}

/// 缺少邀请与 Token 无效属于永久错误；普通初始化失败允许重试。
#[test]
fn only_configuration_failures_are_permanent() {
    assert!(failure_is_permanent(
        &LogText::DiscordInitFailedNotInvited {
            error: "Missing Access"
        }
    ));
    assert!(failure_is_permanent(&LogText::DiscordInitFailedBadToken {
        error: "HTTP 401"
    }));
    assert!(!failure_is_permanent(&LogText::DiscordInitFailed {
        error: "connection refused"
    }));

    assert!(failure_is_permanent(&setup_failure_log_text(
        "Unknown Guild"
    )));
    assert!(!failure_is_permanent(&setup_failure_log_text(
        "connection refused"
    )));
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

/// 内部不变量消息指出正确的配置初始化入口。
#[test]
fn bootstrap_invariants_name_the_entry_point() {
    assert!(
        bootstrap::CONFIG_NOT_LOADED.contains("config::Config::get_or_init()"),
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
    common::install_english();

    let line = text::get().log_text(LogText::CommandReplied {
        caller_id: 42,
        command_path: "bot add",
        reply: "created",
    });

    assert!(line.contains("/bot add"), "{line}");
    assert!(line.contains("42"), "{line}");
    assert!(line.contains("created"), "{line}");
}
