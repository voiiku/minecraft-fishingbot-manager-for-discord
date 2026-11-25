use crate::config::Config;
use crate::operations::{PreparedControl, require_admin};
#[cfg(test)]
use crate::runtime::bot_instances::bot_status::BotStatus;
use crate::runtime::{
    bot_instances::{
        bot_dirs::{BotDirEntry, BotTokenStatus, all_bot_dir_entries},
        bot_sessions::split_bot_entries_by_is_running,
    },
    bot_startup::{StartIssue, StartupQueue, VerifiedStarts, start_status_line},
};
use crate::text::report::Report;
use std::time::Duration;

pub fn start_all_bots(caller_id: u64) -> Result<String, String> {
    prepare_start_all_bots(caller_id)?.execute()
}

pub(crate) fn prepare_start_all_bots(caller_id: u64) -> Result<PreparedControl, String> {
    require_admin(caller_id)?;
    let candidates = start_candidates()?;
    let notice = start_all_bots_notice(candidates.to_start.len());
    Ok(PreparedControl::new(notice, move || execute(candidates)))
}

fn execute(candidates: StartCandidates) -> Result<String, String> {
    let i18n_text = Config::get().text();
    let StartCandidates {
        mut to_start,
        already_running,
        mut missing_token,
    } = candidates;
    // 回执发送期间账号可能被删除或移走。只复核已选目录，不重新扫描或启动新出现的账号。
    let estimated_count = to_start.len();
    to_start.retain(|bot| BotTokenStatus::from_dir(&bot.bot_dir_path).is_ready());
    missing_token += estimated_count - to_start.len();
    let token_ready_bot_count = to_start.len() + already_running;

    if token_ready_bot_count == 0 {
        return Ok(if missing_token == 0 {
            i18n_text.startall_no_valid_accounts().to_string()
        } else {
            i18n_text.startall_no_valid_accounts_with_missing(missing_token)
        });
    }
    if to_start.is_empty() {
        return Ok(i18n_text.startall_all_running().to_string());
    }

    let startup_queue = StartupQueue::get_or_init();
    let results = startup_queue.start_bots(&to_start);
    let mut report = startall_report(results, already_running, missing_token);
    report.basics.push(i18n_text.command_start_queue_summary(
        startup_queue.max_batch_size(),
        startup_queue.bot_login_interval(),
        startup_queue.batch_login_interval(),
    ));
    Ok(report.render())
}

fn startall_report(
    results: VerifiedStarts,
    mut already_running: usize,
    missing_token: usize,
) -> Report {
    let i18n_text = Config::get().text();
    let token_ready_bot_count = already_running + results.len();
    let mut report = Report::new(
        Config::get().i18n.language,
        Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.startall_title());
    let mut cancelled = 0;
    for (name, result) in results {
        match result {
            Ok(status) => report.succeeded.push(start_status_line(&name, status)),
            Err(StartIssue::AlreadyRunning) => already_running += 1,
            Err(StartIssue::Cancelled) => cancelled += 1,
            Err(StartIssue::Failure(error)) => report.failed.push(error),
        }
    }
    report.set_counts(&[
        (i18n_text.count_valid_accounts(), token_ready_bot_count),
        (i18n_text.count_start_verified(), report.succeeded.len()),
        (i18n_text.count_skipped_running(), already_running),
        (i18n_text.count_skipped_missing_token(), missing_token),
        (i18n_text.count_start_cancelled(), cancelled),
        (i18n_text.count_start_failed(), report.failed.len()),
    ]);

    report
}

/// 使用本次执行的同一份候选快照估算耗时。
fn start_all_bots_notice(bot_count: usize) -> String {
    let i18n_text = Config::get().text();

    // 预估需包含已经排队的请求。
    let startup_queue = StartupQueue::get_or_init();
    let queued_bot_count = startup_queue.pending_bot_count();
    let estimated_minutes = estimate_startall_minutes(
        startup_queue.slowest_start_duration(queued_bot_count.saturating_add(bot_count)),
        bot_count,
        Config::get().constants.status_timeout,
    );
    let timeout_minutes = ceil_minutes_with_minimum_one(startup_queue.result_timeout());

    i18n_text.command_startall_queued_notice(
        bot_count,
        queued_bot_count,
        estimated_minutes,
        timeout_minutes,
    )
}

