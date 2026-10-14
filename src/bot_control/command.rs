//! `/command`：向单个运行中的机器人发送游戏内指令。

use crate::text;
use crate::utils::{
    BotDirEntry, ToCleanBotCommand, is_bot_screen_running, send_command_to_single_bot,
};

/// 向运行中的机器人发送游戏内指令（`/command`）。
///
/// 向运行中的机器人发送游戏内指令；权限由路由层检查。
/// 参数由 [`ToCleanBotCommand`] 拼接，发送由 [`send_command_to_single_bot`] 完成。
pub(crate) fn send_command_to_bot(
    bot_dir_entry: &BotDirEntry,
    raw_bot_command_args: &[&str],
) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let bot_command = raw_bot_command_args.to_clean_bot_command()?;

    // 会话不存在时无法发送指令。
    if !is_bot_screen_running(bot_name) {
        return Err(text::get().bot_command_not_running(bot_name));
    }

    send_command_to_single_bot(bot_name, &bot_command)?;

    Ok(text::get().bot_command_sent(bot_name, &bot_command))
}
