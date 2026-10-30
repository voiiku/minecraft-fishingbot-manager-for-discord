//! 看门狗的网络状态判定与 REST 探活。`GatewayState` 仅在本模块使用；
//! 外部通过 `record_*` 方法提交事件，并等待重连信号。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use poise::serenity_prelude as serenity;

use crate::text;

use super::{Watchdog, lock_or_panic};

/// 四种网络状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayState {
    /// 已握手，且探活未连续失败。
    Alive,
    /// 尚未满足任何掉线条件，等待后续事件。
    Waiting,
    /// 本次连接尝试开始后未收到 READY / RESUMED。
    NeverReady,
    /// REST 探活连续失败达到判定窗口。
    Unreachable,
}

impl GatewayState {
    /// 掉线原因对应的日志文案；未掉线时返回 `None`。
    fn lost_log_text(self) -> Option<crate::text::LogText<'static>> {
        match self {
            Self::NeverReady => Some(crate::text::LogText::GatewayLostNeverReady),
            Self::Unreachable => Some(crate::text::LogText::GatewayLostUnreachable),
            Self::Alive | Self::Waiting => None,
        }
    }
}

/// 网络状态判定与探活任务的实现。
impl Watchdog {
    /// 开始新的连接尝试，并清除上一次尝试的状态。
    pub fn begin_connection_attempt(&self, now: Instant) {
        *lock_or_panic(self.started.lock(), text::WatchdogText::StartedLockReset) = now;
        *lock_or_panic(
            self.last_handshake.lock(),
            text::WatchdogText::LastHandshakeLockClear,
        ) = None;
        *lock_or_panic(
            self.unhealthy_since.lock(),
            text::WatchdogText::UnhealthySinceLockClear,
        ) = None;
    }

    /// 记录握手成功时刻，并清除探活失败起始时刻。
    pub fn record_handshake_success(&self, now: Instant) {
        *lock_or_panic(
            self.last_handshake.lock(),
            text::WatchdogText::LastHandshakeLockRecord,
        ) = Some(now);
        *lock_or_panic(
            self.unhealthy_since.lock(),
            text::WatchdogText::UnhealthySinceLockClear,
        ) = None;
    }

    /// 标记已经握手，并返回这是否是进程内第一次调用。
    pub fn is_first_handshake(&self) -> bool {
        !self.ever_handshaken.swap(true, Ordering::Relaxed)
    }

    /// 清除探活失败起始时刻，使状态重新满足 `Alive` 条件。
    pub fn clear_probe_failures(&self) {
        *lock_or_panic(
            self.unhealthy_since.lock(),
            text::WatchdogText::UnhealthySinceLockClear,
        ) = None;
    }

    /// 记录首次探活失败时刻；重复调用不会更新该时刻。
    pub fn record_probe_failure_start(&self, now: Instant) {
        lock_or_panic(
            self.unhealthy_since.lock(),
            text::WatchdogText::UnhealthySinceLockRecord,
        )
        .get_or_insert(now);
    }

    /// 根据当前时刻与看门狗字段判定网络状态。
    fn state(&self, now: Instant) -> GatewayState {
        let handshaken = lock_or_panic(
            self.last_handshake.lock(),
            text::WatchdogText::LastHandshakeLockRead,
        )
        .is_some();
        if !handshaken {
            let timeout = *lock_or_panic(self.started.lock(), text::WatchdogText::StartedLockRead)
                + self.first_handshake_timeout;
            return if now >= timeout {
                GatewayState::NeverReady
            } else {
                GatewayState::Waiting
            };
        }
        let unhealthy_since = *lock_or_panic(
            self.unhealthy_since.lock(),
            text::WatchdogText::UnhealthySinceLockRead,
        );
        match unhealthy_since {
            Some(unhealthy_since) if now.duration_since(unhealthy_since) >= self.window => {
                GatewayState::Unreachable
            }
            _ => GatewayState::Alive,
        }
    }

