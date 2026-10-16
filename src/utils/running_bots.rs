//! 状态口径（是否启动看会话、是否排队看队列）、运行状态分组、运行中的名字、`/show status` 的状态快照、
//! 运行中 / 排队中拒绝文案。
//! 「是否启动」只查一次 `screen -list` 快照（[`active_screen_sessions`]），之后所有「在不在运行」的判断都基于同一份快照；
//! 「是否排队」只查一次启动队列（[`super::startup::StartupQueue`]），与日志无关。
//! 在线 / 暂停的判定在 [`super::bot_status`]，而 `/show status` 的回复格式在本模块。

use std::collections::HashMap;

use crate::text;

use super::bot_dirs::{BotDirEntry, token_ready_bot_dir_entries};
use super::bot_status::{BotPauseStatusMemory, ConnectionStatus, PauseStatus};
use super::screen::{ScreenSession, active_screen_sessions, is_bot_screen_running, session_name};
use super::startup::StartupQueue;
use super::uptime::{format_uptime, process_elapsed_seconds};

/// `/show status` 表头与数据行共用的分隔符。
const STATUS_COLUMN_SEPARATOR: &str = " | ";

/// 一个正在运行的机器人的一次性状态快照：只用于 `/show status` 的渲染。
/// 字段私有，只被 [`RunningBotInfo::into_row`] 消费一次（不复制、不存进别处、不跨调用传递）；
/// 各列的取值与列序在 [`status_table_header`] 与 [`RunningBotInfo::into_row`]。
#[derive(Debug)]
pub struct RunningBotInfo {
    /// 机器人名。
    bot_name: String,
    /// 连接那一格：正常是状态名，异常是那句报错（挑不到日志 / 读不出来 / 为空，正常运行时不会出现）。
    connection_status: String,
    /// 进程内记着的暂停状态的副本（与 `bot_status::BotPauseStatus` 那份记忆同源）。
    pause_status: PauseStatus,
    /// 会话进程已经跑了多少秒。
    running_seconds: u64,
}

/// `/show status` 的第一行（固定表头）：与 [`RunningBotInfo::into_row`] 同序。
pub fn status_table_header() -> String {
    let i18n_text = text::get();
    [
        i18n_text.status_column_bot_name(),
        i18n_text.status_column_connection(),
        i18n_text.status_column_pause(),
        i18n_text.status_column_uptime(),
    ]
    .join(STATUS_COLUMN_SEPARATOR)
}

impl RunningBotInfo {
    /// 把这一份快照拼成一行文本并消耗自己
    pub fn into_row(self) -> String {
        [
            self.bot_name,
            self.connection_status.to_string(),
            self.pause_status.to_string(),
            format_uptime(self.running_seconds),
        ]
        .join(STATUS_COLUMN_SEPARATOR)
    }

    /// 用一次 `screen -list` 快照造一份快照；
    /// `None` = 这个机器人的会话没在跑（或名字拼不出会话名）。
    /// active_sessions 是 [`active_screen_sessions`] 的结果，应该由调用者复用并传递进此函数，避免每个机器人都去查一次 `screen -list`。
    fn from_entry(
        bot_dir_entry: BotDirEntry,
        active_sessions: &HashMap<String, ScreenSession>,
    ) -> Option<Self> {
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

        Some(Self {
            bot_name: bot_dir_entry.bot_name,
            connection_status,
            pause_status,
            running_seconds: process_elapsed_seconds(session.pid).unwrap_or(0),
        })
    }
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

/// 扫描所有正在运行的机器人并返回其状态快照（`/show status` 用）。
/// 只执行一次 `screen -list` 拿会话快照，并且只对在跑的实例读日志（每个实例只读一次）；
pub fn all_running_bot_infos() -> Result<Vec<RunningBotInfo>, String> {
    let active_sessions = active_screen_sessions();

    Ok(token_ready_bot_dir_entries()?
        .into_iter()
        .filter_map(|bot_dir_entry| RunningBotInfo::from_entry(bot_dir_entry, &active_sessions))
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
    use crate::utils::bot_status::ConnectionStatus;

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

        assert!(RunningBotInfo::from_entry(bot_dir_entry, &HashMap::new()).is_none());
    }

    /// 表头与数据行逐格对应：格数与顺序都要对得上（快照字段私有，只有本模块的测试能造它）。
    #[test]
    fn status_table_header_matches_row_columns() {
        let i18n_text = text::get();
        let bot_info = RunningBotInfo {
            bot_name: "duck".to_string(),
            connection_status: ConnectionStatus::Online.to_string(),
            pause_status: PauseStatus::Fishing,
            running_seconds: 65,
        };
        let row = bot_info.into_row();

        let header = status_table_header();
        let header_cells: Vec<&str> = header.split(STATUS_COLUMN_SEPARATOR).collect();
        assert_eq!(
            header_cells,
            vec![
                i18n_text.status_column_bot_name(),
                i18n_text.status_column_connection(),
                i18n_text.status_column_pause(),
                i18n_text.status_column_uptime(),
            ]
        );

        let row_cells: Vec<String> = row
            .split(STATUS_COLUMN_SEPARATOR)
            .map(str::to_string)
            .collect();
        assert_eq!(
            row_cells,
            vec![
                "duck".to_string(),
                ConnectionStatus::Online.to_string(),
                PauseStatus::Fishing.to_string(),
                format_uptime(65),
            ]
        );

        assert_eq!(header_cells.len(), row_cells.len());
    }
}
