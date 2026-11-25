use crate::operations::permitted_bot_dir_entry;
use crate::{
    runtime::bot_instances::bot_sessions::refusal_if_not_stopped,
    utils::fs::remove_dir_all_if_exists,
};

pub fn delete_bot(name: &str, caller_id: u64) -> Result<String, String> {
    let bot_dir_entry = permitted_bot_dir_entry(name, caller_id)?;
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 排队中的启动请求也会访问该目录。
    refusal_if_not_stopped(&bot_dir_entry)?;

    remove_dir_all_if_exists(&bot_dir_entry.bot_dir_path).map_err(|e| {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .bot_delete_failed_log_message(bot_name, &e.to_string())
        );
        crate::config::Config::get()
            .text()
            .bot_delete_failed(bot_name, &e.to_string())
    })?;

    log::info!(
        "{}",
        crate::config::Config::get().text().bot_deleted(bot_name)
    );
    Ok(crate::config::Config::get()
        .text()
        .bot_delete_succeeded(bot_name))
}
