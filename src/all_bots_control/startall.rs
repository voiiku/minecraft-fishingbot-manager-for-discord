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

    // 批量启动并复核（建账本 → 入队 → 等齐 → 复核都由队列那一个入口完成）。
    let startup_queue = StartupQueue::get_or_init();
    let results = startup_queue.start_bots(&not_running_bot_entries);
    // 被"停止全部机器人"撤下来的那几台也进跳过明细，计数时要和"已在运行"分开（先数好，报告会吸收结果）。
    let cancelled_bot_count = results.cancelled_bot_names().len();
    let mut report = results.into_report();
    report.set_title(i18n_text.startall_title());

    // 按回复顺序设置计数。
    let started_bot_count = report.succeeded.len();
    let skipped_running_count = report.skipped.len().saturating_sub(cancelled_bot_count);
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

    // 队列的分批与间隔只有一处来源（`StartupQueue`）：回复里报的就是它的取值。
    report.basics.push(i18n_text.command_start_queue_summary(
        startup_queue.max_batch_size(),
        startup_queue.bot_login_interval_ms(),
        startup_queue.batch_login_interval_ms(),
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

/// `/startall` 的第一次回复：这次要启动几台（分批节拍在跑完的报告里报）。
/// 失败时回复通用回执。
/// `/startall` 的第一条回执：要启动几台、队列里还排着几台，以及据此算出来的耗时预估与等结果上限。
pub(crate) fn planned_startall_notice() -> String {
    let i18n_text = text::get();
    let Ok(candidates) = start_candidates() else {
        return i18n_text.command_start_queued_notice("/startall");
    };

    // 队列中已有的台数排在本批之前，一并计入预估（队列与节奏取值同源）。
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

/// 时长按分钟向上取整（回执里只报数量级，精确到秒没有意义，至少报 1 分钟）。
fn to_minutes(duration: Duration) -> u64 {
    duration.as_secs().div_ceil(60).max(1)
}

/// `/startall` 要处理的几组机器人：两处（正式启动与第一条回执）共用同一份划分。
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

/// 读机器人目录，按凭据与运行状态分组；"谁能启动"的口径只写这一处。
fn start_candidates() -> Result<StartCandidates, String> {
    let (token_ready_bot_entries, missing_token_bot_entries) =
        split_bot_entries_by_token_status(&all_bot_dir_entries()?);
    let (running_bot_entries, not_running_bot_entries) =
        split_bot_entries_by_is_running(&token_ready_bot_entries);

    Ok(StartCandidates {
        token_ready_bot_entries,
        missing_token_bot_entries,
        running_bot_entries,
        not_running_bot_entries,
    })
}
