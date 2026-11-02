//! `/restart`：重启单个机器人。

use crate::text;
use crate::utils::{
    BotDirEntry, BotStartOutcome, StartupQueue, is_bot_screen_running, restart_bot_sessions,
};

/// 重启运行中的机器人；权限由路由层检查，停止与启动均会复核。
pub(crate) fn restart_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if !is_bot_screen_running(bot_name)? {
        if StartupQueue::get_or_init().bot_in_queue(bot_dir_entry) {
            return Err(text::get().bot_start_already_queued(bot_name));
        }

        return Err(text::get().restart_not_running(bot_name));
    }

    log::info!(
        "{}",
        text::get().log_text(text::LogText::BotRestartRequested { bot_name })
    );
    let restart_outcome = restart_bot_sessions(std::slice::from_ref(bot_dir_entry))?;
    if restart_outcome.has_failed_lines() {
        return Err(restart_outcome.to_failure_text());
    }
    // 启动请求可能被并发的停止操作取消。
    if restart_outcome.has_cancelled_bots() {
        return Err(text::get().start_cancelled(bot_name));
    }

    let launch_confirmed = restart_outcome.launch.iter().any(|(entry, outcome)| {
        entry.bot_name == *bot_name && matches!(outcome, BotStartOutcome::Succeeded)
    });
    if !launch_confirmed {
        return Err(text::get().start_failed(bot_name));
    }

    Ok(if restart_outcome.launch.online_confirmed(bot_name) {
        text::get().bot_restart_online_confirmed(bot_name)
    } else {
        text::get().bot_restart_session_unconfirmed(bot_name)
    })
}
