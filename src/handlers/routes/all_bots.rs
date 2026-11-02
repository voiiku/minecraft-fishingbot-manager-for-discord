//! 批量指令与反 AFK 路由。

use super::permission_refusal;
use crate::handlers::{
    planned_startall_notice, restart_all_bots, send_command_to_all_bots,
    set_all_bots_fishing_pause, start_all_bots, start_anti_afk, stop_all_bots, stop_anti_afk,
};
use crate::text;
use crate::utils::{IntoReplyText, PauseCommand, RequiredPermission, has_permission};

/// 面向全体机器人的批量动作，仅管理员可用。
pub enum AllBotsControlAction<'a> {
    StartAll,
    StopAll,
    RestartAll,
    /// 尝试暂停目标机器人的钓鱼并检查回显。
    PauseAll,
    /// 尝试恢复目标机器人的钓鱼并检查回显。
    UnpauseAll,
    /// 向全部运行中的机器人发送游戏内指令。
    CommandAll(&'a [&'a str]),
}

impl AllBotsControlAction<'_> {
    fn required_permission(&self) -> RequiredPermission {
        RequiredPermission::MustBeAdmin
    }
}

/// 反 AFK 开关动作，仅管理员可用。
pub enum AntiAfkAction {
    On,
    Off,
}

/// 返回 `/startall` 进度回执；实际启动时检查管理员权限。
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
        AllBotsControlAction::PauseAll => {
            set_all_bots_fishing_pause(PauseCommand::SetPause).to_reply_text()
        }
        AllBotsControlAction::UnpauseAll => {
            set_all_bots_fishing_pause(PauseCommand::SetUnpause).to_reply_text()
        }
        AllBotsControlAction::CommandAll(raw_bot_command_args) => {
            send_command_to_all_bots(raw_bot_command_args).to_reply_text()
        }
    }
}

/// 处理反 AFK 开关，要求管理员权限。
pub async fn handle_anti_afk(action: AntiAfkAction, caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::MustBeAdmin) {
        return permission_refusal(caller_id);
    }

    let i18n_text = text::get();
    match action {
        AntiAfkAction::On => start_anti_afk()
            .await
            .map(|started| {
                if started {
                    i18n_text.anti_afk_started()
                } else {
                    i18n_text.anti_afk_already_running()
                }
                .to_string()
            })
            .to_reply_text(),
        AntiAfkAction::Off => stop_anti_afk()
            .await
            .map(|()| i18n_text.anti_afk_stopped().to_string())
            .to_reply_text(),
    }
}
