//! `/stopall`：批量停止所有运行中的机器人（仅管理员）。

use crate::text;
use crate::utils::stop_all_bot_sessions;

/// 停止所有运行中的机器人，包含缺少 Token 的会话。
pub(crate) fn stop_all_bots() -> Result<String, String> {
    stop_all_bots_with_title(text::get().stopall_title())
}

/// 进程退出时使用的停止报告。
pub(crate) fn stop_all_bots_for_shutdown() -> Result<String, String> {
    stop_all_bots_with_title(&text::get().log_text(text::LogText::ShutdownReportTitle))
}

/// 组装停止报告。
fn stop_all_bots_with_title(title: &str) -> Result<String, String> {
    let mut report = stop_all_bot_sessions()?.into_report();
    report.set_title(title);
    Ok(report.render())
}
