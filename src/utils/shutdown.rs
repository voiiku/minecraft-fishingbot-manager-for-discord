//! 进程内收尾：`/stop`、`/stopall`、进程退出收尾，以及重启流程的停止阶段
//! （[`crate::utils::restart_bot_sessions`] 直接复用这里的 [`stop_bot_sessions`]）。
//!
//! 流程固定为：发送 Ctrl+C → 等 JVM 自己退出 → 强杀未退出的 → 复核一次。单机与批量同一段代码，
//! 耗时是常数，因此不会超出 systemd 的 `TimeoutStopSec`。
//! 主进程被强杀或卡死时由 systemd 按 `KillMode=control-group` 收掉整个 cgroup，保证不留占着账号的孤儿 JVM。
//! 必须让会话退出，机器人占着游戏账号时 FishingBot 在线会阻止真人玩家登录。
use std::thread;
use std::time::Duration;

use crate::config;
use crate::text;

use super::bot_dirs::{BotDirEntry, all_bot_dir_entries};
use super::bot_status::BotPauseStatusMemory;
use super::reply::Report;
use super::running_bots::split_bot_entries_by_is_running;
use super::screen::{
    active_screen_sessions, force_quit_session, send_stop_signal_to_session, session_name,
};

/// 停止过程的内部明细，供启动管道判断哪些可以重新拉起。
/// 实现了根据自身状态，获取相应回复/文案的方法，必须由外部显式调用（工具函数层不返回成品报告）。
#[derive(Debug, Default)]
pub(crate) struct StopOutcome {
    /// 停止前正在运行的机器人数量。
    pub(crate) running_bot_count: usize,
    /// 确认已停止的机器人名。
    pub(crate) stopped_bot_names: Vec<String>,
    /// 停止后仍在运行的机器人名。
    pub(crate) still_running_bot_names: Vec<String>,
    /// 扫描到但原本未运行的机器人。
    pub(crate) not_running_bot_entries: Vec<BotDirEntry>,
    /// 执行过程中的错误信息。
    pub(crate) failure_lines: Vec<String>,
}

impl StopOutcome {
    /// 有没有失败：收尾后仍有会话在跑，或有错误明细。
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.still_running_bot_names.is_empty() || !self.failure_lines.is_empty()
    }

    /// 失败明细拼成单行错误提示（`/stop` 这类只回一行的指令直接拿它当 `Err`）。
    pub(crate) fn to_failure_text(&self) -> String {
        let i18n_text = text::get();

        let mut failure_lines: Vec<String> = self
            .still_running_bot_names
            .iter()
            .map(|bot_name| i18n_text.stop_still_running(bot_name))
            .collect();
        failure_lines.extend(self.failure_lines.iter().cloned());

        failure_lines.join(" / ")
    }

    /// 根据自身信息，返回统一报告：基础信息 + 各类明细（标题由调用方设置）。
    /// 由外部显式调用；调用即消耗自身（明细被报告吸收）。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new();

        let i18n_text = text::get();
        report.set_counts(&[
            (
                i18n_text.count_scanned_dirs(),
                self.running_bot_count + self.not_running_bot_entries.len(),
            ),
            (i18n_text.count_stopped(), self.stopped_bot_names.len()),
            (
                i18n_text.count_skipped_not_running(),
                self.not_running_bot_entries.len(),
            ),
            (
                i18n_text.count_stop_failed(),
                self.still_running_bot_names.len() + self.failure_lines.len(),
            ),
        ]);

        // 停止是当场复核过的（会话确实消失了），所以这里说得起"已停止"。
        report.succeeded = self
            .stopped_bot_names
            .iter()
            .map(|bot_name| i18n_text.stop_succeeded_line(bot_name))
            .collect();

        report.skipped = self
            .not_running_bot_entries
            .iter()
            .map(|bot_dir_entry| i18n_text.stop_skipped_not_running(&bot_dir_entry.bot_name))
            .collect();

        for bot_name in &self.still_running_bot_names {
            report.failed.push(i18n_text.stop_still_running(bot_name));
        }
        report.failed.extend(self.failure_lines);

        report
    }
}

