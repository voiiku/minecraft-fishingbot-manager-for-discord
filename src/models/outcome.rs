//! 启动、停止与重启结果。

use super::BotDirEntry;

pub(crate) type BotStartEntry = (BotDirEntry, BotStartOutcome);

/// 机器人启动结果及已确认进服的名称。
#[derive(Debug, Default)]
pub struct BotStartResults {
    entries: Vec<BotStartEntry>,
    online_confirmed_bot_names: Vec<String>,
}

/// 单台机器人的启动结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotStartOutcome {
    /// 启动命令已发送；复核失败时改为失败。
    Succeeded,
    /// 机器人已运行，跳过启动。
    Skipped,
    /// 启动前已取消，不计入失败。
    Cancelled,
    /// 启动失败及可用于回复的原因。
    Failure(String),
}

impl BotStartResults {
    pub(crate) fn push(&mut self, bot_dir_entry: BotDirEntry, outcome: BotStartOutcome) {
        self.entries.push((bot_dir_entry, outcome));
    }

    pub(crate) fn mark_online_confirmed(&mut self, bot_name: &str) {
        self.online_confirmed_bot_names.push(bot_name.to_string());
    }

    pub(crate) fn online_confirmed(&self, bot_name: &str) -> bool {
        self.online_confirmed_bot_names
            .iter()
            .any(|name| name == bot_name)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &BotStartEntry> {
        self.entries.iter()
    }

    pub(crate) fn set_outcome(&mut self, bot_name: &str, outcome: BotStartOutcome) {
        if let Some((_, stored)) = self
            .entries
            .iter_mut()
            .find(|(bot_dir_entry, _)| bot_dir_entry.bot_name == bot_name)
        {
            *stored = outcome;
        }
    }

    pub(crate) fn has_failed_lines(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, BotStartOutcome::Failure(_)))
    }

    pub(crate) fn has_skipped_running_bots(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, BotStartOutcome::Skipped))
    }

    pub(crate) fn failure_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|(_, outcome)| match outcome {
                BotStartOutcome::Failure(failure_line) => Some(failure_line.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn skipped_running_bot_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, outcome)| matches!(outcome, BotStartOutcome::Skipped))
            .map(|(bot_dir_entry, _)| bot_dir_entry.bot_name.as_str())
            .collect()
    }

    pub(crate) fn has_cancelled_bots(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, BotStartOutcome::Cancelled))
    }

    pub(crate) fn cancelled_bot_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, outcome)| matches!(outcome, BotStartOutcome::Cancelled))
            .map(|(bot_dir_entry, _)| bot_dir_entry.bot_name.as_str())
            .collect()
    }

    pub(crate) fn to_failure_text(&self) -> String {
        self.failure_lines().join(" / ")
    }
}

/// 停止过程明细。
#[derive(Debug, Default)]
pub(crate) struct StopOutcome {
    pub(crate) stopped_bot_names: Vec<String>,
    pub(crate) still_running_bot_names: Vec<String>,
    pub(crate) failure_lines: Vec<String>,
}

impl StopOutcome {
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.still_running_bot_names.is_empty() || !self.failure_lines.is_empty()
    }
}

/// 重启过程的停止与启动明细。
#[derive(Debug, Default)]
pub(crate) struct RestartOutcome {
    pub(crate) missing_token_bot_names: Vec<String>,
    pub(crate) stop: StopOutcome,
    pub(crate) launch: BotStartResults,
}

impl RestartOutcome {
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.missing_token_bot_names.is_empty()
            || !self.stop.still_running_bot_names.is_empty()
            || !self.stop.failure_lines.is_empty()
            || self.launch.has_failed_lines()
            || self.launch.has_skipped_running_bots()
    }

    pub(crate) fn has_cancelled_bots(&self) -> bool {
        self.launch.has_cancelled_bots()
    }
}
