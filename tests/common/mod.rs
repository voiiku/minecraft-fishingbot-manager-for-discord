#![allow(dead_code)]

//! 集成测试共用的辅助函数。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

pub const TEST_CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";

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

/// 在隔离的子进程中加载测试配置。
pub fn in_configured_child(test_name: &str) -> bool {
    const CHILD: &str = "FISHBOT_CONFIGURED_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        minecraft_fishingbot_manager_for_discord::config::Config::load()
            .expect("load test configuration");
        return true;
    }

    let root = temp_dir("configured-child");
    let data = root.join("data");
    std::fs::create_dir_all(&data).expect("create test data directory");
    let mut value: toml::Value = toml::from_str(example_config()).expect("parse example");
    value["paths"]["base_dir"] = toml::Value::String(data.to_string_lossy().into_owned());
    std::fs::write(
        data.join(value["paths"]["jar"].as_str().unwrap()),
        b"test jar",
    )
    .expect("create test jar");
    std::fs::write(
        root.join(TEST_CONFIG_FILE_NAME),
        toml::to_string(&value).expect("serialize test config"),
    )
    .expect("write test config");
    let executable = root.join("configured-test");
    std::fs::copy(std::env::current_exe().unwrap(), &executable).expect("copy test executable");
    let output = std::process::Command::new(executable)
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .expect("run configured test");
    remove_dir(&root);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

/// 读取仓库根目录的示例配置。
pub fn example_config() -> &'static str {
    include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml")
}

/// 删除测试目录；清理失败会直接终止测试。
pub fn remove_dir(path: &Path) {
    std::fs::remove_dir_all(path).expect("remove temporary test directory");
}
