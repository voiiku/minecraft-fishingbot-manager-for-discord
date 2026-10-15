//! 看门狗的网络状态判定与 REST 探活：`GatewayState` 四态是模块内部细节。
//! 外部只管喂事件（`record_*`）与等重连信号。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use poise::serenity_prelude as serenity;

use crate::text;

use super::Watchdog;

/// 四种网络状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayState {
    /// 已握手，且探活未在连续失败。
    Alive,
    /// 尚未达到任何判断条件，等待中。
    Waiting,
    /// 掉线：本次尝试开始后一直没收到过 READY / RESUMED。
    NeverReady,
    /// 掉线：REST 探活连续失败达到判定窗口。
    Unreachable,
}

impl GatewayState {
    /// 掉线原因的日志文案（含排查方向）；未掉线时是 `None`。
    fn lost_log_text(self) -> Option<crate::text::LogText<'static>> {
        match self {
            Self::NeverReady => Some(crate::text::LogText::GatewayLostNeverReady),
            Self::Unreachable => Some(crate::text::LogText::GatewayLostUnreachable),
            Self::Alive | Self::Waiting => None,
        }
    }
}

/// 实现判定与探活任务的具体逻辑。
impl Watchdog {
    /// 开始新一轮尝试连接并清除上一轮状态。
    pub fn begin_connection_attempt(&self, now: Instant) {
        *self
            .started
            .lock()
            .expect("尝试重置本次连接尝试的起始状态时，看门狗的 started 字段的互斥锁被占用") = now;
        *self
            .last_handshake
            .lock()
            .expect("尝试清除上一次握手状态时，看门狗的 last_handshake 字段的互斥锁被占用") = None;
        *self
            .unhealthy_since
            .lock()
            .expect("尝试清除探活失败状态时，看门狗的 unhealthy_since 字段的互斥锁被占用") = None;
    }

    /// 握手成功：记录最新握手时刻，并清掉探活失败计时。
    pub fn record_handshake_success(&self, now: Instant) {
        *self
            .last_handshake
            .lock()
            .expect("尝试记录握手状态时，看门狗的 last_handshake 字段的互斥锁被占用") = Some(now);
        *self
            .unhealthy_since
            .lock()
            .expect("尝试清除探活失败状态时，看门狗的 unhealthy_since 字段的互斥锁被占用") = None;
    }

    /// 这一次是不是进程内第一次成功握手。
    pub fn is_first_handshake(&self) -> bool {
        !self.ever_handshaken.swap(true, Ordering::Relaxed)
    }

    /// 探活成功：清掉失败计时，状态随之回到 `Alive`。
    pub fn clear_probe_failures(&self) {
        *self
            .unhealthy_since
            .lock()
            .expect("尝试清除探活失败状态时，看门狗的 unhealthy_since 字段的互斥锁被占用") = None;
    }

    /// 记录首次探活失败时刻（因为从第一次失败起才开始算超时）。
    pub fn record_probe_failure_start(&self, now: Instant) {
        self.unhealthy_since
            .lock()
            .expect("尝试记录探活失败状态时，看门狗的 unhealthy_since 字段的互斥锁被占用")
            .get_or_insert(now);
    }

    /// 根据当前时刻判定网络状态（时序取自 [`Watchdog`] 的字段）。
    fn state(&self, now: Instant) -> GatewayState {
        // 先看有没有握过手。
        let handshaken = self
            .last_handshake
            .lock()
            .expect("尝试判定网络状态时，看门狗的 last_handshake 字段的互斥锁被占用")
            .is_some();
        // 没握手时还要分：超过首次握手超时判 NeverReady，否则还在 Waiting。
        if !handshaken {
            // 首次握手超时的截止点：本次尝试开始 + 首次握手超时。
            let timeout = *self
                .started
                .lock()
                .expect("尝试判定网络状态时，看门狗的 started 字段的互斥锁被占用")
                + self.first_handshake_timeout;
            return if now >= timeout {
                GatewayState::NeverReady
            } else {
                GatewayState::Waiting
            };
        }
        // 获取探活失败的起始时刻
        let unhealthy_since = *self
            .unhealthy_since
            .lock()
            .expect("尝试判定网络状态时，看门狗的 unhealthy_since 字段的互斥锁被占用");
        // 失败累计满一个窗口才判断联；没满就是网络波动。
        match unhealthy_since {
            Some(unhealthy_since) if now.duration_since(unhealthy_since) >= self.window => {
                GatewayState::Unreachable
            }
            _ => GatewayState::Alive,
        }
    }