/// 停止全部运行中的机器人，包含 Token 已失效的孤儿进程。
/// 用于 `/stopall` 与进程退出收尾。只返回原始明细（[`StopOutcome`]），报告由调用方拼装。
pub fn stop_all_bot_sessions() -> Result<StopOutcome, String> {
    let (running_bot_entries, not_running_bot_entries) =
        split_bot_entries_by_is_running(&all_bot_dir_entries()?);
    Ok(stop_bot_sessions(
        &running_bot_entries,
        not_running_bot_entries,
    ))
}

/// 停止单个机器人对应的会话（`/stop` 用）。
/// 调用方负责先确认该会话确实在运行，并把已鉴权的条目传进来；同样只返回原始明细。
pub fn stop_single_bot_session(bot_dir_entry: BotDirEntry) -> StopOutcome {
    stop_bot_sessions(&[bot_dir_entry], Vec::new())
}

/// 停止一批会话：发送 Ctrl+C 等待 JVM 正常退出，超时强杀，最后复核一次。
///
/// 这是所有停止入口的唯一实现（`/stop`、`/stopall`、重启的停止阶段、进程收尾）。
/// `entries_to_stop` 是要停的那批；`not_running_entries` 只是顺带带进报告
/// （算「原本未运行，无需停止」），不参与任何停止动作 —— 传它进来是为了让报告里的
/// 「扫描机器人目录」总数说得通。两种等待都自己读配置：正常退出等待（`stop_grace_ms`）
/// 与注销缓冲（`stop_settle_ms`）都在本模块。
/// 这两个键名不带 `batch_` 前缀，因为它们同时服务单机与批量（单机就是一片元素的切片）。
pub(crate) fn stop_bot_sessions(
    entries_to_stop: &[BotDirEntry],
    not_running_entries: Vec<BotDirEntry>,
) -> StopOutcome {
    let mut outcome = StopOutcome {
        running_bot_count: entries_to_stop.len(),
        not_running_bot_entries: not_running_entries,
        ..Default::default()
    };

    if entries_to_stop.is_empty() {
        return outcome;
    }

    // 1. 会话名先统一算出来：拼不出会话名的目录（名字含非法字符）记一条失败明细，不参与后面几步。
    let mut target_sessions: Vec<(&str, String)> = Vec::with_capacity(entries_to_stop.len());
    for bot_dir_entry in entries_to_stop {
        let bot_name = bot_dir_entry.bot_name.as_str();
        match session_name(bot_name) {
            Ok(target_session_name) => target_sessions.push((bot_name, target_session_name)),
            Err(message) => {
                let message = text::get().bot_line(bot_name, &message);
                outcome.failure_lines.push(message);
            }
        }
    }

    // 2. 让这批会话退出：先一起注入 Ctrl+C，等配置里的正常退出等待，仍未退出的强杀。
    //    所有停止入口都复用本函数，停止行为因此一致；等待只发生一次，耗时与台数无关，单机就是一片。
    let target_session_names: Vec<&str> = target_sessions
        .iter()
        .map(|(_, target_session_name)| target_session_name.as_str())
        .collect();
    for target_session_name in &target_session_names {
        if let Err(message) = send_stop_signal_to_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    thread::sleep(Duration::from_millis(config::get().timing.stop_grace_ms));

    // 还没退出的强杀（先发信号的那批是同时等的，所以这里只看一次快照）。
    let still_alive_sessions = active_screen_sessions();
    for target_session_name in target_session_names
        .iter()
        .filter(|target_session_name| still_alive_sessions.contains_key(**target_session_name))
    {
        if let Err(message) = force_quit_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    // 3. 留出会话注销时间，再复核一次，确认账号已释放。
    thread::sleep(Duration::from_millis(config::get().timing.stop_settle_ms));
    let final_sessions = active_screen_sessions();
    for (bot_name, target_session_name) in &target_sessions {
        if final_sessions.contains_key(target_session_name) {
            outcome
                .still_running_bot_names
                .push((*bot_name).to_string());
        } else {
            outcome.stopped_bot_names.push((*bot_name).to_string());
            // 会话没了，这份状态的记忆也一起忘掉（之后按初始值算，而不是接着上一轮的"已暂停"）。
            bot_name.forget_bot_pause_status();
        }
    }

    outcome
}
