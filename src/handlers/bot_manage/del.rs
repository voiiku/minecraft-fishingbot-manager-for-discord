//! `/bot del`：删除机器人及其数据目录。

use crate::text;
use crate::utils::{BotDirEntry, refusal_if_not_stopped, remove_dir_all_if_exists};

/// 删除已停止机器人的数据目录；权限由路由层检查。
pub(crate) fn delete_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 排队中的启动请求也会访问该目录。
    refusal_if_not_stopped(bot_dir_entry)?;

    remove_dir_all_if_exists(&bot_dir_entry.bot_dir_path).map_err(|e| {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::BotDeleteFailed {
                bot_name,
                reason: &e.to_string(),
            })
        );
        text::get().bot_delete_failed(bot_name, &e.to_string())
    })?;

    log::info!(
        "{}",
        text::get().log_text(text::LogText::BotDeleted { bot_name })
    );
    Ok(text::get().bot_delete_succeeded(bot_name))
}
