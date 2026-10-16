//! `/bot del`：删除机器人及其数据目录。

use crate::text;
use crate::utils::{BotDirEntry, refusal_if_not_stopped, remove_dir_all_if_exists};

/// 删除机器人及其数据目录（`/bot del`）。
///
/// 权限由路由层判过（`OwnerOrAdmin`，见 [`crate::handle_bot_manage`]）；名字由路由层解析条目时
/// 清洗过一次，回复与后台日志都用条目里的干净名字。
pub(crate) fn delete_bot(bot_dir_entry: &BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 运行中的机器人必须先停止，否则会删除其正在写入的数据目录；排队待启动的同样如此：队列会将其拉起。
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
