//! 看门狗：判定本进程与 Discord 的连接状态，并负责掉线后的进程内处理。
//! - 本文件：`Watchdog` 的构造方法。
//! - `gateway`：为 `Watchdog` 实现网络状态判定与 REST 探活任务。
//! - `lifecycle`：按 `Watchdog` 的状态实现掉线记录、重连信号与进程收尾。

use std::sync::atomic::AtomicBool;
use std::sync::{LockResult, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use poise::serenity_prelude as serenity;

use crate::text;

mod gateway;
mod lifecycle;

/// 取看门狗内部互斥锁；中毒时按当前语言给出内部不变量说明并终止。
///
/// 锁操作在多个模块重复出现，统一收在这里，避免各处分别拼 `.expect` 文案。
fn lock_or_panic<'a, T>(
    lock_result: LockResult<MutexGuard<'a, T>>,
    message: text::WatchdogText,
) -> MutexGuard<'a, T> {
    lock_result.unwrap_or_else(|_| panic!("{}", text::get().watchdog_text(message)))
}

/// 看门狗：维护网络状态和进程生命周期标识信号。
#[derive(Debug)]
pub struct Watchdog {
    /// 判定窗口：探活连续失败累计满它即判连接失败；掉线后也等待一个它再重建连接。
    window: Duration,
    /// 探活间隔：每隔它用 REST 探活一次。
    probe_interval: Duration,
    /// 首次握手超时：本次尝试开始后超过它仍未握手，就判为从未握手。
    first_handshake_timeout: Duration,
    /// 本次连接尝试的起始时刻。
    started: Mutex<Instant>,
    /// 最后一次成功握手的时刻；`None` 表示本次尝试尚未握手。
    last_handshake: Mutex<Option<Instant>>,
    /// 第一次探活失败的起始时刻；当探活成功时清除。
    unhealthy_since: Mutex<Option<Instant>>,
    /// 进程内是否曾握手，用于只记录一次就绪日志。
    ever_handshaken: AtomicBool,
    /// 掉线后唤醒进程入口的信号。
    reconnect_signal: tokio::sync::Notify,
    /// 本次掉线的原因与时刻。
    offline_since: Mutex<Option<(String, std::time::SystemTime)>>,
    /// 判定侧交给进程入口的掉线原因：入口收尾时取走（见 [`Watchdog::request_reconnect`]）。
    pending_disconnect_reason: Mutex<Option<String>>,
    /// 最近一次握手时拿到的 Discord 上下文：退出信号收尾靠它把报告发到各服务器。
    notice_ctx: Mutex<Option<serenity::Context>>,
}

/// 进程内唯一的看门狗：使用方调用 `WATCHDOG.get_or_init()` 获取（首次调用按配置创建）。
pub static WATCHDOG: WatchdogSlot = WatchdogSlot::new();

/// 进程级单例的槽位：`get_or_init()` 始终取到同一份（首次调用按配置创建）。
/// 事件回调只收函数指针、捕获不了闭包，所以只能走这个入口。
#[derive(Debug, Default)]
pub struct WatchdogSlot(OnceLock<Watchdog>);

impl WatchdogSlot {
    /// 空槽位（给 `static` 用）。
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// 取进程内唯一的看门狗：首次调用按配置 `[timing]` 创建，之后拿到的一直是它。
    /// 只在 `static` 上调用（要交出 `'static` 的那一份），并在 `config::Config::get_or_init()` 之后调。
    pub fn get_or_init(&'static self) -> &'static Watchdog {
        self.0.get_or_init(|| Watchdog::from_config(Instant::now()))
    }
}

impl Watchdog {
    /// 按配置中的 `[timing]` 创建看门狗。
    /// 探活间隔与首次握手超时由判定窗口派生，见 [`crate::config::TimingConfig`]。
    fn from_config(started: Instant) -> Self {
        let timing = &crate::config::Config::get().timing;
        Self::new(
            started,
            timing.watchdog_window(),
            timing.probe_interval(),
            timing.first_handshake_timeout(),
        )
    }

    /// 按显式时序创建看门狗（不读配置，因此仅供测试使用）。
    pub fn new(
        started: Instant,
        window: Duration,
        probe_interval: Duration,
        first_handshake_timeout: Duration,
    ) -> Self {
        Self {
            window,
            probe_interval,
            first_handshake_timeout,
            started: Mutex::new(started),
            last_handshake: Mutex::new(None),
            unhealthy_since: Mutex::new(None),
            ever_handshaken: AtomicBool::new(false),
            reconnect_signal: tokio::sync::Notify::new(),
            offline_since: Mutex::new(None),
            pending_disconnect_reason: Mutex::new(None),
            notice_ctx: Mutex::new(None),
        }
    }

    /// 判定窗口。
    pub fn window(&self) -> Duration {
        self.window
    }

    /// 探活间隔。
    pub fn probe_interval(&self) -> Duration {
        self.probe_interval
    }

    /// 首次握手超时。
    pub fn first_handshake_timeout(&self) -> Duration {
        self.first_handshake_timeout
    }
}