    /// 拉起一个 REST 探活任务：每隔 [`Watchdog::probe_interval`] 取一次自身信息，
    /// 判定掉线就请求重建连接；`stop` 是这一轮专用的停止标志。
    pub fn probe(&'static self, http: Arc<serenity::Http>, stop: Arc<AtomicBool>) {
        tokio::spawn(async move {
            let interval = self.probe_interval;

            loop {
                tokio::time::sleep(interval).await;
                // 如果为 true 就表示这一轮已经结束。
                if stop.load(Ordering::Relaxed) {
                    return;
                }

                // 尝试取自身账号信息
                let probed = http.get_current_user().await;
                // 过程中被叫停就丢掉这次结果。
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                // 取到自身信息就算网络通畅。失败原因写日志。
                match probed {
                    Ok(_) => self.clear_probe_failures(),
                    Err(e) => {
                        log::warn!(
                            "{}\n",
                            text::get().log_text(text::LogText::ProbeFailed {
                                reason: &e.to_string(),
                            })
                        );
                        self.record_probe_failure_start(Instant::now());
                    }
                }
                // 判定掉线就请求重建连接（本任务结束，主循环会再拉一个探活任务）。
                if let Some(reason) = self.state(Instant::now()).lost_log_text() {
                    self.request_reconnect(text::get().log_text(reason));
                    return;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// 测试用的时序参数（值本身不重要，用例都按它们算相对时刻）。
    const WINDOW: Duration = Duration::from_secs(30);
    const PROBE_INTERVAL: Duration = Duration::from_secs(10);
    const FIRST_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

    /// 造一个"启动于 0 时刻"的看门狗，所有用例都用相对时刻断言。
    fn watchdog_at_start() -> (Watchdog, Instant) {
        let started = Instant::now();
        (
            Watchdog::new(started, WINDOW, PROBE_INTERVAL, FIRST_HANDSHAKE_TIMEOUT),
            started,
        )
    }

    /// 还没握手：首次握手超时内是 Waiting，超过它才判 NeverReady。
    #[test]
    fn missing_handshake_is_lost_after_timeout() {
        let (watchdog, started) = watchdog_at_start();

        assert_eq!(watchdog.state(started), GatewayState::Waiting);
        assert_eq!(
            watchdog.state(started + FIRST_HANDSHAKE_TIMEOUT - Duration::from_secs(1)),
            GatewayState::Waiting
        );
        assert_eq!(
            watchdog.state(started + FIRST_HANDSHAKE_TIMEOUT),
            GatewayState::NeverReady
        );
    }

    /// "看门狗已就绪"每进程只报一次：`is_first_handshake` 只在首次返回 `true`。
    #[test]
    fn only_the_first_handshake_reports_ready() {
        let (watchdog, started) = watchdog_at_start();

        assert!(watchdog.is_first_handshake());
        assert!(!watchdog.is_first_handshake());

        // 重连：`begin_connection_attempt` 清掉的是本轮记账，"报过没有"是进程级事实，不该被清。
        watchdog.begin_connection_attempt(started);
        assert!(!watchdog.is_first_handshake());
    }

    /// 握过手、探活也通：Alive。
    #[test]
    fn handshake_and_probe_are_alive() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.clear_probe_failures();

        assert_eq!(watchdog.state(started), GatewayState::Alive);
        // 时间过去很久也没关系，只要探活一直在成功（每轮都会 clear_probe_failures）。
        watchdog.clear_probe_failures();
        assert_eq!(watchdog.state(started + WINDOW * 10), GatewayState::Alive);
    }

    /// 探活连续失败：刚开始只是 Waiting（正在等窗口走满），满一个窗口才判 Unreachable。
    #[test]
    fn continuous_probe_failure_loses_after_window() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);

        assert_eq!(watchdog.state(started), GatewayState::Alive);
        assert_eq!(
            watchdog.state(started + WINDOW - Duration::from_secs(1)),
            GatewayState::Alive
        );
        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Unreachable);
    }

    /// 失败的起始时刻只记第一次：期间反复失败不会把判定无限往后推。
    #[test]
    fn repeated_failures_do_not_push_the_deadline() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);

        // 模拟探活任务一直在失败：每次失败都调一次，但起点不变。
        for _ in 0..5 {
            watchdog.record_probe_failure_start(started);
        }

        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Unreachable);
    }

    /// 判定可逆：探活恢复后立刻回到 Alive。
    #[test]
    fn recovered_probe_revives_the_watchdog() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);
        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Unreachable);

        watchdog.clear_probe_failures();

        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Alive);
    }

    /// 重新握手（掉线后重连成功）同样算健康，并清掉失败计时。
    #[test]
    fn resumed_handshake_revives_the_watchdog() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);

        let resumed_at = started + WINDOW;
        watchdog.record_handshake_success(resumed_at);

        assert_eq!(watchdog.state(resumed_at + WINDOW), GatewayState::Alive);
    }

    /// 开始新的一次尝试：首次握手超时从此刻重新算，上一轮的握手与失败记账一起清掉。
    /// 不重置的话，重连一上来就会被判 `NeverReady`（超时起点还停在进程启动那一刻）。
    #[test]
    fn beginning_a_new_attempt_restarts_the_timeout() {
        let (watchdog, started) = watchdog_at_start();
        assert_eq!(
            watchdog.state(started + FIRST_HANDSHAKE_TIMEOUT),
            GatewayState::NeverReady
        );

        let reconnected_at = started + WINDOW * 10;
        watchdog.begin_connection_attempt(reconnected_at);

        assert_eq!(
            watchdog.state(reconnected_at),
            GatewayState::Waiting,
            "刚重连时还在首次握手超时内"
        );
        assert_eq!(
            watchdog.state(reconnected_at + FIRST_HANDSHAKE_TIMEOUT),
            GatewayState::NeverReady,
            "超时是重新算的（不是沿用进程启动那一刻）"
        );

        // 上一轮的"探活连续失败"也一起清掉：否则重连后立刻就被算成一个窗口的失败。
        watchdog.record_handshake_success(reconnected_at);
        watchdog.record_probe_failure_start(reconnected_at);
        watchdog.begin_connection_attempt(reconnected_at + WINDOW);
        assert_eq!(
            watchdog.state(reconnected_at + WINDOW),
            GatewayState::Waiting
        );
    }
}
