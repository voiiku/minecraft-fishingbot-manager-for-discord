//! 重启：`/restart`、`/restartall` 共用的"先停后起"编排。
//!
//! 单独一个文件，是因为它同时依赖 [`super::shutdown`]（停止）与 [`super::startup`]（拉起）：
//! 「拉起一个会话」与「先停后起」是两件事，混在启动流程里会看不清边界。

use crate::text;

use super::bot_dirs::BotDirEntry;
use super::reply::Report;
use super::running_bots::split_bot_entries_by_is_running;
use super::shutdown::{StopOutcome, stop_bot_sessions};
use super::startup::{LaunchOutcome, start_bot_sessions};

/// 重启过程的结果明细：先停后起，所以它保留着停止与启动两边的明细。
#[derive(Debug, Default)]
pub(crate) struct RestartOutcome {
    /// 因为缺 Token 而没参与重启的机器人（重启失败明细之一）。
    pub(crate) missing_token_bot_names: Vec<String>,
    /// 停止阶段的明细。
    pub(crate) stop: StopOutcome,
    /// 启动阶段的明细。
    pub(crate) launch: LaunchOutcome,
}

impl RestartOutcome {
    /// 失败明细：缺 Token、没停干净、停止阶段报错、启动失败（顺序就是回复里的顺序）。
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
        failure_lines.extend(self.launch.failure_lines.iter().cloned());

        failure_lines
    }

    /// 有没有失败。
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.missing_token_bot_names.is_empty()
            || !self.stop.still_running_bot_names.is_empty()
            || !self.stop.failure_lines.is_empty()
            || self.launch.has_failed_lines()
            || self.launch.has_skipped_running_bots()
    }

    /// 失败明细拼成单行错误提示。
    pub(crate) fn to_failure_text(&self) -> String {
        self.failure_lines()
            .into_iter()
            .chain(
                self.launch
                    .skipped_running_bot_names
                    .iter()
                    .map(|bot_name| text::get().startall_skipped_running(bot_name)),
            )
            .collect::<Vec<_>>()
            .join(" / ")
    }

    /// 明细 → 报告：成功明细来自启动侧，失败明细按上面那个顺序；计数由调用方 `set_counts`。
    pub(crate) fn into_report(self) -> Report {
        let failure_lines = self.failure_lines();

        let mut report = self.launch.into_report();
        report.failed = failure_lines;

        report
    }
}

/// 重启一批机器人：先停掉目标中正在运行的，再把可以启动的全部拉起。
///
/// 缺 Token 的目标**在停止之前**就被剔掉（只写一条失败明细）：重启是"先停再起"，而它起不来，
/// 白停一次会把一个可能还在钓鱼的账号弄下线。
/// 对原本未运行的目标等价于直接启动；停止阶段未能注销干净的会跳过重启，
/// 避免两个 JVM 争用同一账号。
pub fn restart_bot_sessions(bots_to_restart: &[BotDirEntry]) -> RestartOutcome {
    // 缺 Token 的先剔掉 —— 必须在停止之前算出来（见函数文档）。
    let (bot_entries_without_token, restartable_bot_entries): (Vec<BotDirEntry>, Vec<BotDirEntry>) =
        bots_to_restart
            .iter()
            .cloned()
            .partition(|bot_dir_entry| !bot_dir_entry.bot_token_status.is_ready());

    // 只执行一次 `screen -list`，把目标分成「原本在运行 / 原本未运行」。
    let (running_bot_entries, not_running_bot_entries) =
        split_bot_entries_by_is_running(&restartable_bot_entries);

    let stop_outcome = stop_bot_sessions(&running_bot_entries, not_running_bot_entries);

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
        launch: start_bot_sessions(&bots_to_start),
    }
}
