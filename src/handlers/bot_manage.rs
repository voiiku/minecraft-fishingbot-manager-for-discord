//! 机器人账号管理指令：新增、删除、改名、更新 refreshToken、设备码登录。
//!
//! 机器人账号管理指令。权限由路由层检查。

mod add;
mod del;
mod login;
mod rename;
mod token;

// 本模块对外的业务入口。
pub(crate) use add::add_bot;
pub(crate) use del::delete_bot;
pub use login::BotLoginPrompt;
pub(crate) use login::{await_login, begin_login};
pub(crate) use rename::rename_bot;
pub(crate) use token::update_bot_token_file;
