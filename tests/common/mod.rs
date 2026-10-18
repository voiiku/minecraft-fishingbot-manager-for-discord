#![allow(dead_code)]

//! 集成测试共用的辅助函数。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use minecraft_fishingbot_manager_for_discord::text::{self, Language};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

/// 返回唯一的临时目录路径，并确保目录存在且为空。
pub fn temp_dir(name: &str) -> PathBuf {
    let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "minecraft-fishingbot-manager-{name}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temporary test directory");
    dir
}

/// 安装集成测试使用的固定语言。
///
/// `text::install` 只生效一次；所有需要读取文案的测试都应先调用本函数，
/// 避免依赖测试执行顺序。
pub fn install_english() {
    text::install(Language::English);
}

/// 读取仓库根目录的示例配置。
pub fn example_config() -> &'static str {
    include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml")
}

/// 删除测试目录；清理失败会直接终止测试。
pub fn remove_dir(path: &Path) {
    std::fs::remove_dir_all(path).expect("remove temporary test directory");
}