    /// 启动 REST 探活任务：按 [`Watchdog::probe_interval`] 周期请求当前用户信息；
    /// 判定掉线时请求重建连接。`stop` 用于终止本轮任务。
    pub fn probe(&'static self, http: Arc<serenity::Http>, stop: Arc<AtomicBool>) {
        tokio::spawn(async move {
            let interval = self.probe_interval;

            loop {
                tokio::time::sleep(interval).await;
                if stop.load(Ordering::Relaxed) {
                    return;
                }

                let probed = http.get_current_user().await;
                // 请求期间收到停止标志时丢弃结果。
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                // 请求成功视为网络可用；失败时记录原因并开始计时。
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
                // 判定掉线时请求重建连接并结束任务；主循环会创建下一轮探活任务。
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

    /// 测试用超时参数；具体数值仅用于计算相对时刻。
    const WINDOW: Duration = Duration::from_secs(30);
    const PROBE_INTERVAL: Duration = Duration::from_secs(10);
    const FIRST_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

    /// 创建以当前时刻为起始时刻的看门狗，测试使用相对时刻断言。
    fn watchdog_at_start() -> (Watchdog, Instant) {
        let started = Instant::now();
        (
            Watchdog::new(started, WINDOW, PROBE_INTERVAL, FIRST_HANDSHAKE_TIMEOUT),
            started,
        )
    }

    /// 首次握手超时前为 Waiting，达到超时时刻后为 NeverReady。
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

    /// 就绪提示每进程只输出一次：`is_first_handshake` 仅在首次返回 `true`。
    #[test]
    fn only_the_first_handshake_reports_ready() {
        let (watchdog, started) = watchdog_at_start();

        assert!(watchdog.is_first_handshake());
        assert!(!watchdog.is_first_handshake());

        // 重连只清除本轮状态；“是否已输出就绪日志”属于进程级状态，必须保留。
        watchdog.begin_connection_attempt(started);
        assert!(!watchdog.is_first_handshake());
    }

    /// 握手成功且探活正常时状态为 Alive。
    #[test]
    fn handshake_and_probe_are_alive() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.clear_probe_failures();

        assert_eq!(watchdog.state(started), GatewayState::Alive);
        // 探活持续成功时，时间推进不影响状态。
        watchdog.clear_probe_failures();
        assert_eq!(watchdog.state(started + WINDOW * 10), GatewayState::Alive);
    }

    /// 探活连续失败时，先保持 Alive，达到判定窗口后转为 Unreachable。
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

    /// 失败起始时刻只记录第一次；重复失败不会延后判定截止时间。
    #[test]
    fn repeated_failures_do_not_push_the_deadline() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);

        // 模拟连续失败：重复记录不改变起始时刻。
        for _ in 0..5 {
            watchdog.record_probe_failure_start(started);
        }

        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Unreachable);
    }

    /// 清除探活失败状态后，状态立即恢复为 Alive。
    #[test]
    fn recovered_probe_revives_the_watchdog() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);
        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Unreachable);

        watchdog.clear_probe_failures();

        assert_eq!(watchdog.state(started + WINDOW), GatewayState::Alive);
    }

    /// 重新握手成功后，状态恢复为健康并清除探活失败状态。
    #[test]
    fn resumed_handshake_revives_the_watchdog() {
        let (watchdog, started) = watchdog_at_start();
        watchdog.record_handshake_success(started);
        watchdog.record_probe_failure_start(started);

        let resumed_at = started + WINDOW;
        watchdog.record_handshake_success(resumed_at);

        assert_eq!(watchdog.state(resumed_at + WINDOW), GatewayState::Alive);
    }

    /// 新连接尝试从当前时刻重新计算首次握手超时，并清除上一次尝试的状态。
    /// 若不重置，重连后会沿用进程启动时刻并立即判定为 `NeverReady`。
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
            "重连后仍处于首次握手超时窗口内"
        );
        assert_eq!(
            watchdog.state(reconnected_at + FIRST_HANDSHAKE_TIMEOUT),
            GatewayState::NeverReady,
            "超时从重连时刻重新计算，而非沿用进程启动时刻"
        );

        // 清除上一轮的探活连续失败状态；否则重连后会立即被判定为一个窗口的失败。
        watchdog.record_handshake_success(reconnected_at);
        watchdog.record_probe_failure_start(reconnected_at);
        watchdog.begin_connection_attempt(reconnected_at + WINDOW);
        assert_eq!(
            watchdog.state(reconnected_at + WINDOW),
            GatewayState::Waiting
        );
    }
}
