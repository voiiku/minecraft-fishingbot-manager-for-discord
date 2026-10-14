//! 加载、校验并访问配置文件。配置文件位于可执行文件同级目录，只在启动时读取一次。
//! 不存在配置文件时生成模板、配置错误则报错退出，这些都在 [`get_or_init`] 里实现。
//! 内部按结构、解析、校验和模板拆分。

mod parse;
mod schema;
mod template;
mod validate;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::text;

pub use schema::{
    Admins, CommandEchoConfig, Config, DiscordConfig, FishingBotConfig, I18nConfig, KeywordsConfig,
    Paths, PingConfig, RuntimeConfig, StartupQueueConfig, TextConfig, TimingConfig,
};

/// 配置文件名，位于可执行文件的同级目录。
pub const CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";

/// 启动时加载的配置与路径。
struct Settings {
    /// 本次启动使用的配置文件路径。
    path: PathBuf,
    /// 校验并解析后的配置内容。
    config: Config,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

/// 配置的装载结果。
pub enum LoadOutcome {
    /// 配置已装载并通过校验。
    Loaded(&'static Config),
    /// 文件不存在时生成模板并提示，进程应当正常退出（退出码 0）。
    TemplateGenerated,
}

/// 获取配置，没有配置文件就初始化，配置不合法时返回具体的错误字段，由调用方带退出码 1 结束进程。
pub fn get_or_init() -> Result<LoadOutcome, String> {
    if let Some(settings) = SETTINGS.get() {
        return Ok(LoadOutcome::Loaded(&settings.config));
    }

    let path = config_path()?;

    // 首次启动生成模板。
    if template::write_if_missing(&path)? {
        println!(
            "{}",
            text::bootstrap::config_template_created(&path.display().to_string())
        );
        return Ok(LoadOutcome::TemplateGenerated);
    }

    let config_text = std::fs::read_to_string(&path).map_err(|e| {
        crate::text::bootstrap::config_read_failed(&path.display().to_string(), &e.to_string())
    })?;

    let config = parse::parse(&config_text, &path)?;

    // 先安装语言，让后续校验错误使用配置指定的语言；非法语言由 validate 报错。
    text::install(text::Language::from_name(&config.i18n.language).unwrap_or_default());

    validate::validate(&config)?;

    // 并发装载时沿用先写入的配置。
    let _ = SETTINGS.set(Settings { path, config });
    Ok(LoadOutcome::Loaded(get()))
}

// ============================== 取值入口 ==============================

/// 取已装载的配置。
///
/// # Panics
///
/// 进程入口还没有调用 [`get_or_init`] 时 panic，文案是 [`text::bootstrap::CONFIG_NOT_LOADED`]（内部不变量，
/// 固定英文）。
pub fn get() -> &'static Config {
    &SETTINGS
        .get()
        .expect(text::bootstrap::CONFIG_NOT_LOADED)
        .config
}

/// 本次启动使用的配置文件路径（可执行文件同级目录）。
///
/// # Panics
///
/// 同 [`get`]：进程入口还没有调用 [`get_or_init`] 时 panic。
pub fn path() -> &'static Path {
    &SETTINGS
        .get()
        .expect(text::bootstrap::CONFIG_NOT_LOADED)
        .path
}

/// 取管理员名单（`minecraft-fishingbot-manager-for-discord.conf.toml` 的 `[admins]`）。
pub fn admins() -> &'static Admins {
    &get().admins
}

/// 返回可执行文件所在目录，配置与程序日志都位于此处。
pub fn install_dir() -> Result<PathBuf, String> {
    // 定位配置时语言尚未安装，因此使用固定英文的引导文案。
    let exe =
        std::env::current_exe().map_err(|e| text::bootstrap::exe_path_failed(&e.to_string()))?;

    exe.parent()
        .map(PathBuf::from)
        .ok_or_else(|| text::bootstrap::exe_path_no_parent(&exe.display().to_string()))
}

/// 定位配置文件：部署目录下的 `minecraft-fishingbot-manager-for-discord.conf.toml`。
fn config_path() -> Result<PathBuf, String> {
    Ok(install_dir()?.join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 配置文件位置固定为可执行文件同级目录。
    #[test]
    fn config_path_is_next_to_executable() {
        let path = config_path().expect("测试环境下应当能取到可执行文件路径");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(CONFIG_FILE_NAME)
        );
        assert_eq!(
            path.parent(),
            std::env::current_exe().expect("取可执行文件路径").parent()
        );
    }
}
