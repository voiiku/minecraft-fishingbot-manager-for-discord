//! 按指令类别组织的业务入口，不依赖 Discord 上下文。
pub mod accounts;
pub mod all_bots_control;
mod prepared_control;
pub mod queries;
mod set_anti_afk;
pub mod single_bot_control;

pub(crate) use prepared_control::PreparedControl;
pub use set_anti_afk::set_anti_afk;

use crate::config::Config;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;

fn require_admin(caller_id: u64) -> Result<(), String> {
    let config = Config::get();
    if config.admins.is_admin(caller_id) {
        Ok(())
    } else {
        Err(config.text().admin_required(caller_id))
    }
}

/// 不存在与无权访问使用同一回复，不泄露其他用户的机器人名称。
fn permitted_bot_dir_entry(name: &str, caller_id: u64) -> Result<BotDirEntry, String> {
    let bot = BotDirEntry::find_by_name(name)?;
    let config = Config::get();
    if caller_id == bot.owner_id || config.admins.is_admin(caller_id) {
        Ok(bot)
    } else {
        Err(config.text().bot_dir_not_found(&bot.bot_name))
    }
}

/// 同步操作的许可仅在本次调用持锁期间有效，Discord 适配器只执行网络部分。
pub fn begin_command_sync(caller_id: u64) -> Result<tokio::sync::MutexGuard<'static, ()>, String> {
    require_admin(caller_id)?;
    crate::COMMAND_SYNC_LOCK
        .try_lock()
        .map_err(|_| Config::get().text().command_sync_in_progress().to_string())
}
