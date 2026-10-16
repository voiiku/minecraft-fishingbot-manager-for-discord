//! 配置结构与 TOML 表一一对应；解析见 [`super::parse`]，校验见 [`super::validate`]。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

/// `/ping` 的默认目标与超时配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingConfig {
    /// 默认探测的服务器地址，域名或 IP。
    pub default_ping_host: String,
    /// 默认探测的服务器端口。
    pub default_ping_port: u16,
    /// `/ping` 总超时（毫秒），包括 TCP 建连与状态握手。
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
    /// 自动候选（autocomplete）单次返回的最大条数；不超过 [`MAX_AUTOCOMPLETE_CHOICES`]。
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
    pub bot_login_interval_ms: u64,
    /// 批与批之间的间隔（毫秒）。
    ///
    /// 一批跑完（拉起 + 复核）之后至少隔它才开下一批——即使上一批只有一台。
    pub batch_login_interval_ms: u64,
    /// 全局队列最多暂存的启动请求数量。
    ///
    /// 它同时决定等结果的兜底超时（队列排满时最慢的一次启动 × 2，见
    /// [`crate::utils::StartupQueue::result_timeout`]）：排队的台数越多，这个超时越长，不单独配置。
    pub max_pending_requests: usize,
}

/// 启动、停止与看门狗的时序参数，时间单位为毫秒。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingConfig {
    /// 每台机器人拉起后的会话存活宽限期（毫秒）。
    ///
    /// 复核据此判定"拉起来了没有"：拉起后立即判定一次（会话消失或残壳即失败），
    /// 过了宽限期会话仍存活即算启动成功；此刻是否已进服由日志另行说明，不参与判定。
    pub check_grace_ms: u64,
    /// 等待 JVM 优雅退出的时间（毫秒），超时后强杀。
    pub stop_grace_ms: u64,
    /// 强制退出后、重新启动前的等待时间（毫秒）。
    pub stop_settle_ms: u64,
    /// 看门狗判定窗口（毫秒），掉线后也用作重连等待时间。
    /// 探活间隔与首次握手超时由它派生
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// 示例配置必须符合当前 schema。
    #[test]
    fn example_config_matches_schema() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let config = Config::from_toml_text(text).expect(
            "minecraft-fishingbot-manager-for-discord.conf.example.toml 与当前配置 schema 不一致，请同步更新示例文件",
        );

        // 示例配置字段必须齐全。
        assert!(!config.ping.default_ping_host.trim().is_empty());
        assert!(config.ping.default_ping_port > 0);
        assert!(config.ping.timeout_ms > 0);
        assert!(!config.runtime.screen_prefix.trim().is_empty());
        assert!(config.timing.watchdog_window_ms > 0);
        assert!(config.timing.check_grace_ms > 0);
        // 探活间隔与首次握手超时由判定窗口派生：一个窗口里至少能探一次，握手超时覆盖一整轮。
        assert!(config.timing.probe_interval() > Duration::ZERO);
        assert!(config.timing.probe_interval() <= config.timing.watchdog_window());
        assert_eq!(
            config.timing.first_handshake_timeout(),
            config.timing.watchdog_window() * 2
        );
        let startup_queue = &config.startup_queue;
        assert!(startup_queue.max_batch_size > 0);
        assert!(startup_queue.bot_login_interval_ms > 0);
        assert!(startup_queue.max_pending_requests > 0);
        assert!(!config.command_echo.pause_echo.trim().is_empty());
        assert!(!config.command_echo.unpause_echo.trim().is_empty());
        assert!(config.command_echo.wait_ms > 0);
        assert!(config.command_echo.scan_lines > 0);
        // 必填表必须包含在示例配置中。
        assert!(text.contains("[discord]"));
        assert!(text.contains("[admins]"));
        assert!(text.contains("[keywords]"));
        assert!(text.contains("[command_echo]"));
        assert!(text.contains("[startup_queue]"));
        // 帮助文本应不为空。
        assert!(!config.text.help.trim().is_empty());
    }

    /// 示例配置中的字段都应记录在配置文档中。
    #[test]
    fn every_example_key_is_documented() {
        let docs = include_str!("../../docs/mods/config/config-zh_cn.md");
        let keys = example_keys();

        // 先确认抽取器本身没失灵：真抽空了，下面的断言会变成"永远通过"。
        assert!(
            keys.contains(&"max_reply_chars".to_string()),
            "从示例配置里没抽到键名，测试失效: {keys:?}"
        );

        let undocumented_keys: Vec<String> = keys
            .into_iter()
            .filter(|key| !docs.contains(&format!("`{key}`")))
            .collect();

        assert!(
            undocumented_keys.is_empty(),
            "这些字段没写进 docs/mods/config/config-zh_cn.md: {undocumented_keys:?}"
        );
    }

    /// 从示例配置里抽出所有裸键名（`key = value` 里的 `key`）。
    ///
    /// 跳过注释行、表头行、`"Discord用户ID" = "名字"` 这类引号键，以及 `"""` 多行字符串里的内容：
    /// 示例的 `help` 是一整段文案，里面的中文说明不该被当成配置项。
    fn example_keys() -> Vec<String> {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let mut keys = Vec::new();
        let mut in_multiline_string = false;

        for line in text.lines() {
            let trimmed = line.trim();
            let triple_quote_count = trimmed.matches("\"\"\"").count();

            if in_multiline_string {
                // 结束那一行也只有一个三引号（内容在它前面），靠奇偶判断。
                if triple_quote_count % 2 == 1 {
                    in_multiline_string = false;
                }
                continue;
            }

            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
                continue;
            }

            let Some((key, _)) = trimmed.split_once(" = ") else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue; // 引号键（`[admins]` 里的 Discord 用户 ID）不算字段名
            }
            keys.push(key.to_string());

            if triple_quote_count % 2 == 1 {
                in_multiline_string = true;
            }
        }

        keys
    }

    /// 管理员名单：能按 Discord 用户 ID 查权限，也能取到名字。
    #[test]
    fn admins_lookup_by_user_id() {
        let admins = Admins(BTreeMap::from([(
            "2878855255".to_string(),
            "SlanceLenk".to_string(),
        )]));

        assert!(admins.is_admin(2878855255));
        assert!(!admins.is_admin(123456));
        assert!(!admins.is_empty());
        assert!(
            admins
                .iter()
                .any(|(user_id, name)| user_id == "2878855255" && name == "SlanceLenk")
        );
    }

    /// 少写一个必需的表时必须报错，而不是悄悄用某个默认值。
    #[test]
    fn missing_required_table_is_error() {
        let text = r#"
[ping]
host = "example.com"
port = 25565
"#;

        assert!(toml::from_str::<Config>(text).is_err());
    }

    /// 字段名拼错时必须报错（`deny_unknown_fields`）。
    #[test]
    fn unknown_field_is_error() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml")
            .replace("[runtime]", "[runtime]\nunknown_field = 1");

        assert!(toml::from_str::<Config>(&text).is_err());
    }

    /// 造一个绝对路径：本项目只跑在 Linux 上，加个 `/` 就行。
    fn absolute(path: &str) -> PathBuf {
        PathBuf::from(format!("/{path}"))
    }

    /// 按配置文件里的写法拼一张 `[paths]` 表再反序列化：与真实装载走同一条路。
    fn read_paths(
        base_dir: &str,
        jar: &str,
        fishingbot_config_file_name: &str,
    ) -> Result<Paths, String> {
        let text = format!(
            "base_dir = {base_dir:?}\njar = {jar:?}\nfishingbot_config_file_name = {fishingbot_config_file_name:?}\n"
        );

        toml::from_str(&text).map_err(|error| error.to_string())
    }

    /// `base_dir` 必须是绝对路径、`jar` 与 `fishingbot_config_file_name` 必须是相对路径（相对
    /// `base_dir`）；派生出来的 `bots_dir` 一定在 `base_dir` 之下。
    #[test]
    fn paths_follow_the_absolute_relative_rule() {
        let base_dir = absolute("srv/fishbot/data");
        let paths = read_paths(
            &base_dir.display().to_string(),
            "FishingBot.jar",
            "conf.json",
        )
        .expect("符合规则的路径应当通过");

        assert_eq!(paths.base_dir, base_dir);
        assert_eq!(paths.bots_dir, base_dir.join("bots"));
        assert_eq!(paths.jar, base_dir.join("FishingBot.jar"));
        assert_eq!(
            paths.fishingbot_config,
            base_dir.join("conf.json"),
            "配置里写的是文件名，运行时是展开后的完整路径"
        );
    }

    /// 相对路径的 `base_dir` 会被拒绝（提示写绝对路径）。
    #[test]
    fn relative_base_dir_is_rejected() {
        let err = read_paths("fishingbots", "FishingBot.jar", "conf.json")
            .expect_err("相对 base_dir 应当报错");

        // 措辞断言钉默认语言（英语）的片段（默认语言是 `Language::default()`，见 `docs/mods/i18n/i18n-zh_cn.md`）。
        assert!(
            err.contains("absolute path"),
            "报错应说明要写绝对路径: {err}"
        );
    }

    /// 绝对路径的 `jar` / `fishingbot_config_file_name` 会被拒绝。
    #[test]
    fn absolute_jar_or_fishingbot_config_is_rejected() {
        let base_dir = absolute("srv/fishbot/data").display().to_string();

        let err = read_paths(
            &base_dir,
            &absolute("opt/FishingBot.jar").display().to_string(),
            "conf.json",
        )
        .expect_err("绝对 jar 应当报错");
        // 措辞断言同 `relative_base_dir_is_rejected`：钉默认语言（英语）的片段。
        assert!(
            err.contains("relative path"),
            "报错应说明要写相对路径: {err}"
        );

        let err = read_paths(
            &base_dir,
            "FishingBot.jar",
            &absolute("etc/conf.json").display().to_string(),
        )
        .expect_err("绝对 fishingbot_config_file_name 应当报错");
        assert!(
            err.contains("fishingbot_config_file_name"),
            "报错应指出字段名: {err}"
        );
    }
}
