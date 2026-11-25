//! 根据 screen 会话筛选机器人，并检查是否允许修改其目录或凭据。

use super::bot_dirs::{BotDirEntry, token_ready_bot_dir_entries};
use super::screen::{is_bot_screen_running, screen_session_snapshot, session_name};
use crate::runtime::bot_startup::StartupQueue;

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

/// 已配置 Token 且有 screen 会话的机器人名；死残壳也计入。
pub fn token_ready_bot_names_with_screen_sessions() -> Result<Vec<String>, String> {
    Ok(token_ready_bot_entries_with_screen_sessions()?
        .into_iter()
        .map(|bot_dir_entry| bot_dir_entry.bot_name)
        .collect())
}

/// 使用一次 screen 快照筛选有 Token 和会话的条目，包括死残壳。
pub(crate) fn token_ready_bot_entries_with_screen_sessions() -> Result<Vec<BotDirEntry>, String> {
    let (running_bot_entries, _) =
        split_bot_entries_by_is_running(&token_ready_bot_dir_entries()?)?;
    Ok(running_bot_entries)
}

/// 会话存在或机器人正在排队时返回拒绝文案。
pub fn refusal_if_not_stopped(bot_dir_entry: &BotDirEntry) -> Result<(), String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if is_bot_screen_running(bot_name)? {
        return Err(crate::config::Config::get()
            .text()
            .refusal_bot_running(bot_name));
    }

    // 排队中的机器人会被拉起：此时修改其凭据或目录会与正在进行的启动冲突。
    if StartupQueue::get_or_init().bot_in_queue(bot_dir_entry) {
        return Err(crate::config::Config::get()
            .text()
            .refusal_bot_queued(bot_name));
    }

    Ok(())
}
