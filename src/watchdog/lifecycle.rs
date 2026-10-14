//! 一次连接尝试的结束方式，以及随之而来的进程级收尾（停机、重连、退出）。

use poise::serenity_prelude as serenity;

use crate::all_bots_control::stop_all_bots_for_shutdown;
use crate::text;
use crate::utils::IntoReplyText;

use super::Watchdog;

/// 这一次尝试是怎么结束的（仅本模块内部用）：只有尝试结束了才有它；正常运行时三个分支谁都不发生，
/// [`Watchdog::wait_for_attempt`] 会一直挂着。变体名说明"是谁结束了它"。
enum AttemptEnding {
    /// `client.start()` 返回了：客户端被关停，或 shard 启动失败与 token / Intent 非法这类致命错误。
    /// 普通掉线不会让它返回（serenity 自己重连），掉线由看门狗判定后走下面的 `WatchdogSawDisconnect`。
    ClientReturned(Result<(), serenity::Error>),
    /// 看门狗判定掉线（探活连续失败达到判定窗口，或首次握手超时）：原因由判定侧交过来，收尾在下面做。
    WatchdogSawDisconnect,
    /// 收到退出信号（SIGTERM / Ctrl+C）：退出进程。
    ShutdownSignal,
}

/// 进程级收尾：停机、退出、掉线记账、重连信号，以及"等这一次尝试结束 / 等判定窗口 / 重连通报"。
impl Watchdog {
    /// 停止全部机器人，供进程退出时调用。
    pub fn shutdown_all_bots(&self) -> String {
        stop_all_bots_for_shutdown().to_reply_text()
    }

    /// 不可恢复错误：停止机器人并以退出码 1 结束进程。
    pub fn exit_on_permanent_failure(&self, reason: &str) -> ! {
        log::error!("{reason}");
        log::info!("{}", self.shutdown_all_bots());
        std::process::exit(1);
    }

    /// 取进程入口等待的重连信号。
    pub fn reconnect_signal(&self) -> &tokio::sync::Notify {
        &self.reconnect_signal
    }

    /// 在认定掉线时被调用以停止全部机器人并记录掉线原因与时刻（重连成功后的通报）。
    fn stop_all_bots_on_disconnect(&self, reason: String) {
        log::warn!("{reason}");
        log::info!("{}", self.shutdown_all_bots());
        *self
            .offline_since
            .lock()
            .expect("尝试记录掉线状态时，看门狗的 offline_since 字段的互斥锁被占用") =
            Some((reason, std::time::SystemTime::now()));
    }

    /// 判定掉线的一方（看门狗判定侧、入口自己的装配错误出口）用它把原因交给进程入口并叫醒入口：
    /// 收尾（停机、记账、写日志）与重建连接都由入口的 [`Watchdog::wait_for_attempt`] 做，这里不动机器人。
    pub fn request_reconnect(&self, reason: String) {
        *self
            .pending_disconnect_reason
            .lock()
            .expect("尝试记录掉线原因时，看门狗的 pending_disconnect_reason 字段的互斥锁被占用") =
            Some(reason);
        // 保留唤醒信号，直到入口回到 select。
        self.reconnect_signal().notify_one();
    }

    /// 取走判定侧交给入口的掉线原因；取不到说明这次唤醒是迟到的（收尾已经在上一轮做过）。
    fn take_pending_disconnect_reason(&self) -> Option<String> {
        self.pending_disconnect_reason
            .lock()
            .expect("尝试读取掉线原因时，看门狗的 pending_disconnect_reason 字段的互斥锁被占用")
            .take()
    }

    /// 收到退出信号后的收尾：记日志、停掉全部机器人，然后以 0 结束进程（正常退出，systemd 不会重拉）。
    pub fn exit_on_shutdown_signal(&self) -> ! {
        log::info!("{}", text::get().log_text(text::LogText::ShutdownSignal));
        log::info!("{}", self.shutdown_all_bots());
        std::process::exit(0);
    }

    /// 等这一次尝试结束，并按结束方式收尾（正常运行时它一直不返回：`client.start()` 没返回、看门狗没判
    /// 掉线、也没有退出信号，也就是连接一切正常）。两条掉线路径都在这里停机：
    /// 客户端自己返回（被关停或致命错误）就掉线收尾；看门狗判定掉线就取来判定侧交的原因再收尾；
    /// 收到退出信号就退出进程。
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
                // 看门狗判定掉线：收尾在这里做，原因由判定侧交过来。
                // 取不到原因的是迟到的唤醒（上一轮已经收过尾），那时不再动机器人。
                if let Some(reason) = self.take_pending_disconnect_reason() {
                    self.stop_all_bots_on_disconnect(reason);
                }
            }
            AttemptEnding::ShutdownSignal => self.exit_on_shutdown_signal(),
        }
    }

    /// 等一个判定窗口再重建连接；期间收到退出信号就直接退出进程。
    pub async fn wait_before_reconnect(&self) {
        tokio::select! {
            _ = tokio::time::sleep(self.window()) => {}
            _ = wait_for_shutdown_signal() => self.exit_on_shutdown_signal(),
        }
    }

    /// 掉线后又成功重连：向本机器人所在的所有服务器发送一条通知，并把结果写进日志。
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

    /// 获取本次掉线记录并清空（重连成功的通报要用，通报完就清掉）。
    fn take_offline_since(&self) -> Option<(String, std::time::SystemTime)> {
        self.offline_since
            .lock()
            .expect("尝试读取掉线状态时，看门狗的 offline_since 字段的互斥锁被占用")
            .take()
    }
}

/// 等待退出信号：`systemctl stop/restart`（SIGTERM）与 Ctrl+C（SIGINT）都算作退出信号。
async fn wait_for_shutdown_signal() {
    let mut terminate_signal =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            // 在未配置为 service 时提醒。
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

/// 发不出去的服务器不重试，只记一条告警日志。
fn log_when_reconnect_notice_failed(guild_name: &str, reason: &str) {
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::ReconnectNoticeFailed { guild_name, reason })
    );
}

/// 选通报频道：优先系统频道，其次第一个文字频道，都没有就 `None`。
fn choose_notice_channel(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Option<serenity::ChannelId> {
    let guild = ctx.cache.guild(guild_id)?;

    if let Some(system_channel_id) = guild.system_channel_id {
        return Some(system_channel_id);
    }

    // 缓存里的频道顺序每次都不同，固定取 ID 最小的那个。
    guild
        .channels
        .values()
        .filter(|channel| channel.kind == serenity::ChannelType::Text)
        .min_by_key(|channel| channel.id)
        .map(|channel| channel.id)
}
