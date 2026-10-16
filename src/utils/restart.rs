//! 重启。

use crate::text;

use super::bot_dirs::BotDirEntry;
use super::reply::Report;
use super::running_bots::split_bot_entries_by_is_running;
use super::shutdown::{StopOutcome, stop_bot_sessions};
use super::startup::{BotStartResults, StartupQueue};

/// 重启过程的结果明细：先停后起，所以它保留着停止与启动两边的明细。
#[derive(Debug, Default)]
pub(crate) struct RestartOutcome {
    /// 因为缺 Token 而没参与重启的机器人（重启失败明细之一）。
    pub(crate) missing_token_bot_names: Vec<String>,
    /// 停止阶段的明细。
    pub(crate) stop: StopOutcome,
    /// 启动阶段的明细。
    pub(crate) launch: BotStartResults,
}

impl RestartOutcome {
    /// 失败明细：缺 Token、停止阶段报错、启动失败（顺序就是回复里的顺序）。
    fn failure_lines(&self) -> Vec<String> {
        let i18n_text = text::get();

        let mut failure_lines: Vec<String> = self
            .missing_token_bot_names
            .iter()
            .map(|bot_name| i18n_text.restart_skipped_missing_token(bot_name))
            .collect();
        failure_lines.extend(
            self.stop
                .still_running_bot_names
                .iter()
                .map(|bot_name| i18n_text.restart_skipped_session_alive(bot_name)),
        );
        failure_lines.extend(self.stop.failure_lines.iter().cloned());
        failure_lines.extend(self.launch.failure_lines());

        failure_lines
    }

    /// 有没有失败。被取消的启动不算失败（见 [`BotStartOutcome::Cancelled`]）。
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.missing_token_bot_names.is_empty()
            || !self.stop.still_running_bot_names.is_empty()
            || !self.stop.failure_lines.is_empty()
            || self.launch.has_failed_lines()
            || self.launch.has_skipped_running_bots()
    }

    /// 有没有被"停止全部机器人"撤下来的启动。
    pub(crate) fn has_cancelled_bots(&self) -> bool {
        self.launch.has_cancelled_bots()
    }

    /// 失败明细拼成单行错误提示。
    pub(crate) fn to_failure_text(&self) -> String {
        self.failure_lines()
            .into_iter()
            .chain(
                self.launch
                    .skipped_running_bot_names()
                    .into_iter()
                    .map(|bot_name| text::get().startall_skipped_running(bot_name)),
            )
            .collect::<Vec<_>>()
            .join(" / ")
    }

    /// 将明细转换为报告
    pub(crate) fn into_report(self) -> Report {
        let failure_lines = self.failure_lines();
        // 被"停止全部机器人"取消的启动需要保留：它不属于"跳过（已在运行）"，否则会被下一行一并清除。
        let cancelled_lines: Vec<String> = self
            .launch
            .cancelled_bot_names()
            .into_iter()
            .map(|bot_name| text::get().start_cancelled(bot_name))
            .collect();

        let mut report = self.launch.into_report();
        report.failed = failure_lines;
        // `/restartall` 只报成功与失败，跳过明细里只留被取消的那几条。
        report.skipped = cancelled_lines;

        report
    }
}

/// 重启一批机器人：先停掉目标中正在运行的，再启动。
/// 缺 Token 的目标直接写入失败明细。
/// 停止阶段未能注销干净的会跳过，避免竞争同一个账号。
pub(crate) fn restart_bot_sessions(bots_to_restart: &[BotDirEntry]) -> RestartOutcome {
    // 缺 Token 的先剔掉 —— 必须在停止之前算出来（见函数文档）。
    let (bot_entries_without_token, restartable_bot_entries): (Vec<BotDirEntry>, Vec<BotDirEntry>) =
        bots_to_restart
            .iter()
            .cloned()
            .partition(|bot_dir_entry| !bot_dir_entry.bot_token_status.is_ready());

    // 只执行一次 `screen -list`，把目标分成「原本在运行 / 原本未运行」。
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&restartable_bot_entries);

    let stop_outcome = stop_bot_sessions(&running_bot_entries);

    // 停止后仍在运行的跳过重启，其余（停成功的与原本未运行的）统一启动。
    let bots_to_start: Vec<BotDirEntry> = restartable_bot_entries
        .iter()
        .filter(|bot_dir_entry| {
            !stop_outcome
                .still_running_bot_names
                .contains(&bot_dir_entry.bot_name)
        })
        .cloned()
        .collect();

    RestartOutcome {
        missing_token_bot_names: bot_entries_without_token
            .iter()
            .map(|bot_dir_entry| bot_dir_entry.bot_name.clone())
            .collect(),
        stop: stop_outcome,
        launch: StartupQueue::get_or_init().start_bots(&bots_to_start),
    }
}