fn estimate_startall_minutes(
    queue_duration: Duration,
    bot_count: usize,
    status_timeout: Duration,
) -> u64 {
    let status_query_budget =
        status_timeout.saturating_mul(u32::try_from(bot_count).unwrap_or(u32::MAX));
    ceil_minutes_with_minimum_one(queue_duration.saturating_add(status_query_budget))
}

/// 时长按分钟向上取整，至少为 1 分钟。
fn ceil_minutes_with_minimum_one(duration: Duration) -> u64 {
    let seconds = duration.as_secs();
    (seconds / 60 + u64::from(!seconds.is_multiple_of(60) || duration.subsec_nanos() != 0)).max(1)
}

/// `/startall` 进度回执与启动流程共用的候选分组。
struct StartCandidates {
    to_start: Vec<BotDirEntry>,
    already_running: usize,
    missing_token: usize,
}

/// 按凭据与运行状态划分启动候选。
fn start_candidates() -> Result<StartCandidates, String> {
    let mut ready = all_bot_dir_entries()?;
    let total = ready.len();
    ready.retain(|bot| bot.bot_token_status.is_ready());
    let (running, to_start) = split_bot_entries_by_is_running(&ready)?;
    Ok(StartCandidates {
        to_start,
        already_running: running.len(),
        missing_token: total - ready.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_summary_counts_each_outcome_once_and_preserves_order() {
        crate::config::init_for_test();
        let status = BotStatus {
            joined: true,
            paused: false,
            no_rod_available: false,
            has_bobber: false,
            last_fish: 0,
        };
        let report = startall_report(
            vec![
                ("goose".into(), Ok(status)),
                (
                    "duck".into(),
                    Ok(BotStatus {
                        joined: false,
                        ..status
                    }),
                ),
                ("raven".into(), Err(StartIssue::AlreadyRunning)),
                ("owl".into(), Err(StartIssue::Cancelled)),
                ("swan".into(), Err(StartIssue::Failure("test error".into()))),
            ],
            2,
            1,
        );
        let t = Config::get().text();
        assert_eq!(
            report.succeeded,
            [t.start_join_confirmed("goose"), t.start_not_joined("duck")]
        );
        assert_eq!(report.failed, ["test error"]);
        assert!(report.skipped.is_empty());
        assert_eq!(
            report.basics,
            [
                t.count_line(t.count_valid_accounts(), 7),
                t.count_line(t.count_start_verified(), 2),
                t.count_line(t.count_skipped_running(), 3),
                t.count_line(t.count_skipped_missing_token(), 1),
                t.count_line(t.count_start_cancelled(), 1),
                t.count_line(t.count_start_failed(), 1),
            ]
        );
    }

    #[test]
    fn minute_estimate_rounds_up_fractional_seconds() {
        assert_eq!(ceil_minutes_with_minimum_one(Duration::ZERO), 1);
        assert_eq!(ceil_minutes_with_minimum_one(Duration::from_secs(60)), 1);
        assert_eq!(
            ceil_minutes_with_minimum_one(Duration::from_millis(60_001)),
            2
        );
        assert_eq!(ceil_minutes_with_minimum_one(Duration::from_secs(120)), 2);
        assert_eq!(ceil_minutes_with_minimum_one(Duration::from_nanos(1)), 1);
        assert_eq!(
            ceil_minutes_with_minimum_one(Duration::MAX),
            u64::MAX / 60 + 1
        );
    }

    #[test]
    fn startall_estimate_includes_per_bot_status_queries() {
        assert_eq!(
            estimate_startall_minutes(Duration::from_secs(59), 0, Duration::from_secs(2)),
            1
        );
        assert_eq!(
            estimate_startall_minutes(Duration::from_secs(59), 1, Duration::from_secs(2)),
            2
        );
        assert_eq!(
            estimate_startall_minutes(Duration::from_secs(58), 1, Duration::from_secs(1)),
            1
        );
        assert_eq!(
            estimate_startall_minutes(Duration::from_secs(58), 1, Duration::from_secs(3)),
            2
        );
    }
}
