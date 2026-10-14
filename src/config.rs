//! 加载、校验并访问配置文件。配置文件位于可执行文件同级目录，只在启动时读取一次。
//! 运行时结构 [`Config`] 与它的取值接口都在本文件（它同时是 TOML 文件的映射目标）
//! 配置文件中各个表对应的结构体在 `config/schema.rs`中定义
//! 解析逻辑全部在parse.rs中各自实现在表结构体上
//! 校验逻辑全部在validate.rs中各自实现在表结构体上
//! 生成模板的逻辑定义在template.rs中。

mod parse;
mod schema;
mod template;
mod validate;

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::text;

pub use schema::{
    Admins, CommandEchoConfig, DiscordConfig, FishingBotConfig, I18nConfig, KeywordsConfig, Paths,
    PingConfig, RuntimeConfig, StartupQueueConfig, TextConfig, TimingConfig,
};
use schema::{LogConfig, RawGameLogsConfig};

/// 配置文件名，位于可执行文件的同级目录。
pub const CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";

/// 解析并校验后的配置，字段顺序与示例配置一致（`config_file_path` 除外，他不在配置中，由初始化时算出来）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// 本次启动使用的配置文件路径：部署目录下，装载时按可执行文件位置算出来。
    #[serde(skip, default = "default_config_file_path")]
    pub config_file_path: PathBuf,
    /// Discord bot 的连接与身份信息（唯一凭据，必填）。
    pub discord: DiscordConfig,
    /// 磁盘路径，相对路径已按配置文件所在目录展开。
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

/// `config_file_path` 的默认值：按可执行文件位置现算（取不到就留空，装载期会以同一个错误退出）。
fn default_config_file_path() -> PathBuf {
    Config::install_dir().map_or_else(|_| PathBuf::new(), |dir| dir.join(CONFIG_FILE_NAME))
}

/// 进程内唯一的配置。
static CONFIG: OnceLock<Config> = OnceLock::new();

impl Config {
    /// 获取配置
    /// 没有配置文件就初始化，首次启动会生成模板并打印提示、然后正常退出（退出码 0）
    /// 发送错误返回提示并以退出码 1 结束进程。
    pub fn get_or_init() -> &'static Self {
        if let Some(config) = CONFIG.get() {
            return config;
        }

        let path = match Self::install_dir() {
            Ok(dir) => dir.join(CONFIG_FILE_NAME),
            Err(message) => Self::exit_on_error(message),
        };

        // 首次启动生成模板。
        match template::create_config_file_when_missing(&path) {
            // 配置文件不存在，按模板生成以0退出
            Ok(true) => {
                println!(
                    "{}",
                    text::bootstrap::config_template_created(&path.display().to_string())
                );
                std::process::exit(0);
            }
            // 配置文件已存在，就在终端输出路径，然后进入校验流程。
            Ok(false) => println!(
                "{}",
                text::bootstrap::config_file_found(&path.display().to_string())
            ),
            Err(message) => Self::exit_on_error(message),
        }
        // 获取配置文件的文本内容
        let config_text = match std::fs::read_to_string(&path) {
            Ok(config_text) => config_text,
            Err(e) => Self::exit_on_error(text::bootstrap::config_read_failed(
                &path.display().to_string(),
                &e.to_string(),
            )),
        };
        // 尝试把文本内容解析为配置结构体
        let config = match Self::from_toml_text(&config_text) {
            Ok(config) => config,
            Err(message) => Self::exit_on_error(message),
        };

        // 解析成功后，先安装语言，然后由 validate 校验各个字段的合法性。
        text::install(text::Language::from_name(&config.i18n.language).unwrap_or_default());

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

// 取值方法

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
