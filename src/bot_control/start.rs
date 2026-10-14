//! `/start`：启动单个机器人。

use crate::text;
use crate::utils::{BotDirEntry, is_bot_screen_running, start_bot_sessions};

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

    // 避免重复启动。
    if is_bot_screen_running(bot_name) {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }

    // 启动并检查会话。
    let launch_outcome = start_bot_sessions(std::slice::from_ref(bot_dir_entry));
    if launch_outcome.has_failed_lines() {
        return Err(launch_outcome.to_failure_text());
    }
    if launch_outcome.has_skipped_running_bots() {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }

    Ok(i18n_text.bot_start_command_sent(bot_name))
}
