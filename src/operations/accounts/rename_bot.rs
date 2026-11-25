use crate::operations::permitted_bot_dir_entry;
use crate::{
    runtime::bot_instances::{bot_dirs::is_bot_name_taken, bot_sessions::refusal_if_not_stopped},
    utils::bot_name::ToCleanBotName,
};
use std::fs;

pub fn rename_bot(name: &str, raw_new_bot_name: &str, caller_id: u64) -> Result<String, String> {
    let bot_dir_entry = permitted_bot_dir_entry(name, caller_id)?;
    let clean_old_bot_name: &str = &bot_dir_entry.bot_name;
    let clean_new_bot_name = raw_new_bot_name
        .to_clean_bot_name()
        .map_err(crate::runtime::bot_instances::bot_dirs::bot_name_error)?;

    if clean_old_bot_name == clean_new_bot_name {
        return Err(crate::config::Config::get()
            .text()
            .bot_rename_same_name()
            .to_string());
    }

    refusal_if_not_stopped(&bot_dir_entry)?;

    if is_bot_name_taken(&clean_new_bot_name)? {
        return Err(crate::config::Config::get()
            .text()
            .bot_rename_name_taken(&clean_new_bot_name));
    }

    let old_bot_dir = &bot_dir_entry.bot_dir_path;
    let owner_dir = old_bot_dir.parent().ok_or_else(|| {
        crate::config::Config::get()
            .text()
            .bot_rename_no_parent_dir()
            .to_string()
    })?;
    let new_bot_dir = owner_dir.join(&clean_new_bot_name);

    fs::rename(old_bot_dir, &new_bot_dir).map_err(|e| {
        crate::config::Config::get()
            .text()
            .bot_rename_failed(&e.to_string())
    })?;

    Ok(crate::config::Config::get()
        .text()
        .bot_rename_succeeded(clean_old_bot_name, &clean_new_bot_name))
}
