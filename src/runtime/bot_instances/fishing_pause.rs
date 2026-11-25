//! 确认 FishingBot 的实时暂停状态。
use super::{bot_dirs::BotDirEntry, bot_status::query_bot_status};

/// 发送指令，再用 FishingBot 的 `/status` 确认目标状态。
pub(crate) fn set_fishing_pause(bot: &BotDirEntry, paused: bool) -> Result<String, String> {
    let command = if paused { "/pause" } else { "/unpause" };
    let name = &bot.bot_name;
    let i18n = crate::config::Config::get().text();
    let Some(status) = query_bot_status(bot, Some(command))? else {
        return Err(i18n.bot_pause_query_unavailable(name, command));
    };
    match (paused, status.paused) {
        (true, true) => Ok(i18n.bot_pause_confirmed(name)),
        (false, false) => Ok(i18n.bot_unpause_confirmed(name)),
        (_, paused) => {
            let current = if paused {
                i18n.bot_pause_paused()
            } else {
                i18n.bot_pause_unpaused()
            };
            Err(i18n.bot_line(name, &i18n.bot_pause_status_mismatch(command, current)))
        }
    }
}
