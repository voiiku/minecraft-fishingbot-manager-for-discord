//! 配置结构与 TOML 表一一对应；解析见 [`super::parse`]，校验见 [`super::validate`]。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

/// 反 AFK 输入及执行间隔，目标使用 `runtime.screen_prefix`。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntiAfkConfig {
    /// 使用 input-pulse 语法发送到 screen 的输入。
    pub command: String,
    /// 每轮输入完成后的等待时间（秒），必须大于 0。
    #[serde(rename = "execution_interval")]
    pub execution_interval_seconds: u64,
    /// 每个字符或按键后的延迟（秒），允许为 0。
    #[serde(rename = "input_delay")]
    pub input_delay_seconds: f64,
}

impl AntiAfkConfig {
    /// 构建并验证实际运行使用的 input-pulse 配置。
    pub(crate) fn input_pulse_config(
        &self,
        screen_prefix: &str,
    ) -> Result<input_pulse::InputPulseConfig, String> {
        input_pulse::InputPulseConfig::new(
            input_pulse::TargetMode::ScreenSession,
            screen_prefix,
            &self.command,
            self.execution_interval_seconds,
            self.input_delay_seconds,
        )
    }
}

/// `/ping` 的默认目标与超时配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingConfig {
    /// 默认探测的服务器地址，域名或 IP。
    pub default_ping_host: String,
    /// 默认探测的服务器端口。
    pub default_ping_port: u16,
    /// `/ping` 总超时（毫秒），包括 TCP 建连与状态握手。
    #[serde(
        rename = "timeout",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub timeout_ms: u64,
}

/// Discord bot 的连接与身份信息。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscordConfig {
    /// Bot Token，必填。
    pub token: String,
}

/// 程序日志级别。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    /// 是否启用 DEBUG 级日志。
    pub debug: bool,
}

/// 程序使用的语言，影响 Discord 回复与程序日志，不影响 FishingBot 日志。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct I18nConfig {
    /// 语言名，同时写入 FishingBot 配置的 `misc.language`。
    pub language: String,
}

/// 管理员名单：Discord 用户 ID 到显示名称的映射。
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Admins(BTreeMap<String, String>);

impl Admins {
    /// 这个 Discord 用户 ID 是不是管理员。
    pub fn is_admin(&self, user_id: u64) -> bool {
        self.0.contains_key(&user_id.to_string())
    }

    /// 逐条遍历（按用户 ID 排序，`BTreeMap` 保证顺序稳定）。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(user_id, name)| (user_id.as_str(), name.as_str()))
    }

    /// 名单是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// 状态判断关键词：只配置随部署环境变化的内容。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeywordsConfig {
    /// 成功进入游戏服务器的关键词。
    pub online: String,
    /// 过滤垃圾日志用的正则，直接交给 `grep -E`。
    pub filter_regex: String,
}

/// FishingBot 原始日志的目录、文件名和丢弃目标。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGameLogsConfig {
    /// 原始日志的目录名：`<机器人目录>/<runtime.bot_logs_dir>/<它>`。
    pub dir_name: String,
    /// 原始日志文件名。
    pub file_name: String,
    /// 丢弃日志内容的目标路径。
    pub discard_path: String,
}

/// 展开后的文件路径：文件里的 `[paths]` 表写一个绝对 `base_dir` 加两个相对名，装载时展开成这里的绝对路径。
#[derive(Debug)]
pub struct Paths {
    /// 数据根目录：机器人目录、jar 与游戏配置的公共父目录。
    pub base_dir: PathBuf,
    /// 机器人目录（`base_dir/bots`），其下按 Discord 用户 ID 再分一层。
    pub bots_dir: PathBuf,
    /// FishingBot JAR 路径。
    pub jar: PathBuf,
    /// 所有机器人共用的 FishingBot 配置路径。
    pub fishingbot_config: PathBuf,
}

/// FishingBot `conf.json` 中由本项目管理的字段；其余字段沿用模板。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FishingBotConfig {
    /// 默认服务器地址。
    pub server_ip: String,
    /// 游戏服务器端口。
    pub server_port: u16,
    /// 协议版本；可填 `AUTOMATIC` 或版本号（如 `1.20.4`）。
    pub default_protocol: String,
    /// 掉线后自动重连。
    pub auto_reconnect: bool,
    /// 自动重连前的等待秒数。
    pub auto_reconnect_time: u64,
    /// 进服后执行的指令；空表表示不执行。
    pub start_text: Vec<String>,
    /// 钓到物品时的控制台播报类型。
    pub announce_type_console: String,
    /// 钓到物品时的游戏聊天播报类型。
    pub announce_type_chat: String,
}

