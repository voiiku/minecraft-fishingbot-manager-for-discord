//! 运行时配置、机器人会话、进程管理与共享文件操作。
//!
//! 内部工具不作为公开模块暴露：
//! ```compile_fail
//! use minecraft_fishingbot_manager_for_discord::runtime::utils;
//! ```

pub(crate) mod anti_afk;
pub mod bot_instances;
pub mod bot_log;
pub mod bot_startup;
pub mod config;
mod discord_notices;
pub mod log_files;
mod process_log;
pub mod shutdown;
mod utils;
mod watchdog;

pub use process_log::ProcessLog;
pub use watchdog::Watchdog;
