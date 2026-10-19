//! 首次启动时生成配置模板，内容来自仓库根目录的示例配置。

use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::text;

/// 配置模板（即仓库根的示例配置）。
pub(super) const TEMPLATE: &str =
    include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");

/// 配置不存在时以 0600 权限创建并写入模板；已存在时不覆盖。返回是否新建。
pub(super) fn create_config_file_when_missing(path: &Path) -> Result<bool, String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty() && !parent.is_dir())
    {
        return Err(text::bootstrap::config_template_no_parent_dir(
            &parent.display().to_string(),
        ));
    }

    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => {
            return Err(text::bootstrap::config_template_write_failed(
                &path.display().to_string(),
                &error.to_string(),
            ));
        }
    };

    file.write_all(TEMPLATE.as_bytes()).map_err(|e| {
        text::bootstrap::config_template_write_failed(&path.display().to_string(), &e.to_string())
    })?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_test_dir;
    use std::path::PathBuf;

    /// 返回测试用的配置文件路径。
    fn temp_config_path(tag: &str) -> PathBuf {
        let dir = temp_test_dir(&format!("template-{tag}"));
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");

        dir.join(crate::config::CONFIG_FILE_NAME)
    }

    /// 文件不存在时生成模板。
    #[test]
    fn writes_template_when_missing() {
        let path = temp_config_path("missing");

        let created = create_config_file_when_missing(&path).expect("生成模板失败");
        let written = std::fs::read_to_string(&path).expect("读取生成的模板失败");

        assert!(created, "文件不存在时应当新建");
        assert_eq!(written, TEMPLATE, "生成的内容应当与示例配置完全一致");
    }

    /// 新建的模板必须仅所有者可读写。
    #[test]
    fn generated_template_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_config_path("private");
        assert!(create_config_file_when_missing(&path).expect("生成模板失败"));

        let mode = std::fs::metadata(&path)
            .expect("读取模板元数据失败")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    /// 不覆盖已有配置。
    #[test]
    fn keeps_existing_file() {
        let path = temp_config_path("existing");
        std::fs::write(&path, "使用者自己写的配置").expect("写入测试文件失败");

        let created = create_config_file_when_missing(&path).expect("已存在的文件不应报错");
        let content = std::fs::read_to_string(&path).expect("读取失败");

        assert!(!created, "文件已存在时不应重建");
        assert_eq!(content, "使用者自己写的配置", "不应覆盖使用者的配置");
    }

    /// 父目录不存在时返回错误。
    #[test]
    fn missing_parent_dir_is_error() {
        // 临时目录已清理，父目录不存在。
        let path = temp_test_dir("template-absent").join(crate::config::CONFIG_FILE_NAME);

        let err = create_config_file_when_missing(&path).expect_err("目录不存在时应当报错");

        // 引导期文案固定为英文。
        assert!(
            err.contains("does not exist"),
            "报错应说明目录不存在: {err}"
        );
    }
}
