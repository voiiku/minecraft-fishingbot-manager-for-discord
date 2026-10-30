//! `/startall`：批量启动所有未运行的机器人（仅管理员）。

use std::time::Duration;

use crate::text;
use crate::utils::{
    BotDirEntry, StartupQueue, all_bot_dir_entries, split_bot_entries_by_is_running,
    split_bot_entries_by_token_status,
};

/// 启动所有已配置 Token 且未运行的机器人；权限由路由层检查。
pub(crate) fn start_all_bots() -> Result<String, String> {
    let i18n_text = text::get();
    let StartCandidates {
        token_ready_bot_entries,
        missing_token_bot_entries,
        running_bot_entries,
        not_running_bot_entries,
    } = start_candidates()?;
    let token_ready_bot_count = token_ready_bot_entries.len();

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

    let startup_queue = StartupQueue::get_or_init();
    let results = startup_queue.start_bots(&not_running_bot_entries);
    let skipped_running_count = results.skipped_running_bot_names().len();
    let cancelled_bot_count = results.cancelled_bot_names().len();
    let mut report = results.into_report();
    report.set_title(i18n_text.startall_title());

    let started_bot_count = report.succeeded.len();
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
        (i18n_text.count_start_cancelled(), cancelled_bot_count),
        (i18n_text.count_start_failed(), start_failure_count),
    ]);

    report.basics.push(i18n_text.command_start_queue_summary(
        startup_queue.max_batch_size(),
        startup_queue.bot_login_interval_ms(),
        startup_queue.batch_login_interval_ms(),
    ));

    // 跳过与取消数量已在计数行中，避免重复显示。
    report.skipped.clear();

    Ok(report.render())
}

/// 生成 `/startall` 的进度回执；无法获取候选时返回通用回执。
pub(crate) fn planned_startall_notice() -> String {
    let i18n_text = text::get();
    let Ok(candidates) = start_candidates() else {
        return i18n_text.command_start_queued_notice("/startall");
    };

    // 预估需包含已经排队的请求。
    let startup_queue = StartupQueue::get_or_init();
    let bot_count = candidates.not_running_bot_entries.len();
    let queued_bot_count = startup_queue.pending_bot_count();
    let estimated_minutes = to_minutes(
        startup_queue.slowest_start_duration(queued_bot_count.saturating_add(bot_count)),
    );
    let timeout_minutes = to_minutes(startup_queue.result_timeout());

    i18n_text.command_startall_queued_notice(
        bot_count,
        queued_bot_count,
        estimated_minutes,
        timeout_minutes,
    )
}

/// 时长按分钟向上取整，至少为 1 分钟。
fn to_minutes(duration: Duration) -> u64 {
    duration.as_secs().div_ceil(60).max(1)
}

/// `/startall` 进度回执与启动流程共用的候选分组。
struct StartCandidates {
    /// 有 Token 的机器人。
    token_ready_bot_entries: Vec<BotDirEntry>,
    /// 缺 Token 的机器人（跳过）。
    missing_token_bot_entries: Vec<BotDirEntry>,
    /// 已经在运行的机器人（跳过）。
    running_bot_entries: Vec<BotDirEntry>,
    /// 这次真正要启动的机器人。
    not_running_bot_entries: Vec<BotDirEntry>,
}

/// 按凭据与运行状态划分启动候选。
fn start_candidates() -> Result<StartCandidates, String> {
    let (token_ready_bot_entries, missing_token_bot_entries) =
        split_bot_entries_by_token_status(&all_bot_dir_entries()?);
    let (running_bot_entries, not_running_bot_entries) =
        split_bot_entries_by_is_running(&token_ready_bot_entries)?;

    Ok(StartCandidates {
        token_ready_bot_entries,
        missing_token_bot_entries,
        running_bot_entries,
        not_running_bot_entries,
    })
}
