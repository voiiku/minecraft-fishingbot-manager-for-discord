//! 单个机器人的控制指令：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`、`/log`。
//!
//! 单机器人控制指令。会话操作复用 `utils`；权限由路由层检查。
//! `/pause` 与 `/unpause` 还会检查日志回显，逻辑在 [`utils::confirm_pause_command`]。

mod command;
mod log;
mod pause;
mod restart;
mod start;
mod stop;
mod unpause;

// 本模块对外的业务入口。
pub(crate) use command::send_command_to_bot;
pub(crate) use log::bot_log_view;
pub(crate) use pause::pause_fishing;
pub(crate) use restart::restart_bot;
pub(crate) use start::start_bot;
pub(crate) use stop::stop_bot;
pub(crate) use unpause::unpause_fishing;

/// `/log` 附件数据，由命令层组装发送。
pub use log::BotLogView;
