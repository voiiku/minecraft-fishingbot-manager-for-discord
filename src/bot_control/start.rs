//! `/start`：启动单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, is_bot_screen_running};

/// 启动单个机器人；权限由路由层检查，结果由启动队列复核。
pub(crate) fn start_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let i18n_text = text::get();

    if !bot_dir_entry.bot_token_status.is_ready() {
        return Err(i18n_text.bot_start_missing_token(bot_name));
    }
    if is_bot_screen_running(bot_name)? {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }
    let startup_queue = StartupQueue::get_or_init();

    if startup_queue.bot_in_queue(bot_dir_entry) {
        return Err(i18n_text.bot_start_already_queued(bot_name));
    }

    let start_report = startup_queue.start_bots(std::slice::from_ref(bot_dir_entry));
    if start_report.has_failed_lines() {
        return Err(start_report.to_failure_text());
    }
    if start_report.has_cancelled_bots() {
        return Err(i18n_text.start_cancelled(bot_name));
    }
    if start_report.has_skipped_running_bots() {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }

    let report = start_report.into_report();
    if report.succeeded.is_empty() {
        return Err(i18n_text.start_failed(bot_name));
    }

    Ok(report.succeeded.join("\n"))
}
