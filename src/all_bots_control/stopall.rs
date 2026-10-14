//! `/stopall`：批量停止所有运行中的机器人（仅管理员）。

use crate::text;
use crate::utils::stop_all_bot_sessions;

/// 停止所有运行中的机器人，包含缺少 Token 的会话。
pub(crate) fn stop_all_bots() -> Result<String, String> {
    let mut report = stop_all_bot_sessions()?.into_report();
    report.set_title(text::get().stopall_title());

    Ok(report.render())
}
