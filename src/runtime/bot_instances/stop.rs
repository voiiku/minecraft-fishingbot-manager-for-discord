//! 停止机器人会话并复核结果，不退出管理器进程。
use super::bot_dirs::{BotDirEntry, all_bot_dir_entries};
use super::bot_sessions::split_bot_entries_by_is_running;
use super::screen::{
    force_quit_session, screen_session_snapshot, send_stop_signal_to_session, session_name,
};
use crate::config;
use crate::runtime::bot_startup::StartupQueue;
use crate::text::report::Report;
use std::{thread, time::Duration};

/// 停止过程明细。
#[derive(Debug, Default)]
pub(crate) struct StopOutcome {
    pub(crate) stopped_bot_names: Vec<String>,
    pub(crate) still_running_bot_names: Vec<String>,
    pub(crate) failure_lines: Vec<String>,
}

/// 单台停止及登录收尾都要求确认退出，并保留停止操作中的错误。
pub(crate) fn stop_single_bot_session(bot: &BotDirEntry) -> Result<(), String> {
    let outcome = stop_bot_sessions(std::slice::from_ref(bot))?;
    let errors: Vec<_> = outcome
        .still_running_bot_names
        .iter()
        .map(|name| crate::bootstrap_error_text().stop_still_running(name))
        .chain(outcome.failure_lines)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" / "))
    }
}

impl StopOutcome {
    /// 根据停止明细生成报告；标题由调用方设置。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new(
            crate::config::Config::get().i18n.language,
            crate::config::Config::get().runtime.max_reply_chars,
        );

        let i18n_text = crate::config::Config::get().text();
        report.set_counts(&[
            (i18n_text.count_stopped(), self.stopped_bot_names.len()),
            (
                i18n_text.count_stop_failed(),
                self.still_running_bot_names.len() + self.failure_lines.len(),
            ),
        ]);

        report.succeeded = self
            .stopped_bot_names
            .iter()
            .map(|bot_name| i18n_text.stop_succeeded_line(bot_name))
            .collect();

        for bot_name in &self.still_running_bot_names {
            report
                .failed
                .push(crate::bootstrap_error_text().stop_still_running(bot_name));
        }
        report.failed.extend(self.failure_lines);

        report
    }
}

/// 停止全部运行中的机器人，包括 Token 已失效的会话。
/// 清空队列后停止会话，并再次扫描以处理首次快照后启动的机器人。
pub(crate) fn stop_all_bot_sessions() -> Result<StopOutcome, String> {
    let startup_queue = StartupQueue::get_or_init();
    startup_queue.request_stop();
    startup_queue.remove_all_bots_from_queue();

    let all_bot_entries = all_bot_dir_entries()?;
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&all_bot_entries)?;

    let mut outcome = stop_bot_sessions(&running_bot_entries)?;

    // 再次扫描仅处理首次快照后出现的会话，不重复尝试已记录失败的会话。
    let late_outcome = stop_bot_sessions(&late_running_bot_entries(
        &all_bot_entries,
        &running_bot_entries,
    )?)?;
    outcome
        .stopped_bot_names
        .extend(late_outcome.stopped_bot_names);
    outcome
        .still_running_bot_names
        .extend(late_outcome.still_running_bot_names);
    outcome.failure_lines.extend(late_outcome.failure_lines);

    Ok(outcome)
}

/// 停止全部机器人并返回可直接记录的报告；停止失败时返回错误。
pub fn shutdown_all_bots() -> Result<String, String> {
    let mut report = stop_all_bot_sessions()?.into_report();
    report.set_title(crate::config::Config::get().text().shutdown_report_title());
    Ok(report.render())
}

/// 返回首次扫描后出现的会话，排除已处理的机器人。
fn late_running_bot_entries(
    all_bot_entries: &[BotDirEntry],
    already_handled: &[BotDirEntry],
) -> Result<Vec<BotDirEntry>, String> {
    let (running_bot_entries, _) = split_bot_entries_by_is_running(all_bot_entries)?;

    Ok(running_bot_entries
        .into_iter()
        .filter(|bot_dir_entry| {
            !already_handled
                .iter()
                .any(|handled| handled.bot_name == bot_dir_entry.bot_name)
        })
        .collect())
}

/// 批量发送 Ctrl+C，等待退出并强制结束超时会话，最后复核。
/// 等待时间由 `stop_grace` 和 `stop_settle` 配置。
pub(crate) fn stop_bot_sessions(entries_to_stop: &[BotDirEntry]) -> Result<StopOutcome, String> {
    let mut outcome = StopOutcome::default();

    if entries_to_stop.is_empty() {
        return Ok(outcome);
    }

    // 无效会话名记录为失败，不参与停止。
    let mut target_sessions: Vec<(&str, String)> = Vec::with_capacity(entries_to_stop.len());
    for bot_dir_entry in entries_to_stop {
        let bot_name = bot_dir_entry.bot_name.as_str();
        match session_name(bot_name) {
            Ok(target_session_name) => target_sessions.push((bot_name, target_session_name)),
            Err(message) => {
                let message = crate::bootstrap_error_text().bot_line(bot_name, &message);
                outcome.failure_lines.push(message);
            }
        }
    }

    // 会话共享等待时间，避免等待时长随数量增长。
    for (_, target_session_name) in &target_sessions {
        if let Err(message) = send_stop_signal_to_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    thread::sleep(Duration::from_millis(
        config::Config::get().timing.stop_grace_ms,
    ));

    let still_alive_sessions = screen_session_snapshot()?;
    for (_, target_session_name) in &target_sessions {
        if !still_alive_sessions.contains_key(target_session_name) {
            continue;
        }
        if let Err(message) = force_quit_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    // 等待会话注销后复核。
    thread::sleep(Duration::from_millis(
        config::Config::get().timing.stop_settle_ms,
    ));
    let final_sessions = screen_session_snapshot()?;
    for (bot_name, target_session_name) in &target_sessions {
        if final_sessions.contains_key(target_session_name) {
            outcome
                .still_running_bot_names
                .push((*bot_name).to_string());
        } else {
            outcome.stopped_bot_names.push((*bot_name).to_string());
        }
    }

    Ok(outcome)
}
