//! 看门狗：判定本进程与 Discord 的连接状态，并处理掉线后的进程内生命周期。
//! - 本文件：`Watchdog` 的构造与单例入口。
//! - `gateway`：网络状态判定与 REST 探活。
//! - `lifecycle`：掉线记录、重连信号与进程退出。

use std::sync::atomic::AtomicBool;
use std::sync::{LockResult, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use poise::serenity_prelude as serenity;

use crate::text;

mod gateway;
mod lifecycle;

/// 获取看门狗内部互斥锁；锁中毒时按当前语言输出诊断信息并终止进程。
///
/// 所有看门狗锁操作统一经此函数，避免在调用点重复维护诊断文案。
fn lock_or_panic<'a, T>(
    lock_result: LockResult<MutexGuard<'a, T>>,
    message: text::WatchdogText,
) -> MutexGuard<'a, T> {
    lock_result.unwrap_or_else(|_| panic!("{}", text::get().watchdog_text(message)))
}

/// 看门狗：维护连接状态、掉线记录、重连信号与退出通知上下文。
#[derive(Debug)]
pub struct Watchdog {
    /// 判定窗口：探活连续失败累计达到该时长即判定连接失败；掉线后也等待该时长再重建连接。
    window: Duration,
    /// REST 探活间隔。
    probe_interval: Duration,
    /// 首次握手超时：连接尝试开始后超过该时长仍未握手，判定为从未就绪。
    first_handshake_timeout: Duration,
    /// 本次连接尝试的起始时刻。
    started: Mutex<Instant>,
    /// 最后一次成功握手的时刻；`None` 表示本次尝试尚未握手。
    last_handshake: Mutex<Option<Instant>>,
    /// 第一次探活失败的起始时刻；探活成功后清除。
    unhealthy_since: Mutex<Option<Instant>>,
    /// 进程内是否曾成功握手，用于只输出一次就绪日志。
    ever_handshaken: AtomicBool,
    /// 掉线后唤醒进程入口的信号。
    reconnect_signal: tokio::sync::Notify,
    /// 本次掉线的原因与时刻。
    offline_since: Mutex<Option<(String, std::time::SystemTime)>>,
    /// 判定任务提交给进程入口的掉线原因；入口处理重连时读取并清空。
    pending_disconnect_reason: Mutex<Option<String>>,
    /// 最近一次成功握手时记录的 Discord 上下文；退出时用于发送报告。
    notice_ctx: Mutex<Option<serenity::Context>>,
}

/// 进程内唯一的看门狗。使用方通过 [`WatchdogSlot::get_or_init`] 获取；
/// 首次调用按配置创建。
pub static WATCHDOG: WatchdogSlot = WatchdogSlot::new();

/// 进程级单例槽位：事件回调只接收函数指针，无法捕获闭包，因此通过该静态入口访问。
#[derive(Debug, Default)]
pub struct WatchdogSlot(OnceLock<Watchdog>);

impl WatchdogSlot {
    /// 创建空槽位，供 `static` 初始化使用。
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// 返回进程内唯一的看门狗。首次调用按配置 `[timing]` 创建；
    /// 必须在 `config::Config::get_or_init()` 之后调用。
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
