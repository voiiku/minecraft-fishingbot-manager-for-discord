//! 加载、校验并访问可执行文件同级的配置文件，配置只在启动时读取一次。

mod constants;
mod parse;
mod schema;
mod template;
mod validate;

use std::path::PathBuf;

use serde::Deserialize;

use crate::{CONFIG, text};

pub use constants::Constants;
pub use schema::{
    Admins, AntiAfkConfig, DiscordConfig, FishingBotConfig, I18nConfig, KeywordsConfig, Paths,
    PingConfig, RuntimeConfig, StartupQueueConfig, TextConfig, TimingConfig,
};
use schema::{LogConfig, RawGameLogsConfig};

use constants::CONFIG_FILE_NAME;

/// 解析并校验后的运行时配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// 不由 TOML 提供、随文件配置一起加载的固定运行参数。
    #[serde(skip, default = "constants::defaults")]
    pub constants: Constants,
    /// 本次启动使用的配置文件的路径，装载时按可执行文件位置推算。
    #[serde(skip, default = "default_config_file_path")]
    pub config_file_path: PathBuf,
    /// Discord bot 信息。
    pub discord: DiscordConfig,
    /// 磁盘路径，文件名相对于 `base_dir` 展开。
    pub paths: Paths,
    /// 游戏侧配置中由本项目管理的键，启动时据此生成配置。
    pub fishingbot: FishingBotConfig,
    /// 管理员名单。
    pub admins: Admins,
    /// 机器人日志过滤配置。
    pub keywords: KeywordsConfig,
    /// 程序运行使用的语言（面向 Discord 的文案）。
    pub i18n: I18nConfig,
    /// 机器人运行时参数。
    pub runtime: RuntimeConfig,
    /// 反 AFK 的输入和间隔参数。
    pub anti_afk: AntiAfkConfig,
    /// 启动队列的批次与冷却策略。
    pub startup_queue: StartupQueueConfig,
    /// 单批内启动错峰、会话复核、停止与看门狗时序参数。
    pub timing: TimingConfig,
    /// FishingBot 原始日志配置。
    pub raw_game_logs: RawGameLogsConfig,
    /// `/ping` 默认目标。
    pub ping: PingConfig,
    /// 程序日志的级别开关。
    pub log: LogConfig,
    /// 可自定义的提示文案。
    pub text: TextConfig,
}

/// `config_file_path` 的值，按可执行文件位置推导（取不到就留空，装载配置时会以错误退出）。
fn default_config_file_path() -> PathBuf {
    Config::install_dir().map_or_else(|_| PathBuf::new(), |dir| dir.join(CONFIG_FILE_NAME))
}

#[cfg(test)]
pub(crate) fn init_for_test() {
    CONFIG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../../minecraft-fishingbot-manager-for-discord.conf.example.toml"
        ))
        .expect("test configuration must parse")
    });
}

impl Config {
    /// 加载配置；不存在时生成模板并退出，加载失败时返回错误。
    pub fn load() -> Result<(), String> {
        if CONFIG.get().is_some() {
            return Ok(());
        }

        let path = Self::install_dir()?.join(CONFIG_FILE_NAME);

        if template::create_config_file_when_missing(&path)? {
            println!(
                "{}",
                text::bootstrap::config_template_created(&path.display().to_string())
            );
            std::process::exit(0);
        }
        println!(
            "{}",
            text::bootstrap::config_file_found(&path.display().to_string())
        );
        let config_text = std::fs::read_to_string(&path).map_err(|error| {
            text::bootstrap::config_read_failed(&path.display().to_string(), &error.to_string())
        })?;
        let config = Self::from_toml_text(&config_text)?;

        validate::validate(&config)?;

        // 并发装载时沿用先写入的配置。
        let _ = CONFIG.set(config);
        Ok(())
    }

    /// 取已装载的配置。
    /// 当配置还未初始化时 panic，文案是 [`text::bootstrap::CONFIG_NOT_LOADED`]
    pub fn get() -> &'static Config {
        CONFIG.get().expect(text::bootstrap::CONFIG_NOT_LOADED)
    }

    /// 根据已验证的配置选择文案。
    pub fn text(&self) -> &'static dyn text::Text {
        self.i18n.language.implementation()
    }

    pub(crate) fn is_loaded() -> bool {
        CONFIG.get().is_some()
    }

    /// 返回可执行文件所在目录，配置文件与程序日志的文件夹都位于此处。
    pub fn install_dir() -> Result<PathBuf, String> {
        let exe = std::env::current_exe()
            .map_err(|e| text::bootstrap::exe_path_failed(&e.to_string()))?;

        exe.parent()
            .map(PathBuf::from)
            .ok_or_else(|| text::bootstrap::exe_path_no_parent(&exe.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    /// 装载后配置路径固定为可执行文件同级目录下的配置文件。
    #[test]
    fn config_file_path_is_next_to_executable() {
        let config = Config::from_toml_text(include_str!(
            "../../../minecraft-fishingbot-manager-for-discord.conf.example.toml"
        ))
        .expect("示例配置应当能解析");

        assert_eq!(
            config
                .config_file_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some(CONFIG_FILE_NAME)
        );
        assert_eq!(
            config.config_file_path.parent(),
            std::env::current_exe().expect("取可执行文件路径").parent()
        );
    }

    #[test]
    fn display_time_includes_timezone_and_utc_offset() {
        let config = Config::from_toml_text(include_str!(
            "../../../minecraft-fishingbot-manager-for-discord.conf.example.toml"
        ))
        .unwrap();
        let time = FixedOffset::east_opt(8 * 60 * 60)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 25, 22, 16, 16)
            .unwrap();
        assert_eq!(
            time.format(config.constants.display_timestamp_format)
                .to_string(),
            "2026-11-25 22:16:16 +08:00 (UTC+08:00)"
        );
    }
}
