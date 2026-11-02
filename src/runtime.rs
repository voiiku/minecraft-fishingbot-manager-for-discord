//! 运行时配置、启动队列、网关看门狗与进程状态。

pub(crate) mod bot_state;
pub mod config;
mod process_log;
pub(crate) mod startup;
mod watchdog;

pub use process_log::ProcessLog;
pub use watchdog::{WATCHDOG, Watchdog, WatchdogSlot};
