//! 一次连接尝试的结束方式，以及对应的进程级处理（停止机器人、重连、退出）。

use std::time::Duration;

use poise::serenity_prelude as serenity;

use crate::text;

use super::{Watchdog, lock_or_panic};

/// 退出信号处理阶段，向各服务器发送报告的总时限。
const SHUTDOWN_REPORT_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// 停止全部机器人，返回可直接写入日志的报告。
    pub fn shutdown_all_bots(&self) -> String {
        match crate::utils::stop_all_bot_sessions() {
            Ok(outcome) => {
                let mut report = outcome.into_report();
                report.set_title(text::get().log_text(text::LogText::ShutdownReportTitle));
                report.render()
            }
            Err(message) => message,
        }
    }

    /// 处理不可恢复错误：停止机器人，并以退出码 1 结束进程。
    pub fn exit_on_permanent_failure(&self, reason: &str) -> ! {
        log::error!("{reason}");
        log::info!("{}", self.shutdown_all_bots());
        std::process::exit(1);
    }

    /// 返回进程入口等待的重连信号。
    pub fn reconnect_signal(&self) -> &tokio::sync::Notify {
        &self.reconnect_signal
    }

    /// 判定掉线后停止全部机器人，并记录掉线原因与时刻，供重连成功后通报。
    fn stop_all_bots_on_disconnect(&self, reason: String) {
        log::warn!("{reason}");
        log::info!("{}", self.shutdown_all_bots());
        *lock_or_panic(
            self.offline_since.lock(),
            text::WatchdogText::OfflineSinceLockRecord,
        ) = Some((reason, std::time::SystemTime::now()));
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

    /// 处理退出信号：记录日志、停止全部机器人并发送收尾报告，
    /// 然后以退出码 0 结束进程（正常退出，systemd 不会重启服务）。
    pub async fn exit_on_shutdown_signal(&self) -> ! {
        log::info!("{}", text::get().log_text(text::LogText::ShutdownSignal));

        let report = match crate::utils::stop_all_bot_sessions() {
            Ok(outcome) => {
                let mut report = outcome.into_report();
                report.set_title(text::get().log_text(text::LogText::ShutdownSignalReportTitle));
                report.render()
            }
            Err(message) => {
                log::warn!("{message}");
                message
            }
        };
        log::info!("{report}");

        // 仅退出信号路径发送频道报告；客户端返回或看门狗判定掉线时连接已不可用。
        // 发送总时限见 SHUTDOWN_REPORT_TIMEOUT。
        if tokio::time::timeout(SHUTDOWN_REPORT_TIMEOUT, self.send_shutdown_report(&report))
            .await
            .is_err()
        {
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::ShutdownReportTimeout {
                    seconds: SHUTDOWN_REPORT_TIMEOUT.as_secs(),
                })
            );
        }

        std::process::exit(0);
    }

    /// 向每个服务器的通报频道发送收尾报告；无 Discord 上下文时只写日志，单个服务器发送失败时记录告警。
    async fn send_shutdown_report(&self, report: &str) {
        let ctx = lock_or_panic(
            self.notice_ctx.lock(),
            text::WatchdogText::NoticeCtxLockSendReport,
        )
        .clone();
        let Some(ctx) = ctx else {
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::ShutdownReportNoContext)
            );
            return;
        };

        let mut notified = 0usize;
        let mut failed = 0usize;
        for guild_id in ctx.cache.guilds() {
            let guild_name = ctx
                .cache
                .guild(guild_id)
                .map(|guild| guild.name.clone())
                .unwrap_or_else(|| guild_id.to_string());

            let Some(channel_id) = choose_notice_channel(&ctx, guild_id) else {
                failed += 1;
                log_when_shutdown_report_failed(
                    &guild_name,
                    &text::get().log_text(text::LogText::ReconnectNoticeNoChannel),
                );
                continue;
            };

            match channel_id.say(&ctx.http, report).await {
                Ok(_) => notified += 1,
                Err(e) => {
                    failed += 1;
                    log_when_shutdown_report_failed(&guild_name, &e.to_string());
                }
            }
        }

        log::info!(
            "{}",
            text::get().log_text(text::LogText::ShutdownReportNotified {
                notified_guilds: notified,
                failed_guilds: failed,
            })
        );
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
                    Ok(()) => text::get().log_text(text::LogText::DiscordClientEnded),
                    Err(e) => text::get().log_text(text::LogText::DiscordClientError {
                        error: &e.to_string(),
                    }),
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
            AttemptEnding::ShutdownSignal => self.exit_on_shutdown_signal().await,
        }
    }

    /// 等待一个判定窗口后重建连接；等待期间收到退出信号时退出进程。
    pub async fn wait_before_reconnect(&self) {
        tokio::select! {
            _ = tokio::time::sleep(self.window()) => {}
            _ = wait_for_shutdown_signal() => self.exit_on_shutdown_signal().await,
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
        let Some((_reason, offline_since)) = self.take_offline_since() else {
            return;
        };
        let offline_since_text = chrono::DateTime::<chrono::Local>::from(offline_since)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let offline_secs = offline_since
            .elapsed()
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        let notice = text::get().log_text(text::LogText::ReconnectNoticeBody {
            offline_since: &offline_since_text,
            offline_secs,
        });

        let mut notified = 0usize;
        let mut failed = 0usize;
        for guild_id in ctx.cache.guilds() {
            let guild_name = ctx
                .cache
                .guild(guild_id)
                .map(|guild| guild.name.clone())
                .unwrap_or_else(|| guild_id.to_string());

            let Some(channel_id) = choose_notice_channel(ctx, guild_id) else {
                failed += 1;
                log_when_reconnect_notice_failed(
                    &guild_name,
                    &text::get().log_text(text::LogText::ReconnectNoticeNoChannel),
                );
                continue;
            };

            match channel_id.say(&ctx.http, &notice).await {
                Ok(_) => notified += 1,
                Err(e) => {
                    failed += 1;
                    log_when_reconnect_notice_failed(&guild_name, &e.to_string());
                }
            }
        }

        log::info!(
            "{}",
            text::get().log_text(text::LogText::ReconnectedAfterOffline {
                offline_secs,
                notified_guilds: notified,
                failed_guilds: failed,
            })
        );
    }

    /// 读取并清空本次掉线记录，供重连成功通知使用。
    fn take_offline_since(&self) -> Option<(String, std::time::SystemTime)> {
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
                    text::get().log_text(text::LogText::SigtermRegisterFailed {
                        reason: &e.to_string(),
                    })
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

/// 记录重连通知发送失败；不重试。
fn log_when_reconnect_notice_failed(guild_name: &str, reason: &str) {
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::ReconnectNoticeFailed { guild_name, reason })
    );
}

/// 记录收尾报告发送失败；不重试。
fn log_when_shutdown_report_failed(guild_name: &str, reason: &str) {
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::ShutdownReportNoticeFailed { guild_name, reason })
    );
}

/// 选择通报频道：优先系统频道，否则选择 ID 最小的文字频道；均不存在时返回 `None`。
fn choose_notice_channel(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Option<serenity::ChannelId> {
    let guild = ctx.cache.guild(guild_id)?;

    if let Some(system_channel_id) = guild.system_channel_id {
        return Some(system_channel_id);
    }

    // 缓存中的频道顺序不稳定，固定选择 ID 最小的文字频道。
    guild
        .channels
        .values()
        .filter(|channel| channel.kind == serenity::ChannelType::Text)
        .min_by_key(|channel| channel.id)
        .map(|channel| channel.id)
}
