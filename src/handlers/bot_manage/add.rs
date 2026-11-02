//! `/bot add`：添加机器人并创建数据目录。

use crate::text;
use crate::utils::create_bot_dir;

/// 添加机器人并创建数据目录（`/bot add`）。
pub(crate) fn add_bot(raw_bot_name: &str, caller_id: u64) -> Result<String, String> {
    match create_bot_dir(raw_bot_name, caller_id) {
        // 回复与后台日志都用磁盘上的真实目录名（清洗后的名字）。
        Ok(bot_dir_entry) => {
            log::info!(
                "{}",
                text::get().log_text(text::LogText::BotAdded {
                    bot_name: &bot_dir_entry.bot_name,
                })
            );
            Ok(text::get().bot_add_succeeded(&bot_dir_entry.bot_name))
        }
        Err(message) => {
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::BotAddFailed {
                    raw_bot_name,
                    reason: &message,
                })
            );
            Err(text::get().bot_add_failed(raw_bot_name, &message))
        }
    }
}
