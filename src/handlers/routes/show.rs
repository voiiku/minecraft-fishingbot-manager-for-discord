//! 只读查询指令路由。

use super::permission_refusal;
use crate::handlers::{
    FishingbotConfigView, admins_list, all_bots_list, bot_and_anti_afk_status_report,
    fishingbot_config_view, my_bots_list,
};
use crate::utils::{IntoReplyText, RequiredPermission, has_permission};

/// `/show` 的文本查询动作；附件查询使用 [`handle_fishingbot_config`]。
pub enum ShowAction {
    Status,
    MyBots,
    AllBots,
    Admins,
}

impl ShowAction {
    fn required_permission(&self) -> RequiredPermission {
        RequiredPermission::Everyone
    }
}

/// 处理只读查询指令。
pub fn handle_show(action: ShowAction, caller_id: u64) -> String {
    if !has_permission(caller_id, action.required_permission()) {
        return permission_refusal(caller_id);
    }

    match action {
        ShowAction::Status => bot_and_anti_afk_status_report().to_reply_text(),
        ShowAction::MyBots => my_bots_list(caller_id).to_reply_text(),
        ShowAction::AllBots => all_bots_list().to_reply_text(),
        ShowAction::Admins => admins_list(),
    }
}

/// 返回共享的游戏侧配置文件。
pub fn handle_fishingbot_config(caller_id: u64) -> Result<FishingbotConfigView, String> {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return Err(permission_refusal(caller_id));
    }

    fishingbot_config_view()
}
