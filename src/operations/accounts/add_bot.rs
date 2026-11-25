use crate::runtime::bot_instances::bot_dirs::create_bot_dir;

/// 添加机器人并创建数据目录（`/bot add`）。
pub fn add_bot(raw_bot_name: &str, caller_id: u64) -> Result<String, String> {
    match create_bot_dir(raw_bot_name, caller_id) {
        // 回复与后台日志都用磁盘上的真实目录名（清洗后的名字）。
        Ok(bot_dir_entry) => {
            log::info!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .bot_added(&bot_dir_entry.bot_name)
            );
            Ok(crate::config::Config::get()
                .text()
                .bot_add_succeeded(&bot_dir_entry.bot_name))
        }
        Err(message) => {
            log::warn!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .bot_add_failed_log_message(raw_bot_name, &message)
            );
            Err(crate::config::Config::get()
                .text()
                .bot_add_failed(raw_bot_name, &message))
        }
    }
}
