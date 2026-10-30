//! 加载、校验并访问可执行文件同级的配置文件，配置只在启动时读取一次。

mod parse;
mod schema;
mod template;
mod validate;

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::text;

pub use schema::{
    Admins, AntiAfkConfig, CommandEchoConfig, DiscordConfig, FishingBotConfig, I18nConfig,
    KeywordsConfig, Paths, PingConfig, RuntimeConfig, StartupQueueConfig, TextConfig, TimingConfig,
};
use schema::{LogConfig, RawGameLogsConfig};

/// 配置文件名，位于可执行文件的同级目录。
pub const CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";

/// 解析并校验后的运行时配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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
    /// 判断机器人状态用的日志关键词。
    pub keywords: KeywordsConfig,
    /// `/pause`、`/unpause` 的确认回显与检查参数。
    pub command_echo: CommandEchoConfig,
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

/// 进程内唯一的配置。
static CONFIG: OnceLock<Config> = OnceLock::new();

impl Config {
    /// 加载配置；不存在时生成模板并退出，加载失败时以退出码 1 结束进程。
    pub fn get_or_init() -> &'static Self {
        if let Some(config) = CONFIG.get() {
            return config;
        }

        let path = match Self::install_dir() {
            Ok(dir) => dir.join(CONFIG_FILE_NAME),
            Err(message) => Self::exit_on_error(message),
        };

        match template::create_config_file_when_missing(&path) {
            Ok(true) => {
                println!(
                    "{}",
                    text::bootstrap::config_template_created(&path.display().to_string())
                );
                std::process::exit(0);
            }
            Ok(false) => println!(
                "{}",
                text::bootstrap::config_file_found(&path.display().to_string())
            ),
            Err(message) => Self::exit_on_error(message),
        }
        let config_text = match std::fs::read_to_string(&path) {
            Ok(config_text) => config_text,
            Err(e) => Self::exit_on_error(text::bootstrap::config_read_failed(
                &path.display().to_string(),
                &e.to_string(),
            )),
        };
        let config = match Self::from_toml_text(&config_text) {
            Ok(config) => config,
            Err(message) => Self::exit_on_error(message),
        };

        // 校验先安装语言，后续错误文案才能使用所选语言。
        if let Err(message) = validate::validate(&config) {
            Self::exit_on_error(message);
        }

        // 并发装载时沿用先写入的配置。
        let _ = CONFIG.set(config);
        Self::get()
    }

    /// 装载失败的统一报错出口，以 1 结束进程。
    fn exit_on_error(message: String) -> ! {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

impl Config {
    /// 取已装载的配置。
    /// 当配置还未初始化时 panic，文案是 [`text::bootstrap::CONFIG_NOT_LOADED`]
    pub fn get() -> &'static Config {
        CONFIG.get().expect(text::bootstrap::CONFIG_NOT_LOADED)
    }

    /// 取管理员名单（`minecraft-fishingbot-manager-for-discord.conf.toml` 的 `[admins]`）。
    pub fn admins() -> &'static Admins {
        &Self::get().admins
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

    /// 装载后配置路径固定为可执行文件同级目录下的配置文件。
    #[test]
    fn config_file_path_is_next_to_executable() {
        let config = Config::from_toml_text(include_str!(
            "../minecraft-fishingbot-manager-for-discord.conf.example.toml"
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
}
