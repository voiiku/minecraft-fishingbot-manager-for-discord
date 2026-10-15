//! 指令入口层：统一检查权限、分发动作并将结果转换为回复文本。
//! 业务模块的子模块均为私有，只从父模块导入入口。

mod all_bots_control;
mod bot_control;
mod bot_manage;
pub mod config;
mod general;
mod process_log;
mod show;
pub mod text;
mod utils;
mod watchdog;

/// 测试辅助函数。
#[cfg(test)]
mod test_support;

// 业务函数统一从父模块导入；子模块保持私有。
use crate::all_bots_control::{
    planned_startall_notice, restart_all_bots, send_command_to_all_bots, start_all_bots,
    stop_all_bots,
};
use crate::bot_control::{
    bot_log_view, pause_fishing, restart_bot, send_command_to_bot, start_bot, stop_bot,
    unpause_fishing,
};
use crate::bot_manage::{
    add_bot, await_login, begin_login, delete_bot, rename_bot, update_bot_token_file,
};
use crate::general::{checkserver_report, help, ping_report, whoami};
use crate::show::{
    admins_list, all_bots_list, all_running_bots_status, fishingbot_config_view, my_bots_list,
};
use crate::utils::IntoReplyText;

pub use utils::{
    BotDirEntry, RequiredPermission, all_bot_dir_entries, bot_dir_entries_of_owner, has_permission,
    split_bot_entries_by_is_running,
};

pub use watchdog::{WATCHDOG, Watchdog, WatchdogSlot};

pub use bot_control::BotLogView;
pub use bot_manage::BotLoginPrompt;
pub use process_log::ProcessLog;
pub use show::FishingbotConfigView;
pub use utils::regenerate_fishingbot_config;

/// `/show` 的文本查询动作；附件查询使用 [`handle_fishingbot_config`]。
pub enum ShowAction {
    Status,
    MyBots,
    AllBots,
    Admins,
}

impl ShowAction {
    /// 只读查询对所有人开放。
    fn required_permission(&self) -> RequiredPermission {
        RequiredPermission::Everyone
    }
}

/// 机器人账号管理动作。
pub enum BotManageAction {
    Add,
    Delete,
    Rename(String),
    Token(String),
}

/// 单个机器人的控制动作。
pub enum BotControlAction<'a> {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    /// 向机器人发送游戏内指令：命令层传递的原始片段（拼指令与校验在业务函数中执行）。
    Command(&'a [&'a str]),
}

/// 面向全体机器人的批量动作（仅管理员）。
pub enum AllBotsControlAction<'a> {
    StartAll,
    StopAll,
    RestartAll,
    /// 向全部运行中的机器人发送同一条指令（原始片段，口径同 [`BotControlAction::Command`]）。
    CommandAll(&'a [&'a str]),
}

impl AllBotsControlAction<'_> {
    /// 这批指令的权限要求：碰的是全服机器人，只有管理员能做。
    fn required_permission(&self) -> RequiredPermission {
        RequiredPermission::MustBeAdmin
    }
}

/// 命令需求管理员权限，而权限不足时的回复。
fn permission_refusal(caller_id: u64) -> String {
    text::get().admin_required(caller_id)
}

/// 需要使用机器人条目，且要求本人或管理员的命令调用本函数，找不到与无权访问使用相同回复。
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

/// 返回 `/help` 帮助文本。权限：所有人可用。
pub fn handle_help(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    help()
}

/// 返回 `/whoami` 文本（调用者自己的 Discord 用户 ID）。权限：所有人可用。
pub fn handle_whoami(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    whoami(caller_id)
}

/// 探测游戏服务器（`/ping`）。权限：所有人可用。
pub async fn handle_ping(caller_id: u64, host: Option<&str>, port: Option<u16>) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    ping_report(host, port).await.to_reply_text()
}

/// 返回钓鱼服务器自身的资源信息（`/checkserver`）。权限：所有人可用。
pub async fn handle_checkserver(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    checkserver_report().await.to_reply_text()
}

/// 处理只读查询指令（`/show status|mybots|allbots|admins`）。
///
/// 权限要求由动作自己声明（现在是所有人可用）。
pub fn handle_show(action: ShowAction, caller_id: u64) -> String {
    if !has_permission(caller_id, action.required_permission()) {
        return permission_refusal(caller_id);
    }

    match action {
        ShowAction::Status => all_running_bots_status().to_reply_text(),
        ShowAction::MyBots => my_bots_list(caller_id).to_reply_text(),
        ShowAction::AllBots => all_bots_list().to_reply_text(),
        ShowAction::Admins => admins_list(),
    }
}
/// 返回共享的游戏侧配置文件，权限对所有人开放。
pub fn handle_fishingbot_config(caller_id: u64) -> Result<FishingbotConfigView, String> {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return Err(permission_refusal(caller_id));
    }

    fishingbot_config_view()
}

