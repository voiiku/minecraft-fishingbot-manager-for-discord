//! 机器人运行状态分组、`/show status` 数据行与拒绝文案。

use std::collections::HashMap;

use crate::text;

use super::bot_dirs::{BotDirEntry, token_ready_bot_dir_entries};
use super::bot_status::{BotPauseStatusMemory, ConnectionStatus};
use super::screen::{ScreenSession, is_bot_screen_running, screen_session_snapshot, session_name};
use super::startup::StartupQueue;
use super::uptime::{format_uptime, process_elapsed_seconds};

/// `/show status` 一行的列分隔符（表头与数据行各用一次：`名字 | 状态 | 暂停 | 时长`）。
const STATUS_COLUMN_SEPARATOR: &str = " | ";

/// 使用共享 screen 快照生成状态行；无对应会话时返回 `None`。
pub(crate) fn status_row(
    bot_dir_entry: &BotDirEntry,
    active_sessions: &HashMap<String, ScreenSession>,
) -> Option<String> {
    let session = session_name(&bot_dir_entry.bot_name)
        .ok()
        .and_then(|target_session_name| active_sessions.get(&target_session_name))
        .copied()?;

    let pause_status = bot_dir_entry.bot_name.to_bot_pause_status().pause_status();
    // 死残壳的日志可能仍显示在线，因此优先使用会话状态。
    let connection_status = if session.is_dead {
        ConnectionStatus::Stopped.to_string()
    } else {
        match bot_dir_entry.connection_status() {
            Ok(connection_status) => connection_status.to_string(),
            Err(message) => message,
        }
    };

    Some(
        [
            bot_dir_entry.bot_name.clone(),
            connection_status,
            pause_status.to_string(),
            format_uptime(process_elapsed_seconds(session.pid).unwrap_or(0)),
        ]
        .join(STATUS_COLUMN_SEPARATOR),
    )
}

/// 查一次 `screen -list` 快照，把全部机器人（无 Token 过滤）按 在运行 / 未运行 分组。
/// screen 查询失败时返回错误。
/// 全部停止与进程收尾用 [`super::bot_dirs::all_bot_dir_entries`] 查询，因为要考虑孤儿进程。
/// `/startall` 用 `token_ready_bot_dir_entries`。
pub fn split_bot_entries_by_is_running(
    bot_entries: &[BotDirEntry],
) -> Result<(Vec<BotDirEntry>, Vec<BotDirEntry>), String> {
    let active_sessions = screen_session_snapshot()?;

    let mut running_bot_entries = Vec::with_capacity(bot_entries.len());
    let mut not_running_bot_entries = Vec::new();
    for bot_dir_entry in bot_entries {
        let is_running = session_name(&bot_dir_entry.bot_name)
            .is_ok_and(|target_session_name| active_sessions.contains_key(&target_session_name));
        if is_running {
            running_bot_entries.push(bot_dir_entry.clone());
        } else {
            not_running_bot_entries.push(bot_dir_entry.clone());
        }
    }

    Ok((running_bot_entries, not_running_bot_entries))
}

/// 所有正在运行的机器人的名字（`/commandall` 用）。
/// 只查一次 `screen -list` 快照，不读日志（批量发指令只要名字）。
pub fn all_running_bot_names() -> Result<Vec<String>, String> {
    let (running_bot_entries, _) =
        split_bot_entries_by_is_running(&token_ready_bot_dir_entries()?)?;
    Ok(running_bot_entries
        .into_iter()
        .map(|bot_dir_entry| bot_dir_entry.bot_name)
        .collect())
}

/// 会话存在或机器人正在排队时返回拒绝文案。
pub fn refusal_if_not_stopped(bot_dir_entry: &BotDirEntry) -> Result<(), String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if is_bot_screen_running(bot_name)? {
        return Err(text::get().refusal_bot_running(bot_name));
    }
    // 排队中的机器人会被拉起：此时修改其凭据或目录会与正在进行的启动冲突。
    if StartupQueue::get_or_init().bot_in_queue(bot_dir_entry) {
        return Err(text::get().refusal_bot_queued(bot_name));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::bot_dirs::BotTokenStatus;

    /// 没在跑时返回 `None`：这里用非法名字让"拼会话名"这一步先失败，所以不必装真配置
    ///（另一支 —— 真在跑 —— 要读配置里的 `screen_prefix` 与 `bot_logs_dir`，单测碰不到）。
    #[test]
    fn from_entry_is_none_when_the_session_is_missing() {
        let bot_dir_entry = BotDirEntry {
            owner_id: 1,
            bot_name: "非法 名字".to_string(),
            bot_dir_path: std::path::PathBuf::from("/nonexistent"),
            bot_token_status: BotTokenStatus::Missing,
        };

        assert!(status_row(&bot_dir_entry, &HashMap::new()).is_none());
    }
}
