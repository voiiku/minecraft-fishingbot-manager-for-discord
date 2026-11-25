//! 一次连接尝试的结束方式，以及对应的进程级处理（停止机器人、重连、退出）。

use poise::serenity_prelude as serenity;

use crate::runtime::{bot_instances::stop, discord_notices::notify_guilds, shutdown};
use crate::{config, text};

use super::{Watchdog, lock_or_panic};

/// 本次连接尝试的结束原因。
/// 前两种表示连接异常结束，最后一种表示收到退出信号。
/// [`Watchdog::wait_for_attempt`] 会等待该结束原因。
enum AttemptEnding {
    /// `client.start()` 返回：客户端被关停，或 token / Intent 无效等致命错误。
    /// 普通掉线不会导致返回（serenity 自行重连），由看门狗判定。
    ClientReturned(Result<(), serenity::Error>),
    /// 看门狗判定掉线（探活连续失败达到窗口，或首次握手超时）；原因由判定任务提交。
    WatchdogSawDisconnect,
    /// 收到退出信号（SIGTERM / Ctrl+C）；退出进程。
    ShutdownSignal,
}

/// 进程级生命周期处理：停止机器人、处理退出、记录掉线、发送重连信号，
/// 以及等待连接尝试结束、等待判定窗口和通报重连结果。
impl Watchdog {
    /// 返回进程入口等待的重连信号。
    pub fn reconnect_signal(&self) -> &tokio::sync::Notify {
        &self.reconnect_signal
    }

    /// 判定掉线后记录原因、停止全部机器人，保留掉线时刻供恢复通知使用。
    fn stop_all_bots_on_disconnect(&self, reason: String) {
        log::warn!("{reason}");
        match stop::shutdown_all_bots() {
            Ok(report) => log::info!("{report}"),
            Err(error) => log::error!("{error}"),
        }
        *lock_or_panic(
            self.offline_since.lock(),
            text::WatchdogText::OfflineSinceLockRecord,
        ) = Some(std::time::SystemTime::now());
    }

    /// 判定任务提交掉线原因并唤醒进程入口。
    /// 收尾与重建连接由入口的 [`Watchdog::wait_for_attempt`] 执行。
    pub fn request_reconnect(&self, reason: String) {
        *lock_or_panic(
            self.pending_disconnect_reason.lock(),
            text::WatchdogText::PendingDisconnectReasonLockRecord,
        ) = Some(reason);
        // 唤醒信号保留至入口返回 `select!`。
        self.reconnect_signal().notify_one();
    }

    /// 读取并清空待处理掉线原因；返回 `None` 表示该唤醒对应已结束的上一轮尝试。
    fn take_pending_disconnect_reason(&self) -> Option<String> {
        lock_or_panic(
            self.pending_disconnect_reason.lock(),
            text::WatchdogText::PendingDisconnectReasonLockRead,
        )
        .take()
    }

    fn notice_context(&self) -> Option<serenity::Context> {
        lock_or_panic(
            self.notice_ctx.lock(),
            text::WatchdogText::NoticeCtxLockSendReport,
        )
        .clone()
    }

    /// 等待本次连接尝试结束；正常运行时持续阻塞。根据结束方式执行处理：
    /// 客户端返回或看门狗判定掉线时停止机器人，收到退出信号时退出进程。
    pub async fn wait_for_attempt(&self, client: &mut serenity::Client) {
        let ending = tokio::select! {
            result = client.start() => AttemptEnding::ClientReturned(result),
            _ = self.reconnect_signal().notified() => AttemptEnding::WatchdogSawDisconnect,
            _ = wait_for_shutdown_signal() => AttemptEnding::ShutdownSignal,
        };

        match ending {
            AttemptEnding::ClientReturned(result) => {
                let rendered = match result {
                    Ok(()) => crate::bootstrap_error_text().discord_client_ended(),
                    Err(e) => crate::bootstrap_error_text().discord_client_error(&e.to_string()),
                };
                self.stop_all_bots_on_disconnect(rendered);
            }
            AttemptEnding::WatchdogSawDisconnect => {
                // 看门狗判定掉线：在此执行收尾，原因由判定任务提交。
                // 未取得原因表示该唤醒对应已结束的上一轮尝试，此时不停止机器人。
                if let Some(reason) = self.take_pending_disconnect_reason() {
                    self.stop_all_bots_on_disconnect(reason);
                }
            }
            AttemptEnding::ShutdownSignal => {
                shutdown::exit_on_shutdown_signal(self.notice_context()).await
            }
        }
    }

    /// 等待一个判定窗口后重建连接；等待期间收到退出信号时退出进程。
    pub async fn wait_before_reconnect(&self) {
        tokio::select! {
            _ = tokio::time::sleep(self.window()) => {}
            _ = wait_for_shutdown_signal() => shutdown::exit_on_shutdown_signal(self.notice_context()).await,
        }
    }

    /// 记录最近一次握手成功后的 Discord 上下文，供退出时发送报告。
    pub fn remember_notice_context(&self, ctx: &serenity::Context) {
        *lock_or_panic(
            self.notice_ctx.lock(),
            text::WatchdogText::NoticeCtxLockRecord,
        ) = Some(ctx.clone());
    }

    /// 重连成功后向所有服务器发送掉线与恢复通知，并记录发送结果。
    pub async fn notify_reconnect_success(&self, ctx: &serenity::Context) {
        let Some(offline_since) = self.take_offline_since() else {
            return;
        };
        let offline_since_text = chrono::DateTime::<chrono::Local>::from(offline_since)
            .format(config::Config::get().constants.display_timestamp_format)
            .to_string();
        let offline_secs = offline_since
            .elapsed()
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        let notice = crate::config::Config::get()
            .text()
            .reconnect_notice_body(&offline_since_text, offline_secs);

        let (notified, failed) = notify_guilds(ctx, &notice, |guild, reason| {
            log::warn!(
                "{}",
                crate::bootstrap_error_text().reconnect_notice_failed(guild, reason)
            );
        })
        .await;

        log::info!(
            "{}",
            crate::config::Config::get()
                .text()
                .reconnected_after_offline(offline_secs, notified, failed)
        );
    }

    /// 读取并清空本次掉线记录，供重连成功通知使用。
    fn take_offline_since(&self) -> Option<std::time::SystemTime> {
        lock_or_panic(
            self.offline_since.lock(),
            text::WatchdogText::OfflineSinceLockRead,
        )
        .take()
    }
}

/// 等待退出信号：`systemctl stop/restart`（SIGTERM）与 Ctrl+C（SIGINT）。
async fn wait_for_shutdown_signal() {
    let mut terminate_signal =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            // 注册 SIGTERM 失败时记录告警，并退化为等待 Ctrl+C。
            Err(e) => {
                log::warn!(
                    "{}",
                    crate::bootstrap_error_text().sigterm_register_failed(&e.to_string())
                );
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate_signal.recv() => {}
    }
}
