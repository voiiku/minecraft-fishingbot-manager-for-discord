//! 机器人账号管理指令路由。

use super::permitted_bot_dir_entry;
use crate::handlers::{
    BotLoginPrompt, add_bot, await_login, begin_login, delete_bot, rename_bot,
    update_bot_token_file,
};
use crate::utils::IntoReplyText;

/// 机器人账号管理动作。
pub enum BotManageAction {
    Add,
    Delete,
    Rename(String),
    Token(String),
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

/// 启动登录并返回设备码。
pub fn handle_bot_login(raw_bot_name: &str, caller_id: u64) -> Result<BotLoginPrompt, String> {
    permitted_bot_dir_entry(raw_bot_name, caller_id)
        .and_then(|bot_dir_entry| begin_login(&bot_dir_entry))
}

/// 等待登录凭据写入或登录失败。
pub fn handle_bot_login_await(prompt: BotLoginPrompt) -> String {
    await_login(prompt)
}
