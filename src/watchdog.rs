//! 看门狗：判定本进程与 Discord 的连接状态，并负责掉线后的进程内处理。
//! - 本文件：Watchdog的构造方法
//! - `gateway`：为Watchdog实现网络判定与 REST 探活任务
//! - `lifecycle`：根据Watchdog状态实现掉线记录、重连信号与进程收尾相关具体处理逻辑。

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

mod gateway;
mod lifecycle;

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
    /// 掉线后唤醒进程入口的信号（进程入口会用select!异步等待，谁先到就处理谁）。
    reconnect_signal: tokio::sync::Notify,
    /// 本次掉线的原因与时刻。
    offline_since: Mutex<Option<(String, std::time::SystemTime)>>,
    /// 判定侧交给进程入口的掉线原因：入口收尾时取走（见 [`Watchdog::request_reconnect`]）。
    pending_disconnect_reason: Mutex<Option<String>>,
}

/// 进程内唯一的那一份看门狗：谁要谁调 `WATCHDOG.get_or_init()`（首次调用按配置建）。
pub static WATCHDOG: WatchdogSlot = WatchdogSlot::new();

/// 进程级单例的槽位：`get_or_init()` 取同一份（首次调用按配置建）。
///
/// 事件回调只收函数指针、捕获不了闭包，所以它们只能走这个入口，没法把引用传进闭包。
#[derive(Debug, Default)]
pub struct WatchdogSlot(OnceLock<Watchdog>);

impl WatchdogSlot {
    /// 空槽位（给 `static` 用）。
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// 取进程内唯一的那一份看门狗：首次调用按配置 `[timing]` 建，之后拿到的都是它。
    ///
    /// 只在 `static` 上调用：要交出 `'static` 的那一份，槽位自己得活满整个进程。
    /// 另外要在 `config::get_or_init()` 之后调：首次调用会读配置。
    pub fn get_or_init(&'static self) -> &'static Watchdog {
        self.0.get_or_init(|| Watchdog::from_config(Instant::now()))
    }
}

impl Watchdog {
    /// 按配置中的 `[timing]` 创建看门狗。
    /// 探活间隔与首次握手超时由判定窗口派生，见 [`crate::config::TimingConfig`]。
    fn from_config(started: Instant) -> Self {
        let timing = &crate::config::get().timing;
        Self::new(
            started,
            timing.watchdog_window(),
            timing.probe_interval(),
            timing.first_handshake_timeout(),
        )
    }

    /// 按显式时序建一个看门狗（不读配置，所以暂时仅供测试用）。
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
