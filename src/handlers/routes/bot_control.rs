//! 单机器人控制指令路由。

use super::permitted_bot_dir_entry;
use crate::handlers::{
    BotLogView, bot_log_view, pause_fishing, restart_bot, send_command_to_bot, start_bot, stop_bot,
    unpause_fishing,
};
use crate::utils::IntoReplyText;

/// 单个机器人的控制动作。
pub enum BotControlAction<'a> {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    /// 发送游戏内指令，原始片段由业务逻辑校验。
    Command(&'a [&'a str]),
}

/// 读取机器人日志，要求本人或管理员权限。
pub fn handle_bot_log(
    caller_id: u64,
    raw_bot_name: &str,
    line_count: usize,
) -> Result<BotLogView, String> {
    let bot_dir_entry = permitted_bot_dir_entry(raw_bot_name, caller_id)?;

    bot_log_view(&bot_dir_entry, line_count)
}

/// 处理单机器人控制指令，要求本人或管理员权限。
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
