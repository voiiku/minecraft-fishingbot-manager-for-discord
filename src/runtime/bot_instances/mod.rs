//! 管理本机的 FishingBot 实例：查找账号目录、生成游戏配置、查询和控制 screen 会话。
//! 这些操作供用户命令、启动队列和进程收尾共用；不做 Discord 用户鉴权，也不退出管理器。

pub mod bot_dirs;
pub mod bot_sessions;
pub mod bot_status;
pub(crate) mod fishing_pause;
pub mod fishingbot_config;
pub(crate) mod restart;
pub mod screen;
pub mod stop;
