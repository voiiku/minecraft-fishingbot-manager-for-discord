//! `/stop`：停止单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, is_bot_screen_running, stop_single_bot_session};

/// 取消待启动请求或停止会话；权限由路由层检查。
pub(crate) fn stop_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 请求可能已出队，取消失败时继续检查会话。
    let cancelled =
        StartupQueue::get_or_init().remove_bots_from_queue(std::slice::from_ref(bot_dir_entry));
    if cancelled.iter().any(|(_, removed)| *removed) {
        return Ok(text::get().bot_stop_cancelled_queue(bot_name));
    }

    if !is_bot_screen_running(bot_name)? {
        return Err(text::get().bot_stop_not_running(bot_name));
    }

    let outcome = stop_single_bot_session(bot_dir_entry.clone())?;
    if outcome.has_failed_lines() {
        return Err(outcome.to_failure_text());
    }

    log::info!(
        "{}",
        text::get().log_text(text::LogText::BotStopped { bot_name })
    );
    Ok(text::get().bot_stop_succeeded(bot_name))
}
