use crate::config::Config;
use crate::operations::{PreparedControl, require_admin};
use crate::runtime::{
    bot_instances::{
        bot_dirs::all_bot_dir_entries, bot_sessions::split_bot_entries_by_is_running,
        restart::restart_bot_sessions,
    },
    bot_startup::start_status_line,
};
use crate::text::report::Report;

pub fn restart_all_bots(caller_id: u64) -> Result<String, String> {
    prepare_restart_all_bots(caller_id)?.execute()
}

pub(crate) fn prepare_restart_all_bots(caller_id: u64) -> Result<PreparedControl, String> {
    require_admin(caller_id)?;
    let notice = Config::get()
        .text()
        .command_start_queued_notice("/restartall");
    Ok(PreparedControl::new(notice, execute))
}

fn execute() -> Result<String, String> {
    // 未运行的机器人由 `/startall` 负责；缺 Token 的机器人由重启流程标记为失败。
    let i18n_text = Config::get().text();
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&all_bot_dir_entries()?)?;
    let running_bot_count = running_bot_entries.len();

    if running_bot_count == 0 {
        return Ok(i18n_text.restartall_nothing_running().to_string());
    }

    let outcome = restart_bot_sessions(&running_bot_entries)?;
    let mut report = Report::new(
        Config::get().i18n.language,
        Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.restartall_title());
    report.succeeded = outcome
        .verified
        .into_iter()
        .map(|(name, status)| start_status_line(&name, status))
        .collect();
    report.skipped = outcome
        .cancelled
        .iter()
        .map(|name| i18n_text.start_cancelled(name))
        .collect();
    report.failed = outcome.failures;

    let verified_restart_count = report.succeeded.len();
    let restart_failure_count = report.failed.len();
    report.set_counts(&[
        (i18n_text.count_running_accounts(), running_bot_count),
        (i18n_text.count_restart_verified(), verified_restart_count),
        (i18n_text.count_restart_failed(), restart_failure_count),
    ]);

    Ok(report.render())
}
