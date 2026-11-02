//! Discord 指令路由与权限检查。

mod all_bots;
mod bot_control;
mod bot_manage;
mod general;
mod show;

use crate::text;
use crate::utils::{BotDirEntry, RequiredPermission, has_permission};

pub use all_bots::{
    AllBotsControlAction, AntiAfkAction, handle_all_bots_control, handle_anti_afk,
    handle_startall_notice,
};
pub use bot_control::{BotControlAction, handle_bot_control, handle_bot_log};
pub use bot_manage::{
    BotManageAction, handle_bot_login, handle_bot_login_await, handle_bot_manage,
};
pub use general::{
    check_sync_commands_permission, handle_checkserver, handle_help, handle_ping, handle_whoami,
};
pub use show::{ShowAction, handle_fishingbot_config, handle_show};

fn permission_refusal(caller_id: u64) -> String {
    text::get().admin_required(caller_id)
}

/// 解析机器人并检查归属；不存在与无权访问使用相同回复。
fn permitted_bot_dir_entry(raw_bot_name: &str, caller_id: u64) -> Result<BotDirEntry, String> {
    let bot_dir_entry = BotDirEntry::find_by_name(raw_bot_name)?;

    if !has_permission(
        caller_id,
        RequiredPermission::OwnerOrAdmin(bot_dir_entry.owner_id),
    ) {
        return Err(text::get().bot_dir_not_found(&bot_dir_entry.bot_name));
    }

    Ok(bot_dir_entry)
}
