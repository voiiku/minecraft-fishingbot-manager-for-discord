//! `/bot rename`：重命名机器人目录。

use std::fs;

use crate::text;
use crate::utils::{BotDirEntry, ToCleanBotName, is_bot_name_taken, refusal_if_running};

/// 重命名机器人（`/bot rename`）。
///
/// 权限由路由层判过（`OwnerOrAdmin`，见 [`crate::handle_bot_manage`]）。
pub(crate) fn rename_bot(
    bot_dir_entry: &BotDirEntry,
    raw_new_bot_name: &str,
) -> Result<String, String> {
    // 目录名一律用清洗后的名字：旧名是条目里的（路由层解析时清洗过），新名在这里清洗一次。
    let clean_old_bot_name: &str = &bot_dir_entry.bot_name;
    let clean_new_bot_name = raw_new_bot_name.to_clean_bot_name()?;

    if clean_old_bot_name == clean_new_bot_name {
        return Err(text::get().bot_rename_same_name().to_string());
    }

    // 拒绝运行中的机器人
    refusal_if_running(clean_old_bot_name)?;

    // 全服重名校验。
    if is_bot_name_taken(&clean_new_bot_name) {
        return Err(text::get().bot_rename_name_taken(&clean_new_bot_name));
    }

    // 新目录与旧目录同属同一个用户目录，因此只在父目录里换名字。
    let old_bot_dir = &bot_dir_entry.bot_dir_path;
    let owner_dir = old_bot_dir
        .parent()
        .ok_or_else(|| text::get().bot_rename_no_parent_dir().to_string())?;
    let new_bot_dir = owner_dir.join(&clean_new_bot_name);

    fs::rename(old_bot_dir, &new_bot_dir)
        .map_err(|e| text::get().bot_rename_failed(&e.to_string()))?;

    Ok(text::get().bot_rename_succeeded(clean_old_bot_name, &clean_new_bot_name))
}
