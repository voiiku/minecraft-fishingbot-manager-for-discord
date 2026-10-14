//! `/startall`：批量启动所有未运行的机器人（仅管理员）。

use crate::text;
use crate::utils::{
    all_bot_dir_entries, split_bot_entries_by_is_running, split_bot_entries_by_token_status,
    start_bot_sessions,
};

/// 启动所有已配置 Token 且未运行的机器人；权限由路由层检查。
pub(crate) fn start_all_bots() -> Result<String, String> {
    let i18n_text = text::get();
    let (token_ready_bot_entries, missing_token_bot_entries) =
        split_bot_entries_by_token_status(&all_bot_dir_entries()?);
    let token_ready_bot_count = token_ready_bot_entries.len();
    let (running_bot_entries, not_running_bot_entries) =
        split_bot_entries_by_is_running(&token_ready_bot_entries);

    // 没有可启动对象时直接返回结论。
    if token_ready_bot_count == 0 {
        return Ok(if missing_token_bot_entries.is_empty() {
            i18n_text.startall_no_valid_accounts().to_string()
        } else {
            i18n_text.startall_no_valid_accounts_with_missing(missing_token_bot_entries.len())
        });
    }
    if not_running_bot_entries.is_empty() {
        return Ok(i18n_text.startall_all_running().to_string());
    }

    // 批量启动并复核。
    let mut report = start_bot_sessions(&not_running_bot_entries).into_report();
    report.set_title(i18n_text.startall_title());

    // 按回复顺序设置计数。
    let started_bot_count = report.succeeded.len();
    let skipped_running_count = report.skipped.len();
    let start_failure_count = report.failed.len();
    report.set_counts(&[
        (i18n_text.count_valid_accounts(), token_ready_bot_count),
        (i18n_text.count_start_command_sent(), started_bot_count),
        (
            i18n_text.count_skipped_running(),
            running_bot_entries.len() + skipped_running_count,
        ),
        (
            i18n_text.count_skipped_missing_token(),
            missing_token_bot_entries.len(),
        ),
        (i18n_text.count_start_failed(), start_failure_count),
    ]);

    report.basics.push(i18n_text.command_start_queue_summary(
        crate::config::get().startup_queue.max_batch_size,
        crate::config::get().startup_queue.login_interval_ms,
    ));

    // 分别列出已运行和缺少 Token 的账号。
    report.skipped.extend(
        running_bot_entries
            .iter()
            .map(|bot_dir_entry| i18n_text.startall_skipped_running(&bot_dir_entry.bot_name))
            .chain(missing_token_bot_entries.iter().map(|bot_dir_entry| {
                i18n_text.startall_skipped_missing_token(&bot_dir_entry.bot_name)
            })),
    );

    Ok(report.render())
}
