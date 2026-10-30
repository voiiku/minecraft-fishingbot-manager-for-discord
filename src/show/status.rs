//! `/show status`：汇总反 AFK 与运行中、排队中的机器人状态。

use crate::all_bots_control::is_anti_afk_running;
use crate::text;
use crate::utils::{
    Report, StartupQueue, all_bot_dir_entries, screen_session_snapshot,
    split_bot_entries_by_token_status, status_row,
};

/// 汇总机器人状态（`/show status`）。
pub(crate) fn all_running_bots_status() -> Result<String, String> {
    let i18n_text = text::get();
    let active_sessions = screen_session_snapshot()?;
    let (token_ready_bot_entries, _) = split_bot_entries_by_token_status(&all_bot_dir_entries()?);
    let status_rows: Vec<String> = token_ready_bot_entries
        .iter()
        .filter_map(|entry| status_row(entry, &active_sessions))
        .collect();
    let queued_bot_names: Vec<String> = StartupQueue::get_or_init()
        .bots_in_queue(&token_ready_bot_entries)
        .into_iter()
        .filter_map(|(bot_dir_entry, in_queue)| in_queue.then_some(bot_dir_entry.bot_name))
        .collect();

    let mut report = Report::new();
    report.set_title(i18n_text.status_title());
    report.basics.push(
        i18n_text
            .anti_afk_status(is_anti_afk_running()?)
            .to_string(),
    );

    if status_rows.is_empty() && queued_bot_names.is_empty() {
        report
            .basics
            .push(i18n_text.status_no_running_bots().to_string());
        return Ok(report.render());
    }

    if !status_rows.is_empty() {
        report
            .basics
            .push(i18n_text.common_total_count(status_rows.len()));
        report.basics.push(
            [
                i18n_text.status_column_bot_name(),
                i18n_text.status_column_connection(),
                i18n_text.status_column_pause(),
                i18n_text.status_column_uptime(),
            ]
            .join(" | "),
        );
        report.basics.extend(status_rows);
    }

    report.queued.extend(queued_bot_names);

    Ok(report.render())
}
