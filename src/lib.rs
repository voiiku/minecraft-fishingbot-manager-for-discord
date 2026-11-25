pub mod discord_commands;
pub mod operations;
pub mod runtime;
pub mod text;
pub mod utils;

#[cfg(test)]
#[path = "../tests/common/unit_support.rs"]
mod test_support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub use operations::{
    accounts::BotLoginPrompt,
    queries::{BotLogView, FishingbotConfigView},
};
pub use runtime::{
    ProcessLog, Watchdog,
    bot_instances::{
        bot_dirs::{BotDirEntry, all_bot_dir_entries, bot_dir_entries_of_owner},
        bot_sessions::split_bot_entries_by_is_running,
        bot_status::BotStatus,
        fishingbot_config::regenerate_fishingbot_config,
    },
    bot_startup::StartupQueue,
    config,
};

// 进程级共享状态；初始化与访问规则由所属模块负责。
pub(crate) static CONFIG: OnceLock<config::Config> = OnceLock::new();
pub(crate) static STARTUP_COMPLETE: AtomicBool = AtomicBool::new(false);
pub(crate) static PROCESS_LOGGER: OnceLock<ProcessLog> = OnceLock::new();
pub(crate) static STARTUP_QUEUE: OnceLock<StartupQueue> = OnceLock::new();
pub(crate) static WATCHDOG: OnceLock<Watchdog> = OnceLock::new();
pub(crate) static ANTI_AFK: Mutex<Option<input_pulse::InputPulse>> = Mutex::new(None);
pub(crate) static SCREEN_COMMAND_LOCKS: OnceLock<Box<[Mutex<()>]>> = OnceLock::new();
pub(crate) static COMMAND_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 使用启动期英文或首次框架初始化后配置的语言。
pub fn bootstrap_error_text() -> &'static dyn text::Text {
    if STARTUP_COMPLETE.load(Ordering::Acquire) {
        config::Config::get().text()
    } else {
        text::Language::English.implementation()
    }
}

/// 首次框架初始化成功后启用配置语言的错误文案。
pub fn complete_startup() {
    STARTUP_COMPLETE.store(true, Ordering::Release);
}
