//! 重启中的停止、重新排队与复核。
use super::{
    bot_dirs::BotDirEntry, bot_sessions::split_bot_entries_by_is_running, bot_status::BotStatus,
    stop::stop_bot_sessions,
};
use crate::runtime::bot_startup::{StartIssue, StartupQueue};
/// 重启过程的停止与启动明细。
#[derive(Debug, Default)]
pub(crate) struct RestartOutcome {
    pub(crate) verified: Vec<(String, BotStatus)>,
    pub(crate) cancelled: Vec<String>,
    pub(crate) failures: Vec<String>,
}

/// 重启一批机器人：先停掉目标中正在运行的，再启动。
/// 缺 Token 的目标直接写入失败明细。
/// 停止阶段未能注销干净的会跳过，避免竞争同一个账号。
pub(crate) fn restart_bot_sessions(
    bots_to_restart: &[BotDirEntry],
) -> Result<RestartOutcome, String> {
    let i18n = crate::config::Config::get().text();
    let mut outcome = RestartOutcome::default();
    let mut restartable_bot_entries = Vec::new();
    for bot in bots_to_restart {
        if bot.bot_token_status.is_ready() {
            restartable_bot_entries.push(bot.clone());
        } else {
            outcome
                .failures
                .push(i18n.restart_skipped_missing_token(&bot.bot_name));
        }
    }
    if restartable_bot_entries.is_empty() {
        return Ok(outcome);
    }

    // 只执行一次 `screen -list`，把目标分成「原本在运行 / 原本未运行」。
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&restartable_bot_entries)?;

    let stop_outcome = stop_bot_sessions(&running_bot_entries)?;

    // 停止后仍在运行的跳过重启，其余（停成功的与原本未运行的）统一启动。
    restartable_bot_entries
        .retain(|bot| !stop_outcome.still_running_bot_names.contains(&bot.bot_name));
    outcome.failures.extend(
        stop_outcome
            .still_running_bot_names
            .iter()
            .map(|name| i18n.restart_skipped_session_alive(name)),
    );
    outcome.failures.extend(stop_outcome.failure_lines);
    for (name, result) in StartupQueue::get_or_init().start_bots(&restartable_bot_entries) {
        match result {
            Ok(status) => outcome.verified.push((name, status)),
            Err(StartIssue::AlreadyRunning) => {
                outcome.failures.push(i18n.startall_skipped_running(&name))
            }
            Err(StartIssue::Cancelled) => outcome.cancelled.push(name),
            Err(StartIssue::Failure(error)) => outcome.failures.push(error),
        }
    }
    Ok(outcome)
}