/// 处理机器人账号管理指令并检查目标权限。
pub fn handle_bot_manage(action: BotManageAction, raw_bot_name: &str, caller_id: u64) -> String {
    match action {
        BotManageAction::Add => add_bot(raw_bot_name, caller_id).to_reply_text(),
        BotManageAction::Delete => permitted_bot_dir_entry(raw_bot_name, caller_id)
            .and_then(|bot_dir_entry| delete_bot(&bot_dir_entry))
            .to_reply_text(),
        BotManageAction::Rename(new_name) => permitted_bot_dir_entry(raw_bot_name, caller_id)
            .and_then(|bot_dir_entry| rename_bot(&bot_dir_entry, &new_name))
            .to_reply_text(),
        BotManageAction::Token(raw_token) => permitted_bot_dir_entry(raw_bot_name, caller_id)
            .and_then(|bot_dir_entry| update_bot_token_file(&bot_dir_entry, raw_token))
            .to_reply_text(),
    }
}

/// `/bot login` 第一段：启动登录并返回设备码。权限要求与 [`handle_bot_manage`] 的 Token 操作相同。
pub fn handle_bot_login(raw_bot_name: &str, caller_id: u64) -> Result<BotLoginPrompt, String> {
    permitted_bot_dir_entry(raw_bot_name, caller_id)
        .and_then(|bot_dir_entry| begin_login(&bot_dir_entry))
}

/// `/bot login` 第二段：等待凭据落盘或登录失败。
pub fn handle_bot_login_await(prompt: BotLoginPrompt) -> String {
    await_login(prompt)
}

/// 读取机器人日志；权限要求本人或管理员。
pub fn handle_bot_log(
    caller_id: u64,
    raw_bot_name: &str,
    line_count: usize,
) -> Result<BotLogView, String> {
    let bot_dir_entry = permitted_bot_dir_entry(raw_bot_name, caller_id)?;

    bot_log_view(&bot_dir_entry, line_count)
}

/// 处理单机器人控制指令，要求本人或管理员，已在[`permitted_bot_dir_entry`]校验过
pub fn handle_bot_control(
    action: BotControlAction<'_>,
    raw_bot_name: &str,
    caller_id: u64,
) -> String {
    let bot_dir_entry = match permitted_bot_dir_entry(raw_bot_name, caller_id) {
        Ok(bot_dir_entry) => bot_dir_entry,
        Err(message) => return message,
    };

    match action {
        BotControlAction::Start => start_bot(&bot_dir_entry).to_reply_text(),
        BotControlAction::Stop => stop_bot(&bot_dir_entry).to_reply_text(),
        BotControlAction::Restart => restart_bot(&bot_dir_entry).to_reply_text(),
        BotControlAction::Pause => pause_fishing(&bot_dir_entry).to_reply_text(),
        BotControlAction::Unpause => unpause_fishing(&bot_dir_entry).to_reply_text(),
        BotControlAction::Command(raw_bot_command_args) => {
            send_command_to_bot(&bot_dir_entry, raw_bot_command_args).to_reply_text()
        }
    }
}

/// 处理管理员批量指令。
/// `/startall` 的第一条回执：这次要启动几台、预计多久。权限：所有人可用（真正的启动在
/// [`handle_all_bots_control`] 里检查管理员权限）。
pub fn handle_startall_notice() -> String {
    planned_startall_notice()
}

/// 处理管理员批量指令。
pub fn handle_all_bots_control(action: AllBotsControlAction<'_>, caller_id: u64) -> String {
    if !has_permission(caller_id, action.required_permission()) {
        return permission_refusal(caller_id);
    }

    match action {
        AllBotsControlAction::StartAll => start_all_bots().to_reply_text(),
        AllBotsControlAction::StopAll => stop_all_bots().to_reply_text(),
        AllBotsControlAction::RestartAll => restart_all_bots().to_reply_text(),
        AllBotsControlAction::CommandAll(raw_bot_command_args) => {
            send_command_to_all_bots(raw_bot_command_args).to_reply_text()
        }
    }
}
