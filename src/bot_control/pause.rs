//! `/pause`：暂停钓鱼但保持在线。

use crate::bot_control::command::send_command_to_bot;
use crate::text;
use crate::utils::{BotDirEntry, PauseCommand, confirm_pause_command};

/// 暂停钓鱼但保持在线（`/pause`）。
///
/// 暂停钓鱼并检查 JAR 回显。权限由路由层检查，回显由 `[command_echo] pause_echo` 配置。
pub(crate) fn pause_fishing(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let pause_command = PauseCommand::SetPause;
    let confirmed = confirm_pause_command(
        bot_dir_entry,
        // 复用 `/command` 的发送与校验逻辑。
        || send_command_to_bot(bot_dir_entry, &[pause_command.command()]).map(|_reply| ()),
        pause_command,
    )?;

    if confirmed {
        return Ok(text::get().bot_pause_confirmed(bot_name));
    }

    Ok(text::get().bot_command_sent(bot_name, pause_command.command()))
}
