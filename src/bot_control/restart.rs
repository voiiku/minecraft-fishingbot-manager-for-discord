//! `/restart`：重启单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, is_bot_screen_running, restart_bot_sessions};

/// 重启单个机器人（`/restart`）。
///
/// 重启单个运行中的机器人；未运行时请使用 `/start`，还在排队等启动的等它起来再说。
/// 权限由路由层检查，流程与 `/restartall` 共用。
/// 停止会话会复核，启动部分只确认命令已发送。
pub(crate) fn restart_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if !is_bot_screen_running(bot_name) {
        // 还在启动队列里排着的：没有会话可重启，队列会把它拉起来。
        if StartupQueue::get_or_init().bot_in_queue(bot_dir_entry) {
            return Err(text::get().bot_start_already_queued(bot_name));
        }

        return Err(text::get().restart_not_running(bot_name));
    }

    log::info!(
        "{}",
        text::get().log_text(text::LogText::BotRestartRequested { bot_name })
    );
    // 停止、启动并复核。
    let restart_outcome = restart_bot_sessions(std::slice::from_ref(bot_dir_entry));
    if restart_outcome.has_failed_lines() {
        return Err(restart_outcome.to_failure_text());
    }
    // 重启的启动阶段被"停止全部机器人"取消：如实回报取消，不回报命令已发送。
    if restart_outcome.has_cancelled_bots() {
        return Err(text::get().start_cancelled(bot_name));
    }

    Ok(text::get().bot_restart_command_sent(bot_name))
}
