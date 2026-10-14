//! 启动配置校验：检查必填字段、取值范围与命令安全相关的字符串。

use crate::text;

use super::schema::{
    Admins, CommandEchoConfig, Config, DiscordConfig, FishingBotConfig, I18nConfig, KeywordsConfig,
    MAX_AUTOCOMPLETE_CHOICES, Paths, PingConfig, RawGameLogsConfig, RuntimeConfig,
    StartupQueueConfig, TextConfig, TimingConfig,
};

/// shell 命令里未加引号的内联取值禁止出现的字符（`runtime.java_opts`）。
const SHELL_UNSAFE: &[char] = &['\'', '"', '`', '$', ';', '&', '|', '<', '>', '\n', '\r'];

/// 校验整份配置：每张表一个校验函数，这里只按顺序调用它们。
pub(super) fn validate(config: &Config) -> Result<(), String> {
    validate_discord(&config.discord)?;
    validate_paths(&config.paths)?;
    validate_fishingbot(&config.fishingbot)?;
    validate_admins(&config.admins)?;
    validate_keywords(&config.keywords)?;
    validate_command_echo(&config.command_echo)?;
    validate_i18n(&config.i18n)?;
    validate_runtime(&config.runtime)?;
    validate_startup_queue(&config.startup_queue)?;
    validate_timing(&config.timing)?;
    validate_raw_game_logs(&config.raw_game_logs)?;
    validate_ping(&config.ping)?;
    validate_text(&config.text)
}

/// 生成字段级错误信息。
fn invalid(field: impl std::fmt::Display, requirement: text::ConfigRequirement<'_>) -> String {
    text::get().log_text(text::LogText::ConfigFieldInvalid {
        field: &field.to_string(),
        requirement,
    })
}

/// 生成包含字段值的错误信息。
fn invalid_value(
    field: impl std::fmt::Display,
    value: impl std::fmt::Display,
    requirement: text::ConfigRequirement<'_>,
) -> String {
    text::get().log_text(text::LogText::ConfigFieldValueInvalid {
        field: &field.to_string(),
        value: &value.to_string(),
        requirement,
    })
}

/// 检查必填字符串（值已在 `parse` 里去过两头的空白，这里只判空）。
fn require_non_empty(field: impl std::fmt::Display, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(invalid(field, text::ConfigRequirement::NonEmpty));
    }

    Ok(())
}

/// 检查用作单个名字的字段，拒绝空白、路径分隔符及 `.`、`..`。
fn require_plain_name(field: impl std::fmt::Display, value: &str) -> Result<(), String> {
    let has_whitespace = value.chars().any(char::is_whitespace);
    let has_separator = value.contains(['/', '\\']);
    let is_dot_name = matches!(value, "." | "..");

    if has_whitespace || has_separator || is_dot_name {
        return Err(invalid(field, text::ConfigRequirement::PlainName));
    }

    // 空字符串交由非空检查处理。
    require_non_empty(field, value)
}

