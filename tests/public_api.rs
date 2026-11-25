//! 公开 API 的集成测试。

mod common;

use std::time::{Duration, Instant};

use minecraft_fishingbot_manager_for_discord::operations;
use minecraft_fishingbot_manager_for_discord::runtime::bot_instances::bot_sessions::token_ready_bot_names_with_screen_sessions;
use minecraft_fishingbot_manager_for_discord::text::Language;
use minecraft_fishingbot_manager_for_discord::text::report::Report;
use minecraft_fishingbot_manager_for_discord::{
    BotLogView, BotLoginPrompt, FishingbotConfigView, ProcessLog, StartupQueue, Watchdog,
};

#[test]
fn bot_command_types_and_entry_points_remain_public() {
    let _: fn(&str, u64) -> Result<BotLoginPrompt, String> = operations::accounts::begin_bot_login;
    let _: fn(BotLoginPrompt) -> String = operations::accounts::finish_bot_login;
    let _: fn(&str, Option<i64>, u64) -> Result<BotLogView, String> =
        operations::queries::read_bot_log;
    let _: fn() -> Result<FishingbotConfigView, String> = operations::queries::show_bot_config;
    let _: fn(&str, u64) -> Result<String, String> =
        operations::single_bot_control::start_single_bot;
    let _: fn(u64) -> Result<String, String> = operations::all_bots_control::start_all_bots;
    let _: fn() -> Result<Vec<String>, String> = token_ready_bot_names_with_screen_sessions;
    let _: fn() -> &'static ProcessLog = ProcessLog::get_or_init;
    let _: fn() -> &'static StartupQueue = StartupQueue::get_or_init;
    let _: fn() -> &'static Watchdog = Watchdog::get_or_init;
    let _: fn() -> Result<(), String> =
        minecraft_fishingbot_manager_for_discord::config::Config::load;
    let _: fn() -> Result<String, String> =
        minecraft_fishingbot_manager_for_discord::runtime::bot_instances::stop::shutdown_all_bots;
    let _: fn(&str) -> ! =
        minecraft_fishingbot_manager_for_discord::runtime::shutdown::exit_on_permanent_failure;
}

#[test]
fn report_methods_are_available_from_the_owning_module() {
    let mut report = Report::new(Language::English, 2_000);
    assert!(report.title.is_none());
    assert!(report.basics.is_empty());
    assert!(report.succeeded.is_empty());
    assert!(report.skipped.is_empty());
    assert!(report.queued.is_empty());
    assert!(report.failed.is_empty());
    assert!(report.content.is_none());
    report.set_title("test");
    assert_eq!(report.title.as_deref(), Some("test"));
}

/// `/whoami` 对任何调用者返回其自己的用户 ID。
#[test]
fn whoami_reply_contains_the_caller_id() {
    if !common::in_configured_child("whoami_reply_contains_the_caller_id") {
        return;
    }
    let caller_id = 123_456_789_012_345_678u64;

    let reply = operations::queries::whoami(caller_id);

    assert!(reply.contains(&caller_id.to_string()), "{reply}");
}

/// 看门狗的公开方法允许外部推进计时状态；内部判定类型不对外暴露。
#[test]
fn watchdog_bookkeeping_can_be_driven_from_outside() {
    let watchdog = Watchdog::new(
        Instant::now(),
        Duration::from_secs(600),
        Duration::from_secs(200),
        Duration::from_secs(1200),
    );

    watchdog.begin_connection_attempt(Instant::now());
    assert!(watchdog.record_handshake_success(Instant::now()));
    assert!(!watchdog.record_handshake_success(Instant::now()));
    watchdog.clear_probe_failures();
    watchdog.record_probe_failure_start(Instant::now());
}
