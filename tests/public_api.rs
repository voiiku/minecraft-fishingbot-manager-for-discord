//! 公开 API 的集成测试。

mod common;

use std::time::{Duration, Instant};

use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, AntiAfkAction, BotControlAction, BotLogView, BotLoginPrompt,
    BotManageAction, FishingbotConfigView, ProcessLog, ShowAction, StartupQueue, WATCHDOG,
    Watchdog, WatchdogSlot, check_sync_commands_permission, config, handle_all_bots_control,
    handle_anti_afk, handle_bot_control, handle_bot_log, handle_bot_login, handle_bot_login_await,
    handle_bot_manage, handle_checkserver, handle_fishingbot_config, handle_help, handle_ping,
    handle_show, handle_startall_notice, handle_sync_commands, handle_whoami, utils,
};

#[test]
fn bot_command_types_and_entry_points_remain_public() {
    let _: fn(&str, u64) -> Result<BotLoginPrompt, String> = handle_bot_login;
    let _: fn(BotLoginPrompt) -> String = handle_bot_login_await;
    let _: fn(u64, &str, usize) -> Result<BotLogView, String> = handle_bot_log;
    let _: fn(u64) -> Result<FishingbotConfigView, String> = handle_fishingbot_config;
    let _: fn(BotManageAction, &str, u64) -> String = handle_bot_manage;
    let _: fn(BotControlAction<'_>, &str, u64) -> String = handle_bot_control;
    let _: fn(AllBotsControlAction<'_>, u64) -> String = handle_all_bots_control;
    let _: fn(ShowAction, u64) -> String = handle_show;
    let _: fn() -> String = handle_startall_notice;
    let _: fn(u64) -> String = handle_help;
    let _: fn(u64) -> Result<(), String> = check_sync_commands_permission;
    let _: fn(u64) -> Result<(), String> = handle_sync_commands;
    let _: fn() -> Result<Vec<String>, String> = utils::token_ready_bot_names_with_screen_sessions;
    let _: fn() -> Result<Vec<String>, String> = utils::all_running_bot_names;
    let _ = (handle_ping, handle_checkserver, handle_anti_afk);
    let _ = [AntiAfkAction::On, AntiAfkAction::Off];
    let _: fn() -> &'static ProcessLog = ProcessLog::get_or_init;
    let _: fn() -> &'static ProcessLog = utils::ProcessLog::get_or_init;
    let _: fn() -> &'static utils::StartupQueue = StartupQueue::get_or_init;
    let _: fn() -> &'static StartupQueue = utils::StartupQueue::get_or_init;
    let _: &'static WatchdogSlot = &WATCHDOG;
    assert_eq!(
        config::CONFIG_FILE_NAME,
        "minecraft-fishingbot-manager-for-discord.conf.toml"
    );
}

#[test]
fn shared_model_methods_remain_available_via_utils() {
    let mut report = utils::Report::default();
    assert!(report.title.is_none());
    assert!(report.basics.is_empty());
    assert!(report.succeeded.is_empty());
    assert!(report.skipped.is_empty());
    assert!(report.queued.is_empty());
    assert!(report.failed.is_empty());
    assert!(report.content.is_none());
    report.set_title("test");
    assert_eq!(report.title.as_deref(), Some("test"));

    assert_eq!(utils::PauseCommand::SetPause.command(), "/pause");
    assert_eq!(utils::PauseCommand::SetUnpause.command(), "/unpause");
}

/// `/whoami` 对任何调用者返回其自己的用户 ID。
#[test]
fn whoami_reply_contains_the_caller_id() {
    common::install_english();
    let caller_id = 123_456_789_012_345_678u64;

    let reply = handle_whoami(caller_id);

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
    watchdog.record_handshake_success(Instant::now());
    assert!(watchdog.is_first_handshake());
    assert!(!watchdog.is_first_handshake());
    watchdog.clear_probe_failures();
    watchdog.record_probe_failure_start(Instant::now());
}
