//! Discord 指令业务处理入口。

mod all_bots_control;
mod bot_control;
mod bot_manage;
mod general;
mod routes;
mod show;

pub(crate) use all_bots_control::{
    is_anti_afk_running, planned_startall_notice, restart_all_bots, send_command_to_all_bots,
    set_all_bots_fishing_pause, start_all_bots, start_anti_afk, stop_all_bots, stop_anti_afk,
};
pub use bot_control::BotLogView;
pub(crate) use bot_control::{
    bot_log_view, pause_fishing, restart_bot, send_command_to_bot, start_bot, stop_bot,
    unpause_fishing,
};
pub use bot_manage::BotLoginPrompt;
pub(crate) use bot_manage::{
    add_bot, await_login, begin_login, delete_bot, rename_bot, update_bot_token_file,
};
pub(crate) use general::{checkserver, help, ping, whoami};
pub use routes::{
    AllBotsControlAction, AntiAfkAction, BotControlAction, BotManageAction, ShowAction,
    check_sync_commands_permission, handle_all_bots_control, handle_anti_afk, handle_bot_control,
    handle_bot_log, handle_bot_login, handle_bot_login_await, handle_bot_manage,
    handle_checkserver, handle_fishingbot_config, handle_help, handle_ping, handle_show,
    handle_startall_notice, handle_whoami,
};
pub use show::FishingbotConfigView;
pub(crate) use show::{
    admins_list, all_bots_list, bot_and_anti_afk_status_report, fishingbot_config_view,
    my_bots_list,
};