/// `/ping` 的默认目标与探测超时。
fn validate_ping(ping: &PingConfig) -> Result<(), String> {
    require_non_empty("ping.default_ping_host", &ping.default_ping_host)?;
    if ping.default_ping_port == 0 {
        return Err(invalid(
            "ping.default_ping_port",
            text::ConfigRequirement::PortRange,
        ));
    }
    if ping.timeout_ms == 0 {
        return Err(invalid(
            "ping.timeout_ms",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }

    Ok(())
}

/// 检查 Discord Token。
fn validate_discord(discord: &DiscordConfig) -> Result<(), String> {
    if discord.token.is_empty() {
        return Err(invalid(
            "discord.token",
            text::ConfigRequirement::DiscordTokenEmpty,
        ));
    }

    Ok(())
}

/// 检查语言名是否受支持。
fn validate_i18n(i18n: &I18nConfig) -> Result<(), String> {
    crate::text::Language::from_name(&i18n.language)
        .map(|_| ())
        .map_err(|message| {
            invalid(
                "i18n.language",
                text::ConfigRequirement::LanguageUnknown { message: &message },
            )
        })
}

/// 检查管理员 ID 与显示名称；名单可以为空。
fn validate_admins(admins: &Admins) -> Result<(), String> {
    for (user_id, name) in admins.iter() {
        if user_id.parse::<u64>().is_err() {
            return Err(invalid(
                "admins",
                text::ConfigRequirement::AdminsKeyUserId { user_id },
            ));
        }
        if name.is_empty() {
            return Err(invalid(
                "admins",
                text::ConfigRequirement::AdminsNameEmpty { user_id },
            ));
        }
    }

    Ok(())
}

/// 检查日志关键词；过滤正则不能包含单引号。
fn validate_keywords(keywords: &KeywordsConfig) -> Result<(), String> {
    let fields = [
        ("online", &keywords.online),
        ("filter_regex", &keywords.filter_regex),
    ];

    for (field, value) in fields {
        require_non_empty(format!("keywords.{field}"), value)?;
    }

    if keywords.filter_regex.contains('\'') {
        return Err(invalid(
            "keywords.filter_regex",
            text::ConfigRequirement::NoSingleQuote,
        ));
    }

    Ok(())
}

/// 检查数据目录、jar 与 shell 命令中的路径。
fn validate_paths(paths: &Paths) -> Result<(), String> {
    if !paths.base_dir.is_dir() {
        return Err(invalid_value(
            "paths.base_dir",
            paths.base_dir.display(),
            text::ConfigRequirement::BaseDirIsDir,
        ));
    }

    if !paths.jar.is_file() {
        return Err(invalid_value(
            "paths.jar",
            paths.jar.display(),
            text::ConfigRequirement::JarIsFile,
        ));
    }

    // 路径会放进 shell 单引号参数。
    for (field, path) in [
        ("jar", &paths.jar),
        ("fishingbot_config_file_name", &paths.fishingbot_config),
    ] {
        if path.to_string_lossy().contains('\'') {
            return Err(invalid(
                format!("paths.{field}"),
                text::ConfigRequirement::NoSingleQuote,
            ));
        }
    }

    Ok(())
}

/// 检查 JVM 参数、目录名与日志限制。
fn validate_runtime(runtime: &RuntimeConfig) -> Result<(), String> {
    if runtime.java_opts.contains(SHELL_UNSAFE) {
        return Err(invalid(
            "runtime.java_opts",
            text::ConfigRequirement::ShellSafe,
        ));
    }

    for (field, value) in [
        ("screen_prefix", &runtime.screen_prefix),
        ("token_file", &runtime.token_file),
        ("logs_dir", &runtime.logs_dir),
    ] {
        require_plain_name(format!("runtime.{field}"), value)?;
    }

    if runtime.max_log_lines == 0 {
        return Err(invalid(
            "runtime.max_log_lines",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if runtime.max_reply_chars == 0 {
        return Err(invalid(
            "runtime.max_reply_chars",
            text::ConfigRequirement::ReplyCharsPositive,
        ));
    }
    if runtime.max_autocomplete_choices == 0
        || runtime.max_autocomplete_choices > MAX_AUTOCOMPLETE_CHOICES
    {
        return Err(invalid_value(
            "runtime.max_autocomplete_choices",
            runtime.max_autocomplete_choices,
            text::ConfigRequirement::AutocompleteChoicesInRange,
        ));
    }

    Ok(())
}

/// 检查启动队列批次设置。
fn validate_startup_queue(startup_queue: &StartupQueueConfig) -> Result<(), String> {
    if startup_queue.max_batch_size == 0 {
        return Err(invalid(
            "startup_queue.max_batch_size",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if startup_queue.login_interval_ms == 0 {
        return Err(invalid(
            "startup_queue.login_interval_ms",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if startup_queue.max_pending_requests == 0 {
        return Err(invalid(
            "startup_queue.max_pending_requests",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }

    Ok(())
}

/// 检查等待时间与看门狗时序。
fn validate_timing(timing: &TimingConfig) -> Result<(), String> {
    // 复核等待不能为 0，否则"拉起完立刻判定"，等于没复核。
    if timing.settle_per_bot_ms == 0 {
        return Err(invalid(
            "timing.settle_per_bot_ms",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }

    // 看门狗判定窗口写 0 会让判定退化（永远证明不了"还连着 Discord"）。探活间隔与首次握手超时由它
    // 按固定比例派生（见 `TimingConfig::probe_interval` / `first_handshake_timeout`），不再是配置项，
    // 所以"探活比窗口还慢"这种组合已经不可能配出来。
    if timing.watchdog_window_ms == 0 {
        return Err(invalid(
            "timing.watchdog_window_ms",
            text::ConfigRequirement::WatchdogTimingPositive,
        ));
    }

    Ok(())
}

/// 检查 FishingBot 原始日志配置。
fn validate_raw_game_logs(raw_game_logs: &RawGameLogsConfig) -> Result<(), String> {
    require_plain_name("raw_game_logs.dir_name", &raw_game_logs.dir_name)?;
    require_plain_name("raw_game_logs.file_name", &raw_game_logs.file_name)?;
    require_non_empty("raw_game_logs.discard_path", &raw_game_logs.discard_path)
}

/// 检查暂停/恢复回显及其等待参数。
fn validate_command_echo(command_echo: &CommandEchoConfig) -> Result<(), String> {
    require_non_empty("command_echo.pause_echo", &command_echo.pause_echo)?;
    require_non_empty("command_echo.unpause_echo", &command_echo.unpause_echo)?;
    if command_echo.pause_echo.contains(&command_echo.unpause_echo)
        || command_echo.unpause_echo.contains(&command_echo.pause_echo)
    {
        return Err(invalid(
            "command_echo.unpause_echo",
            text::ConfigRequirement::EchoTextsOverlap,
        ));
    }
    if command_echo.wait_ms == 0 {
        return Err(invalid(
            "command_echo.wait_ms",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if command_echo.scan_lines == 0 {
        return Err(invalid(
            "command_echo.scan_lines",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }

    Ok(())
}

/// 帮助文本。
fn validate_text(text: &TextConfig) -> Result<(), String> {
    require_non_empty("text.help", &text.help)
}

/// 检查写入 FishingBot 配置的字段。
fn validate_fishingbot(fishingbot: &FishingBotConfig) -> Result<(), String> {
    require_non_empty("fishingbot.server_ip", &fishingbot.server_ip)?;
    if fishingbot.server_port == 0 {
        return Err(invalid(
            "fishingbot.server_port",
            text::ConfigRequirement::PortRange,
        ));
    }

    let protocol = &fishingbot.default_protocol;
    if protocol != "AUTOMATIC" && !is_mc_version_string(protocol) {
        return Err(invalid_value(
            "fishingbot.default_protocol",
            protocol,
            text::ConfigRequirement::McVersionOrAutomatic,
        ));
    }

    for (index, command) in fishingbot.start_text.iter().enumerate() {
        require_non_empty(format!("fishingbot.start_text[{index}]"), command.as_str())?;
    }

    for (field, value) in [
        (
            "fishingbot.announce_type_console",
            &fishingbot.announce_type_console,
        ),
        (
            "fishingbot.announce_type_chat",
            &fishingbot.announce_type_chat,
        ),
    ] {
        if !ANNOUNCE_TYPES.contains(&value.as_str()) {
            return Err(invalid_value(
                field,
                value,
                text::ConfigRequirement::OneOf(ANNOUNCE_TYPES),
            ));
        }
    }

    Ok(())
}

/// FishingBot 的 `announce-type-chat` / `announce-type-console` 合法取值（上游 `AnnounceType` 枚举）。
const ANNOUNCE_TYPES: &[&str] = &[
    "ALL",
    "ALL_BUT_FISH",
    "ONLY_ENCHANTED",
    "ONLY_BOOKS",
    "NONE",
];

/// 像不像 Minecraft 版本号：`1.20`、`1.20.4`（上游 `ProtocolConstants` 的版本字符串就长这样）。
/// 只做形状检查：具体版本是不是被这份 jar 支持，只有 jar 自己知道。
fn is_mc_version_string(value: &str) -> bool {
    let mut parts = value.split('.');
    let mut part_count = 0;

    for part in parts.by_ref() {
        part_count += 1;
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }

    (2..=3).contains(&part_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;
    use std::path::{Path, PathBuf};

    /// 配置示例；测试只依赖其结构。
    const EXAMPLE: &str =
        include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");

    /// 把 `field = ...` 这一行换成 `field = <value>`（`value` 原样写，例如 `"\"x\""`）。
    fn set_field(text: &str, field: &str, value: &str) -> String {
        let prefix = format!("{field} ");
        let mut done = false;

        text.lines()
            .map(|line| {
                if !done && line.trim_start().starts_with(&prefix) {
                    done = true;
                    format!("{field} = {value}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 更新可选 `[startup_queue]` 表中的一个键。
    fn set_startup_queue_field(text: &str, field: &str, value: &str) -> String {
        let mut in_startup_queue = false;
        let mut replaced = false;
        let mut output = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_startup_queue = trimmed == "[startup_queue]";
            }

            if in_startup_queue && !replaced && trimmed.starts_with(&format!("{field} =")) {
                output.push(format!("{field} = {value}"));
                replaced = true;
            } else {
                output.push(line.to_string());
            }
        }

        assert!(
            replaced,
            "startup_queue.{field} must exist in the example config"
        );
        output.join("\n")
    }

    /// 造一个临时目录，并在里面按示例配置写的 jar 文件名放一个占位文件；
    /// 然后把 `base_dir` 指向这个目录（示例里写的是占位路径，测试必须换掉）。
    ///
    /// 目录名的唯一来源是 [`crate::test_support::temp_test_dir`]（带进程号、跑前先清）。
    fn staged_example(tag: &str) -> (PathBuf, String) {
        let dir = crate::test_support::temp_test_dir(&format!("validate-{tag}"));
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");

        // jar 名字跟着示例走，免得示例里换了版本号就把测试弄挂。
        let jar_name = EXAMPLE
            .lines()
            .find_map(|line| line.trim_start().strip_prefix("jar = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| "FishingBot.jar".to_string());
        std::fs::write(dir.join(jar_name), b"jar").expect("写入测试 jar 失败");

        // 路径里不能出现反斜杠：TOML 基本字符串会把 `\` 当转义符。
        let base_dir = dir.to_string_lossy().replace('\\', "/");
        let text = set_field(EXAMPLE, "base_dir", &format!("\"{base_dir}\""));

        (dir, text)
    }

    /// 跑一遍 `load_from` 的核心流程：解析 + 校验。
    fn load_text(text: &str, config_dir: &Path) -> Result<Config, String> {
        let config = parse::parse(
            text,
            &config_dir.join("minecraft-fishingbot-manager-for-discord.conf.toml"),
        )?;
        validate(&config)?;
        Ok(config)
    }

    /// 路径都就位时，示例配置应当通过校验。
    #[test]
    fn example_config_passes_validation() {
        let (dir, text) = staged_example("ok");

        let config = load_text(&text, &dir).expect("示例配置应当通过校验");
        assert_eq!(config.paths.base_dir, dir);
        assert!(config.paths.jar.is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 自动候选上限必须落在 1..=25：0 与 26 都要被打回来（Discord 的硬上限是 25）。
    #[test]
    fn autocomplete_choices_must_be_within_the_platform_limit() {
        let (dir, text) = staged_example("autocomplete-limit");

        for value in ["0", "26"] {
            let text = set_field(&text, "max_autocomplete_choices", value);
            let error = load_text(&text, &dir).expect_err("超出 1..=25 的候选上限应当被拒绝");
            assert!(
                error.contains("runtime.max_autocomplete_choices"),
                "报错应指出字段名: {error}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `base_dir` 不存在或不是目录（首次部署最容易犯的错）要报出字段名与原因。
    #[test]
    fn missing_base_dir_is_reported() {
        let (dir, text) = staged_example("missing-base");
        // 这里必须仍然写成绝对路径：否则会先被"路径形态"校验拦下，测不到下面这条规则。
        let missing = dir.join("not-there").to_string_lossy().replace('\\', "/");
        let text = set_field(&text, "base_dir", &format!("\"{missing}\""));

        let err = load_text(&text, &dir).expect_err("目录不存在应当报错");

        assert!(err.contains("paths.base_dir"), "报错应指出字段: {err}");
        // 措辞断言钉默认语言（英语）的片段（见 `docs/mods/i18n/i18n-zh_cn.md`）。
        assert!(
            err.contains("does not exist or is not a directory"),
            "报错应说明原因: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 能破坏启动命令的 `java_opts` 要拦下来。
    #[test]
    fn unsafe_java_opts_is_reported() {
        let (dir, text) = staged_example("java-opts");
        let text = set_field(&text, "java_opts", "\"-Xmx128m; rm -rf /\"");

        let err = load_text(&text, &dir).expect_err("危险字符应当报错");

        assert!(err.contains("runtime.java_opts"), "报错应指出字段: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 两条回显相同（或互相包含）时判不出日志里那行属于哪条指令，要在启动时报出来。
    #[test]
    fn overlapping_command_echoes_are_reported() {
        let (dir, text) = staged_example("echo-overlap");
        // 把 /unpause 的回显改成与 /pause 那句一样：这正是抄错一行时的样子。
        let text = set_field(&text, "unpause_echo", "\"Fishing paused\"");

        let err = load_text(&text, &dir).expect_err("两条回显相同应当报错");

        assert!(
            err.contains("command_echo.unpause_echo"),
            "报错应指出字段: {err}"
        );
        // 措辞断言钉默认语言（英语）的片段（见 `docs/mods/i18n/i18n-zh_cn.md`）。
        assert!(
            err.contains("must not be equal to or overlap"),
            "报错应说明原因: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 会话名前缀里的空白会让 screen 命令变形。
    #[test]
    fn unsafe_screen_prefix_is_reported() {
        let (dir, text) = staged_example("screen-prefix");
        let text = set_field(&text, "screen_prefix", "\"fb _\"");

        let err = load_text(&text, &dir).expect_err("空白字符应当报错");

        assert!(
            err.contains("runtime.screen_prefix"),
            "报错应指出字段: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `/log` 的行数上限为 0 时永远看不到日志，必须在启动时就拦下。
    #[test]
    fn zero_max_log_lines_is_reported() {
        let (dir, text) = staged_example("max-log-lines");
        let text = set_field(&text, "max_log_lines", "0");

        let err = load_text(&text, &dir).expect_err("0 应当报错");

        assert!(
            err.contains("runtime.max_log_lines"),
            "报错应指出字段: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `/ping` 的超时为 0 时永远等不到响应，必须在启动时就拦下。
    #[test]
    fn zero_ping_timeout_is_reported() {
        let (dir, text) = staged_example("ping-timeout");
        let text = set_field(&text, "timeout_ms", "0");

        let err = load_text(&text, &dir).expect_err("0 应当报错");

        assert!(err.contains("ping.timeout_ms"), "报错应指出字段: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 语言取值不认识时要在启动时就拦下，并把可选值列出来（部署者照着改即可）。
    #[test]
    fn unknown_language_is_reported() {
        let (dir, text) = staged_example("unknown-language");
        // 用一个确实没实现的取值：`GERMAN` 是 jar 认识的语言，但本项目没写德语文案。
        let text = set_field(&text, "language", "\"GERMAN\"");

        let err = load_text(&text, &dir).expect_err("不认识的语言应当报错");

        assert!(err.contains("i18n.language"), "报错应指出字段: {err}");
        assert!(err.contains("ENGLISH"), "报错应列出可选值: {err}");
        assert!(
            err.contains("CHINESE_SIMPLIFIED"),
            "报错应列出可选值: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 网关看门狗的时序项写 0 会让判定退化（比如永远证明不了"还连着 Discord"），必须在启动时就拦下。
    #[test]
    fn zero_watchdog_timing_is_reported() {
        let (dir, text) = staged_example("zero-watchdog-window");
        let text = set_field(&text, "watchdog_window_ms", "0");

        let err = load_text(&text, &dir).expect_err("0 应当报错");

        assert!(
            err.contains("timing.watchdog_window_ms"),
            "报错应指出字段: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 复核等待写 0 会变成"拉起完立刻判定"，等于没复核：启动时就拦下。
    #[test]
    fn zero_settle_per_bot_ms_is_reported() {
        let (dir, text) = staged_example("zero-settle-per-bot");
        let text = set_field(&text, "settle_per_bot_ms", "0");

        let err = load_text(&text, &dir).expect_err("0 应当报错");

        assert!(
            err.contains("timing.settle_per_bot_ms"),
            "报错应指出字段: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 启动队列的批次大小与登录间隔必须大于 0。
    #[test]
    fn zero_start_batch_timing_is_reported() {
        for (field, tag) in [
            ("max_batch_size", "zero-start-batch-size"),
            ("login_interval_ms", "zero-start-interval"),
            ("max_pending_requests", "zero-start-queue-capacity"),
        ] {
            let (dir, text) = staged_example(tag);
            let text = set_startup_queue_field(&text, field, "0");

            let err = load_text(&text, &dir).expect_err("启动队列设置为 0 应当报错");

            assert!(
                err.contains(&format!("startup_queue.{field}")),
                "报错应指出字段 {field}: {err}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// 管理员名单的键必须是 Discord 用户 ID：写成别的会让权限静默失效。
    #[test]
    fn admin_key_must_be_user_id() {
        let admins = admins_from("\"abc\" = \"名字\"");

        let err = validate_admins(&admins).expect_err("非数字键应当报错");

        // 措辞断言钉默认语言（英语）的片段（见 `docs/mods/i18n/i18n-zh_cn.md`）。
        assert!(
            err.contains("is not a Discord user ID"),
            "报错应说明键要是 Discord 用户 ID: {err}"
        );
    }

    /// 管理员的名字不能是空的，否则 /show admins 会很难看（全空白的名字在 `parse` 里已经被去成空串，
    /// 所以进到 `validate` 的只可能是空串）。
    #[test]
    fn admin_name_must_not_be_empty() {
        let admins = admins_from("\"2878855255\" = \"\"");

        let err = validate_admins(&admins).expect_err("空名字应当报错");

        assert!(
            err.contains("must not be empty"),
            "报错应说明名字为空: {err}"
        );
    }

    /// 正常名单应当放行，并且能按 Discord 用户 ID 查到权限。
    #[test]
    fn good_admins_pass() {
        let admins = admins_from("\"2878855255\" = \"SlanceLenk\"\n\"123456\" = \"小号\"");

        assert!(validate_admins(&admins).is_ok());
        assert!(admins.is_admin(2878855255));
        assert!(admins.is_admin(123456));
    }

    /// 按真实格式解析一份管理员名单：`Admins` 的字段是私有的，测试里走一遍 TOML 更贴近实际。
    fn admins_from(text: &str) -> Admins {
        toml::from_str(text).expect("测试用的管理员名单应当能解析")
    }
}
