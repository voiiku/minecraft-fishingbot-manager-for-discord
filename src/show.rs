//! 只读查询指令：状态、配置与账号列表。

mod admins;
mod allbots;
mod botconfig;
mod mybots;
mod status;

// 本模块对外的查询入口。
pub(crate) use admins::admins_list;
pub(crate) use allbots::all_bots_list;
pub(crate) use botconfig::fishingbot_config_view;
pub(crate) use mybots::my_bots_list;
pub(crate) use status::all_running_bots_status;

/// `/show botconfig` 附件数据，由命令层组装发送。
pub use botconfig::FishingbotConfigView;
