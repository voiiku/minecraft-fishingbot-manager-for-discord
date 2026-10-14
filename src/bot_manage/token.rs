//! `/token`：写入机器人的 refreshToken 文件。
//!
//! Token 由命令层通过 Modal 收集并原样传入。

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use crate::text;
use crate::utils::{BotDirEntry, refusal_if_running, token_file_path};

/// 写入 refreshToken；运行中的机器人须先停止。权限由路由层检查。
pub(crate) fn update_bot_token_file(
    bot_dir_entry: &BotDirEntry,
    raw_token: String,
) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    refusal_if_running(bot_name)?;

    let token_file_path = token_file_path(&bot_dir_entry.bot_dir_path);

    write_token_file(&token_file_path, &raw_token)
        .map_err(|e| text::get().bot_token_write_failed(bot_name, &e.to_string()))?;

    let message = text::get().bot_token_updated(bot_name);
    log::info!("{message}");
    Ok(message)
}

/// 以 `0600` 权限创建或覆盖 refreshToken 文件。
fn write_token_file(path: &Path, raw_token: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        // 不在 open 时截断：先把既有文件权限收紧为 0600，再显式清空内容。
        // 这样覆盖权限过宽的旧文件时，不会在 chmod 前先截断它。
        .truncate(false)
        .mode(0o600)
        .open(path)?;

    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.set_len(0)?;
    file.write_all(raw_token.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 新建与覆盖的文件均限制为所有者可读写。
    #[test]
    fn token_file_is_private_when_created_and_overwritten() {
        let dir = crate::test_support::temp_test_dir("manual-token-permissions");
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        let path = dir.join("refreshToken");

        write_token_file(&path, "first token").expect("创建 Token 文件失败");
        let created_mode = fs::metadata(&path)
            .expect("读取新 Token 文件元数据失败")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(created_mode, 0o600);

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("放宽测试文件权限失败");
        write_token_file(&path, "new refresh token\n").expect("安全写入 Token 失败");

        let permissions = fs::metadata(&path)
            .expect("读取 Token 文件元数据失败")
            .permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);
        assert_eq!(
            fs::read_to_string(&path).expect("读取 Token 文件失败"),
            "new refresh token\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
