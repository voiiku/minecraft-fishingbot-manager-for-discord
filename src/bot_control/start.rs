//! `/start`：启动单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, is_bot_screen_running};

/// 启动单个机器人（`/start`）。
///
/// 启动单个机器人；确认 Token 已配置且会话未运行。
/// 权限由路由层检查，启动流程与 `/startall` 共用。
/// 回复只确认命令已发送，不代表登录或进服成功。
pub(crate) fn start_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let i18n_text = text::get();

    // 未配置 Token 时不启动。
    if !bot_dir_entry.bot_token_status.is_ready() {
        return Err(i18n_text.bot_start_missing_token(bot_name));
    }

    let startup_queue = StartupQueue::get_or_init();

    // 避免重复启动。
    if is_bot_screen_running(bot_name) {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }
    // 已经在启动队列里等着的（例如刚被 `/startall` 排进去）不再重复排队。
    if startup_queue.bot_in_queue(bot_dir_entry) {
        return Err(i18n_text.bot_start_already_queued(bot_name));
    }

    // 启动并检查会话（建账本 → 入队 → 等齐 → 复核都由队列那一个入口完成）。
    let start_report = startup_queue.start_bots(std::slice::from_ref(bot_dir_entry));
    if start_report.has_failed_lines() {
        return Err(start_report.to_failure_text());
    }
    // 尚未拉起即被"停止全部机器人"取消：如实回报取消，不回报命令已发送。
    if start_report.has_cancelled_bots() {
        return Err(i18n_text.start_cancelled(bot_name));
    }
    if start_report.has_skipped_running_bots() {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }

    Ok(i18n_text.bot_start_command_sent(bot_name))
}