/// 机器人运行参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    /// JVM 启动参数。
    pub java_opts: String,
    /// screen 会话名前缀。
    pub screen_prefix: String,
    /// refreshToken 文件名；文件权限限制为 `0600`。
    pub token_file: String,
    /// 机器人日志目录名。
    pub bot_logs_dir: String,
    /// `/log` 返回的最大行数。
    pub max_reply_log_lines: usize,
    /// 单条回复字符上限，超出时按行截断。
    pub max_reply_chars: usize,
    /// 自动候选（autocomplete）单次返回的最大条数；不超过 25（Discord 硬上限）。
    pub max_autocomplete_choices: usize,
}

/// 启动队列的批次与冷却设置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupQueueConfig {
    /// 每批最多启动的机器人数量。
    pub max_batch_size: usize,
    /// 批内相邻两台的登录间隔（毫秒）。
    ///
    /// 微软按出口 IP 限制登录频率（见 `docs/troubleshooting/troubleshooting-zh_cn.md` 第 12 节），
    /// 这个值管"同一批里相邻两台之间"的错峰。
    #[serde(
        rename = "bot_login_interval",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub bot_login_interval_ms: u64,
    /// 批与批之间的间隔（毫秒）。
    ///
    /// 一批跑完（拉起 + 复核）之后至少隔它才开下一批——即使上一批只有一台。
    #[serde(
        rename = "batch_login_interval",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub batch_login_interval_ms: u64,
    /// 全局队列最多暂存的启动请求数量。
    ///
    /// 它同时决定等结果的兜底超时（队列排满时最慢的一次启动 × 2，见
    /// `StartupQueue::result_timeout`）：排队的台数越多，这个超时越长，不单独配置。
    pub max_pending_requests: usize,
}

/// 启动、停止与看门狗的时序参数；配置为秒，内部保存毫秒。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingConfig {
    /// 每次启动请求的机器人全部拉起后的会话存活宽限期（毫秒）。
    ///
    /// 复核据此判定"拉起来了没有"：拉起后立即判定一次（会话消失或残壳即失败），
    /// 过了宽限期会话仍存活即算启动成功；此刻是否已进服由日志另行说明，不参与判定。
    #[serde(
        rename = "check_grace",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub check_grace_ms: u64,
    /// 等待 JVM 优雅退出的时间（毫秒），超时后强杀。
    #[serde(
        rename = "stop_grace",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub stop_grace_ms: u64,
    /// 强制退出后、重新启动前的等待时间（毫秒）。
    #[serde(
        rename = "stop_settle",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub stop_settle_ms: u64,
    /// 看门狗判定窗口（毫秒），掉线后也用作重连等待时间。
    /// 探活间隔与首次握手超时由它派生
    #[serde(
        rename = "watchdog_window",
        deserialize_with = "super::parse::seconds_to_millis"
    )]
    pub watchdog_window_ms: u64,
}

impl TimingConfig {
    /// 判定窗口。
    pub fn watchdog_window(&self) -> Duration {
        Duration::from_millis(self.watchdog_window_ms)
    }

    /// REST 探活间隔：判定窗口的 1/3（示例配置与文档里的比例）；窗口写得很小时也不小于 1 毫秒。
    pub fn probe_interval(&self) -> Duration {
        Duration::from_millis((self.watchdog_window_ms / 3).max(1))
    }

    /// 每轮连接的首次握手超时：判定窗口的 2 倍。
    pub fn first_handshake_timeout(&self) -> Duration {
        Duration::from_millis(self.watchdog_window_ms.saturating_mul(2))
    }
}

/// 自动候选单次返回的条数上限（Discord 对 autocomplete 的硬上限）。
pub const MAX_AUTOCOMPLETE_CHOICES: usize = 25;

/// `/pause` 与 `/unpause` 的 JAR 回显及检查参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEchoConfig {
    /// `/pause` 生效后 JAR 打印的回显（默认值按英文 jar；中文 jar 是 `已暂停钓鱼`）。
    pub pause_echo: String,
    /// `/unpause` 生效后 JAR 打印的回显（默认值按英文 jar；中文 jar 是 `已恢复钓鱼`）。
    pub unpause_echo: String,
    /// 指令发出去之后等多久再去看日志回显（毫秒）。
    #[serde(rename = "wait", deserialize_with = "super::parse::seconds_to_millis")]
    pub wait_ms: u64,
    /// 检查回显时读取的日志行数。
    pub scan_lines: usize,
}

/// 可自定义的帮助文本。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextConfig {
    /// `/help` 的文本。
    pub help: String,
}
