//! 启动后的复核：会话存活满宽限期即算拉起成功，日志只用于说明是否已确认进服。
//!
//! 判定分两次（见 [`verify_batch`]）：拉起后立即判定一次（会话消失或残壳即失败），
//! 等满 `timing.check_grace` 后再判定一次（仍存活即成功）。
//! 日志不参与成败判定，只决定成功明细附注哪一种说法（见 [`BotStartResults::mark_online_confirmed`]）。

use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::Duration;

use crate::text;
use crate::utils::bot_dirs::BotDirEntry;
use crate::utils::bot_status::ConnectionStatus;
use crate::utils::reply::{BotStartOutcome, BotStartResults};
use crate::utils::screen::{ScreenSession, screen_session_snapshot, session_name};

/// 复核这一批：两次判定后把结果写回对应条目（判定依据见模块文档）。
pub(super) fn verify_batch(results: &mut BotStartResults, check_grace: Duration) {
    let launched_bot_entries = launched_bot_entries(results);
    if launched_bot_entries.is_empty() {
        return;
    }

    // 第一次判定：会话不在或残壳的当场失败，不必等满宽限期。
    let (gone_bot_entries, surviving_bot_entries) =
        match split_by_session_state(&launched_bot_entries) {
            Ok(entries) => entries,
            Err(error) => {
                record_lookup_failure(results, &launched_bot_entries, &error);
                return;
            }
        };
    for bot_dir_entry in &gone_bot_entries {
        record_failure(results, bot_dir_entry);
    }
    if surviving_bot_entries.is_empty() {
        return;
    }

    // 第二次判定：等满宽限期，仍存活的算拉起成功；日志只决定成功明细的附注。
    thread::sleep(check_grace);
    let active_session_names: HashSet<String> = match screen_session_snapshot() {
        Ok(sessions) => sessions.into_keys().collect(),
        Err(error) => {
            record_lookup_failure(results, &surviving_bot_entries, &error);
            return;
        }
    };
    for bot_dir_entry in surviving_bot_entries {
        let session_exists = session_name(&bot_dir_entry.bot_name)
            .is_ok_and(|target_session_name| active_session_names.contains(&target_session_name));
        if !session_exists {
            record_failure(results, &bot_dir_entry);
            continue;
        }

        // 会话活过了宽限期：先记下日志是否已确认进服，再写回成功。
        if matches!(
            bot_dir_entry.connection_status(),
            Ok(ConnectionStatus::Online)
        ) {
            results.mark_online_confirmed(&bot_dir_entry.bot_name);
        }
        results.set_outcome(&bot_dir_entry.bot_name, BotStartOutcome::Succeeded);
    }
}

fn record_lookup_failure(results: &mut BotStartResults, entries: &[BotDirEntry], error: &str) {
    let i18n_text = text::get();
    for entry in entries {
        let failure = i18n_text.bot_line(&entry.bot_name, error);
        log::warn!(
            "{}",
            i18n_text.log_text(text::LogText::StartVerificationFailed {
                bot_name: &entry.bot_name,
                reason: &failure,
            })
        );
        results.set_outcome(&entry.bot_name, BotStartOutcome::Failure(failure));
    }
}

/// 记一台的失败明细（原因就是那句能直接进回复的文案），同时写一条程序日志。
fn record_failure(results: &mut BotStartResults, bot_dir_entry: &BotDirEntry) {
    let failure_line = text::get().start_failed(&bot_dir_entry.bot_name);
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::StartVerificationFailed {
            bot_name: &bot_dir_entry.bot_name,
            reason: &failure_line,
        })
    );
    results.set_outcome(
        &bot_dir_entry.bot_name,
        BotStartOutcome::Failure(failure_line),
    );
}

/// 需要复核的条目：拉起命令真发出去了的那些（跳过、取消、失败的不复核）。
fn launched_bot_entries(results: &BotStartResults) -> Vec<BotDirEntry> {
    results
        .iter()
        .filter(|(_, outcome)| matches!(outcome, BotStartOutcome::Succeeded))
        .map(|(bot_dir_entry, _)| bot_dir_entry.clone())
        .collect()
}

/// 按会话状态分组：返回（会话不在或残壳的，仍存活的）；拼不出会话名的按"不在"算。
fn split_by_session_state(
    bot_entries: &[BotDirEntry],
) -> Result<(Vec<BotDirEntry>, Vec<BotDirEntry>), String> {
    let active_sessions: HashMap<String, ScreenSession> = screen_session_snapshot()?;

    Ok(bot_entries.iter().cloned().partition(|bot_dir_entry| {
        !session_name(&bot_dir_entry.bot_name)
            .ok()
            .and_then(|target_session_name| active_sessions.get(&target_session_name))
            .is_some_and(|session| !session.is_dead)
    }))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::utils::bot_dirs::BotTokenStatus;
    use crate::utils::reply::BotStartEntry;

    use super::*;

    fn bot(bot_name: &str) -> BotDirEntry {
        BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: PathBuf::from("/nonexistent"),
            bot_token_status: BotTokenStatus::Missing,
        }
    }

    /// 构造等待复核的启动结果。
    fn launched(bot_name: &str) -> BotStartEntry {
        (bot(bot_name), BotStartOutcome::Succeeded)
    }

    /// 构造拉起阶段失败的结果。
    fn failed(bot_name: &str, failure_line: &str) -> BotStartEntry {
        (
            bot(bot_name),
            BotStartOutcome::Failure(failure_line.to_string()),
        )
    }

    /// 构造一批测试结果。
    fn results(entries: Vec<BotStartEntry>) -> BotStartResults {
        let mut results = BotStartResults::default();
        for (bot_dir_entry, outcome) in entries {
            results.push(bot_dir_entry, outcome);
        }

        results
    }

    /// 复核批次仅包含“拉起命令已发送”的结果；跳过与失败的结果不参与复核。
    #[test]
    fn only_launched_bots_are_verified() {
        let results = results(vec![
            launched("duck"),
            failed("goose", "拉起命令失败"),
            (bot("crane"), BotStartOutcome::Skipped),
        ]);

        let verified_names: Vec<String> = launched_bot_entries(&results)
            .into_iter()
            .map(|bot_dir_entry| bot_dir_entry.bot_name)
            .collect();

        assert_eq!(verified_names, ["duck"]);
    }

    /// 成功明细文案由“日志是否已确认进服”决定：已确认与未确认使用不同文案。
    #[test]
    fn the_success_line_follows_the_online_mark() {
        let mut results = results(vec![launched("duck"), launched("goose")]);
        results.mark_online_confirmed("duck");

        let i18n_text = text::get();
        let report = results.into_report();

        assert_eq!(
            report.succeeded,
            [
                i18n_text.start_command_sent_confirmed("duck"),
                i18n_text.start_command_sent_unconfirmed("goose"),
            ]
        );
        assert!(report.failed.is_empty(), "两种说法都属于成功");
    }
}
