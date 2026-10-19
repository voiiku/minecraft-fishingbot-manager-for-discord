//! `/start`：启动单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, is_bot_screen_running};

/// 启动单个机器人（`/start`）。
///
/// 启动单个机器人；确认 Token 已配置且会话未运行。
/// 权限由路由层检查，启动流程与 `/startall` 共用。
/// 回复直接使用队列的复核结果：日志确认进服、会话存在但状态未确认、启动失败三种结论之一。
pub(crate) fn start_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let i18n_text = text::get();

    // 未配置 Token 时不启动。
    if !bot_dir_entry.bot_token_status.is_ready() {
        return Err(i18n_text.bot_start_missing_token(bot_name));
    }
    // 避免重复启动。
    if is_bot_screen_running(bot_name) {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }
    let startup_queue = StartupQueue::get_or_init();

    // 已经在启动队列里不再重复排队。
    if startup_queue.bot_in_queue(bot_dir_entry) {
        return Err(i18n_text.bot_start_already_queued(bot_name));
    }

    // 启动并检查会话。
    let start_report = startup_queue.start_bots(std::slice::from_ref(bot_dir_entry));
    if start_report.has_failed_lines() {
        return Err(start_report.to_failure_text());
    }
    // 尚未拉起即被"停止全部机器人"取消：回报取消。
    if start_report.has_cancelled_bots() {
        return Err(i18n_text.start_cancelled(bot_name));
    }
    if start_report.has_skipped_running_bots() {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }

    // 复核结果已经包含“已确认进服”或“会话存在但状态未确认”的结论；
    // 单机器人只取成功明细的第一行，不额外拼报告。
    let report = start_report.into_report();
    if report.succeeded.is_empty() {
        return Err(i18n_text.start_failed(bot_name));
    }

    Ok(report.succeeded.join("\n"))
}
