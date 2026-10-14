//! `/stop`：停止单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, is_bot_screen_running, stop_single_bot_session};

/// 停止单个机器人（`/stop`）。
///
/// 停止单个机器人。权限由路由层检查；会话不存在时返回错误。
/// 停止流程与 `/stopall` 共用，并会复核会话是否退出。
pub(crate) fn stop_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 会话不存在时无需停止。
    if !is_bot_screen_running(bot_name) {
        return Err(text::get().bot_stop_not_running(bot_name));
    }

    // 检查停止结果。
    let outcome = stop_single_bot_session(bot_dir_entry.clone());
    if outcome.has_failed_lines() {
        return Err(outcome.to_failure_text());
    }

    log::info!(
        "{}",
        text::get().log_text(text::LogText::BotStopped { bot_name })
    );
    Ok(text::get().bot_stop_succeeded(bot_name))
}
