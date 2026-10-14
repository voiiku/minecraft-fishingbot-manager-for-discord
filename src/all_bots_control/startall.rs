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
        crate::config::Config::get().startup_queue.max_batch_size,
        crate::config::Config::get().startup_queue.login_interval_ms,
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

/// `/startall` 的第一次回复，包含预计时间。
/// 失败时回复通用回执。
pub(crate) fn planned_startall_notice() -> String {
    let i18n_text = text::get();
    let Ok(all_bot_entries) = all_bot_dir_entries() else {
        return i18n_text.command_start_queued_notice("/startall");
    };

    let (token_ready_bot_entries, _) = split_bot_entries_by_token_status(&all_bot_entries);
    let (_, not_running_bot_entries) = split_bot_entries_by_is_running(&token_ready_bot_entries);
    let bot_count = not_running_bot_entries.len();

    i18n_text.command_startall_queued_notice(bot_count, expected_start_duration_secs(bot_count))
}

/// 预计耗时
/// 整条请求分批登录（两次登录之间至少隔 `login_interval_ms`），最后一次登录之后还要复核两遍。
fn expected_start_duration_secs(bot_count: usize) -> u64 {
    if bot_count == 0 {
        return 0;
    }

    let startup_queue = &crate::config::Config::get().startup_queue;
    let settle_per_bot_ms = crate::config::Config::get().timing.settle_per_bot_ms;
    let last_batch_size = bot_count.min(startup_queue.max_batch_size.max(1)) as u64;
    let total_ms = (bot_count as u64 - 1).saturating_mul(startup_queue.login_interval_ms)
        + 2 * settle_per_bot_ms * last_batch_size;

    total_ms.div_ceil(1_000)
}
