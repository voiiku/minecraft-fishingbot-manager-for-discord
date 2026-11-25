//! 单元测试共用的辅助函数，由 lib.rs 仅在测试时引入。

use std::path::PathBuf;

pub const TEST_CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";

/// 返回当前进程的临时测试目录，并先清理旧目录。
pub fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "minecraft-fishingbot-manager-for-discord-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    dir
}
