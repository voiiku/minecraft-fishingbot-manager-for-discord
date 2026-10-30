//! 进程内收尾：`/stop`、`/stopall`、进程退出收尾，以及重启流程的停止阶段
//! （[`crate::utils::restart_bot_sessions`] 直接复用这里的 [`stop_bot_sessions`]）。
//!
//! "全部停止"（[`stop_all_bot_sessions`]）先清空启动队列：上述入口的语义都是全停，排队待启动的机器人一并撤销，
//! 否则启动线程会继续将其拉起。
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
    force_quit_session, screen_session_snapshot, send_stop_signal_to_session, session_name,
};
use super::startup::StartupQueue;

/// 停止过程的内部明细，供启动管道判断哪些可以重新拉起。
/// 提供按自身状态取回复/文案的方法；须由外部显式调用，工具函数层不返回成品报告。
#[derive(Debug, Default)]
pub(crate) struct StopOutcome {
    /// 确认已停止的机器人名。
    pub(crate) stopped_bot_names: Vec<String>,
    /// 停止后仍在运行的机器人名。
    pub(crate) still_running_bot_names: Vec<String>,
    /// 执行过程中的错误信息。
    pub(crate) failure_lines: Vec<String>,
}

impl StopOutcome {
    /// 是否存在失败：收尾后仍有会话在运行，或有错误明细。
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.still_running_bot_names.is_empty() || !self.failure_lines.is_empty()
    }

    /// 把失败明细拼成单行错误提示（`/stop` 这类只回一行的指令直接将其作为 `Err`）。
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

    /// 按自身信息返回统一报告：基础信息与各类明细（标题由调用方设置）。
    /// 由外部显式调用；调用即消耗自身（明细并入报告）。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new();

        let i18n_text = text::get();
        report.set_counts(&[
            (i18n_text.count_stopped(), self.stopped_bot_names.len()),
            (
                i18n_text.count_stop_failed(),
                self.still_running_bot_names.len() + self.failure_lines.len(),
            ),
        ]);

        // 停止已当场复核（会话确实消失），因此这里可以断言"已停止"。
        report.succeeded = self
            .stopped_bot_names
            .iter()
            .map(|bot_name| i18n_text.stop_succeeded_line(bot_name))
            .collect();

        for bot_name in &self.still_running_bot_names {
            report.failed.push(i18n_text.stop_still_running(bot_name));
        }
        report.failed.extend(self.failure_lines);

        report
    }
}

/// 停止全部运行中的机器人，包含 Token 已失效的孤儿进程。
/// 用于 `/stopall` 与进程退出收尾（含看门狗的掉线收尾）。只返回原始明细（[`StopOutcome`]），报告由调用方拼装。
///
/// 顺序固定为：请求启动线程停止拉起 → 清空等待列表 → 关闭全部会话。
/// 关闭后再次扫描会话：启动线程可能在首次快照之后才拉起某一台（正在拉起的这一台无法拦截），由该遍收尾。
pub(crate) fn stop_all_bot_sessions() -> Result<StopOutcome, String> {
    let startup_queue = StartupQueue::get_or_init();
    // 1. 请求停止：启动线程不再取新批；正在处理的批次把剩余条目记为"取消"（见 `starter.rs`）。
    startup_queue.request_stop();
    // 2. 清空等待列表：尚未被取走的条目就此取消（等待账本者由账本回收，见 `ledger.rs`）。
    startup_queue.remove_all_bots_from_queue();

    let all_bot_entries = all_bot_dir_entries()?;
    let (running_bot_entries, _) = split_bot_entries_by_is_running(&all_bot_entries)?;

    // 3. 关掉全部会话（所有停止入口的唯一实现：注入 Ctrl+C → 超时强杀 → 复核）。
    let mut outcome = stop_bot_sessions(&running_bot_entries)?;

    // 4. 再次扫描：首次快照之后才出现的会话在此收掉（第一遍未停掉的不重试，结论已在明细中）。
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

/// 第二遍要收的机器人：首次扫描之后新出现的会话（按名字排除首次已处理者）。
/// 无新出现的会话时返回空，第二遍不会额外等待退出。
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

/// 停止单个机器人对应的会话（供 `/stop` 使用）。
/// 调用方负责先确认该会话确实在运行，并将已鉴权的条目传入；同样只返回原始明细。
pub(crate) fn stop_single_bot_session(bot_dir_entry: BotDirEntry) -> Result<StopOutcome, String> {
    stop_bot_sessions(&[bot_dir_entry])
}

/// 停止一批会话：发送 Ctrl+C 等待 JVM 正常退出，超时则强杀，最后复核一次。
///
/// 这是所有停止入口的唯一实现（`/stop`、`/stopall`、重启的停止阶段、进程收尾）。
/// `entries_to_stop` 为要停的那批（原本未运行的机器人不必传入：报告只记停止的成功与失败）。
/// 两种等待都直接读取配置：正常退出等待（`stop_grace`）与注销缓冲（`stop_settle`）均在本模块。
/// 这两个键名不带 `batch_` 前缀，因为它们同时服务于单机与批量（单机即长度为 1 的切片）。
pub(crate) fn stop_bot_sessions(entries_to_stop: &[BotDirEntry]) -> Result<StopOutcome, String> {
    let mut outcome = StopOutcome::default();

    if entries_to_stop.is_empty() {
        return Ok(outcome);
    }

    // 1. 先统一算出会话名：拼不出会话名的目录（名字含非法字符）记一条失败明细，不参与后续步骤。
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

    // 2. 让这批会话退出：先统一注入 Ctrl+C，等待配置的正常退出时长，仍未退出者强杀。
    //    所有停止入口都复用本函数，停止行为因此一致；等待只发生一次，耗时与台数无关（单机即长度为 1 的切片）。
    let target_session_names: Vec<&str> = target_sessions
        .iter()
        .map(|(_, target_session_name)| target_session_name.as_str())
        .collect();
    for target_session_name in &target_session_names {
        if let Err(message) = send_stop_signal_to_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    thread::sleep(Duration::from_millis(
        config::Config::get().timing.stop_grace_ms,
    ));

    // 强杀尚未退出的会话（先前发信号的那批是同时等待的，因此这里只需一次快照）。
    let still_alive_sessions = screen_session_snapshot()?;
    for target_session_name in target_session_names
        .iter()
        .filter(|target_session_name| still_alive_sessions.contains_key(**target_session_name))
    {
        if let Err(message) = force_quit_session(target_session_name) {
            outcome.failure_lines.push(message);
        }
    }

    // 3. 留出会话注销时间，再复核一次，确认账号已释放。
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
            // 会话结束：同时清除这份状态记忆（其后按初始值计算，而不沿用上一轮的"已暂停"）。
            bot_name.forget_bot_pause_status();
        }
    }

    Ok(outcome)
}
