//! 状态口径（是否启动看会话、是否排队看队列）、运行状态分组、运行中的名字、`/show status` 的数据行、
//! 运行中 / 排队中拒绝文案。
//! 「是否启动」只查一次 `screen -list` 快照（[`active_screen_sessions`]），之后所有「在不在运行」的判断都基于同一份快照；
//! 「是否排队」只查一次启动队列（[`super::startup::StartupQueue`]），与日志无关。
//! 在线 / 暂停的判定在 [`super::bot_status`]，而 `/show status` 的回复格式在本模块。

use std::collections::HashMap;

use crate::text;

use super::bot_dirs::{BotDirEntry, token_ready_bot_dir_entries};
use super::bot_status::{BotPauseStatusMemory, ConnectionStatus};
use super::screen::{ScreenSession, active_screen_sessions, is_bot_screen_running, session_name};
use super::startup::StartupQueue;
use super::uptime::{format_uptime, process_elapsed_seconds};

/// `/show status` 一行的列分隔符（表头与数据行各用一次：`名字 | 状态 | 暂停 | 时长`）。
const STATUS_COLUMN_SEPARATOR: &str = " | ";

/// 拼一台运行中的机器人的一行；会话没在跑（或名字拼不出会话名）时返回 `None`。
/// `active_sessions` 是 [`active_screen_sessions`] 的结果，由调用方传入复用，避免每台都查一次 `screen -list`。
fn status_row(
    bot_dir_entry: BotDirEntry,
    active_sessions: &HashMap<String, ScreenSession>,
) -> Option<String> {
    // 只对确实运行会话解析状态，忽略拼不出会话名的目录。
    let session = session_name(&bot_dir_entry.bot_name)
        .ok()
        .and_then(|target_session_name| active_sessions.get(&target_session_name))
        .copied()?;

    // 各个状态的来源不同：暂停状态读进程内保存的，在线状态必须读日志（结论不保存）。
    let pause_status = bot_dir_entry.bot_name.to_bot_pause_status().pause_status();
    // 连接状态分两种来源：
    // 会话是死残壳（JVM 已经被杀、只剩 screen 的空壳）直接是"进程已结束"，
    // 会话正常就当场读一次日志；三种"读不到日志"的异常直接把原因显示在这一格里。
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
            bot_dir_entry.bot_name,
            connection_status,
            pause_status.to_string(),
            format_uptime(process_elapsed_seconds(session.pid).unwrap_or(0)),
        ]
        .join(STATUS_COLUMN_SEPARATOR),
    )
}

/// 查一次 `screen -list` 快照，把全部机器人（无 Token 过滤）按 在运行 / 未运行 分组。
/// 全部停止与进程收尾用 [`super::bot_dirs::all_bot_dir_entries`] 查询，因为要考虑孤儿进程。
/// `/startall` 用 [`token_ready_bot_dir_entries`]。
pub fn split_bot_entries_by_is_running(
    bot_entries: &[BotDirEntry],
) -> (Vec<BotDirEntry>, Vec<BotDirEntry>) {
    let active_sessions = active_screen_sessions();

    let mut running_bot_entries = Vec::with_capacity(bot_entries.len());
    let mut not_running_bot_entries = Vec::new();
    for bot_dir_entry in bot_entries {
        // 拼不出会话名就按"未运行"算。
        let is_running = session_name(&bot_dir_entry.bot_name)
            .is_ok_and(|target_session_name| active_sessions.contains_key(&target_session_name));
        if is_running {
            running_bot_entries.push(bot_dir_entry.clone());
        } else {
            not_running_bot_entries.push(bot_dir_entry.clone());
        }
    }

    (running_bot_entries, not_running_bot_entries)
}

/// 所有正在运行的机器人的名字（`/commandall` 用）。
/// 只查一次 `screen -list` 快照，不读日志（批量发指令只要名字）。
pub fn all_running_bot_names() -> Result<Vec<String>, String> {
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&token_ready_bot_dir_entries()?);
    Ok(running_bot_entries
        .into_iter()
        .map(|bot_dir_entry| bot_dir_entry.bot_name)
        .collect())
}

/// 扫描所有正在运行的机器人，返回 `/show status` 的数据行（表头由调用方拼）。
/// 只执行一次 `screen -list` 获取会话快照，且仅对正在运行的实例读取日志（每个实例只读一次）。
pub fn all_running_bot_status_rows() -> Result<Vec<String>, String> {
    let active_sessions = active_screen_sessions();

    Ok(token_ready_bot_dir_entries()?
        .into_iter()
        .filter_map(|bot_dir_entry| status_row(bot_dir_entry, &active_sessions))
        .collect())
}

/// 如果该机器人不是"已停止"（有会话，或正在排队启动），给出拒绝文案
/// （供"运行中拒绝操作"的指令统一使用）。
/// 几个拒绝运行时执行的指令的回复文本都复用它（`/bot del`、`/token`、`/bot rename` 都用它）。
/// 该机器人正在运行或排队中时返回 `Err`，否则 `Ok(())`。
pub fn refusal_if_not_stopped(bot_dir_entry: &BotDirEntry) -> Result<(), String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if is_bot_screen_running(bot_name) {
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

        assert!(status_row(bot_dir_entry, &HashMap::new()).is_none());
    }
}
