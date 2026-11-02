//! 库入口：导出指令路由、运行时组件与公共工具。

mod handlers;
mod models;
mod runtime;
pub mod text;
pub mod utils;

#[cfg(test)]
mod test_support;

/// 兼容旧名称；仅检查管理员权限，不执行指令同步。
pub use handlers::check_sync_commands_permission as handle_sync_commands;
pub use handlers::{
    AllBotsControlAction, AntiAfkAction, BotControlAction, BotLogView, BotLoginPrompt,
    BotManageAction, FishingbotConfigView, ShowAction, check_sync_commands_permission,
    handle_all_bots_control, handle_anti_afk, handle_bot_control, handle_bot_log, handle_bot_login,
    handle_bot_login_await, handle_bot_manage, handle_checkserver, handle_fishingbot_config,
    handle_help, handle_ping, handle_show, handle_startall_notice, handle_whoami,
};
pub use runtime::config;
pub use runtime::{WATCHDOG, Watchdog, WatchdogSlot};
pub use utils::{
    BotDirEntry, ProcessLog, RequiredPermission, StartupQueue, all_bot_dir_entries,
    bot_dir_entries_of_owner, has_permission, regenerate_fishingbot_config,
    split_bot_entries_by_is_running,
};
