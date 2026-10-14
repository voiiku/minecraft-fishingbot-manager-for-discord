//! `/unpause`：取消暂停、恢复钓鱼（与 `/pause` 配对）。

use crate::bot_control::command::send_command_to_bot;
use crate::text;
use crate::utils::{BotDirEntry, PauseCommand, confirm_pause_command};

/// 取消暂停、恢复钓鱼（`/unpause`）。
///
/// 恢复钓鱼并检查 JAR 回显。权限由路由层检查，回显由 `[command_echo] unpause_echo` 配置。
pub(crate) fn unpause_fishing(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let pause_command = PauseCommand::SetUnpause;
    let confirmed = confirm_pause_command(
        bot_dir_entry,
        // 复用 `/command` 的发送与校验逻辑。
        || send_command_to_bot(bot_dir_entry, &[pause_command.command()]).map(|_reply| ()),
        pause_command,
    )?;

    if confirmed {
        return Ok(text::get().bot_unpause_confirmed(bot_name));
    }

    Ok(text::get().bot_command_sent(bot_name, pause_command.command()))
}
