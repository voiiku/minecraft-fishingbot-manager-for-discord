//! `/restartall`：批量重启所有运行中的机器人（仅管理员）。

use crate::text;
use crate::utils::{all_bot_dir_entries, restart_bot_sessions, split_bot_entries_by_is_running};

/// 流程与 `/restart` 共用。
pub(crate) fn restart_all_bots() -> Result<String, String> {
    // 未运行的机器人由 `/startall` 负责；缺 Token 的机器人由重启流程标记为失败。
    let i18n_text = text::get();
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&all_bot_dir_entries()?);
    let running_bot_count = running_bot_entries.len();

    if running_bot_count == 0 {
        return Ok(i18n_text.restartall_nothing_running().to_string());
    }

    // 组装重启报告。
    let mut report = restart_bot_sessions(&running_bot_entries).into_report();
    report.set_title(i18n_text.restartall_title());

    // 按回复顺序设置计数。
    let sent_restart_command_count = report.succeeded.len();
    let restart_failure_count = report.failed.len();
    report.set_counts(&[
        (i18n_text.count_running_accounts(), running_bot_count),
        (
            i18n_text.count_restart_command_sent(),
            sent_restart_command_count,
        ),
        (i18n_text.count_restart_failed(), restart_failure_count),
    ]);

    Ok(report.render())
}
