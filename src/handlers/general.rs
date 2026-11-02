//! 通用指令：帮助、用户 ID、服务器探测与主机状态。

mod checkserver;
mod help;
mod ping;
mod whoami;

// 本模块对外的指令入口。
pub(crate) use checkserver::checkserver;
pub(crate) use help::help;
pub(crate) use ping::ping;
pub(crate) use whoami::whoami;
