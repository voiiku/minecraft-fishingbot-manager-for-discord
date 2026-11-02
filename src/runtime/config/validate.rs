//! 启动配置校验：检查必填字段、取值范围与命令安全相关的字符串。
//!
//! [`validate`] 的第一个调用是语言校验：此时还没有文案实现，语言错误固定英文；语言合法后立刻安装，
//! 其余校验的错误才能按配置语言格式化。

use std::time::Duration;

use crate::text;

use super::Config;
use super::schema::{
    Admins, AntiAfkConfig, CommandEchoConfig, DiscordConfig, FishingBotConfig, I18nConfig,
    KeywordsConfig, MAX_AUTOCOMPLETE_CHOICES, Paths, PingConfig, RawGameLogsConfig, RuntimeConfig,
    StartupQueueConfig, TextConfig, TimingConfig,
};

/// 会被拼入 shell 命令的取值禁止出现的字符（`runtime.java_opts`、`token_file`、`bot_logs_dir`）。
const SHELL_UNSAFE: &[char] = &['\'', '"', '`', '$', ';', '&', '|', '<', '>', '\n', '\r'];

/// 校验语言名并在合法后安装语言；失败信息固定英文，不使用 `text::get()`。
fn validate_i18n(i18n: &I18nConfig) -> Result<(), String> {
    let language = text::Language::from_name(&i18n.language)
        .map_err(|message| text::bootstrap::language_invalid(&message))?;
    text::install(language);

    Ok(())
}

/// 校验整份配置：语言第一，安装后再按表校验其余字段。
pub(super) fn validate(config: &Config) -> Result<(), String> {
    validate_i18n(&config.i18n)?;
    validate_discord(&config.discord)?;
    validate_paths(&config.paths)?;
    validate_fishingbot(&config.fishingbot)?;
    validate_admins(&config.admins)?;
    validate_keywords(&config.keywords)?;
    validate_command_echo(&config.command_echo)?;
    validate_runtime(&config.runtime)?;
    validate_anti_afk(&config.anti_afk, &config.runtime.screen_prefix)?;
    validate_startup_queue(&config.startup_queue)?;
    validate_timing(&config.timing)?;
    validate_raw_game_logs(&config.raw_game_logs)?;
    validate_ping(&config.ping)?;
    validate_text(&config.text)
}

fn validate_anti_afk(config: &AntiAfkConfig, screen_prefix: &str) -> Result<(), String> {
    require_non_empty("anti_afk.command", &config.command)?;
    if config.execution_interval_seconds == 0 {
        return Err(invalid(
            "anti_afk.execution_interval",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if Duration::try_from_secs_f64(config.input_delay_seconds).is_err() {
        return Err(invalid(
            "anti_afk.input_delay",
            text::ConfigRequirement::NonNegativeDuration,
        ));
    }
    config.input_pulse_config(screen_prefix).map_err(|error| {
        invalid(
            "anti_afk.command",
            text::ConfigRequirement::InputPulseCommand { reason: &error },
        )
    })?;
    Ok(())
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

/// 检查必填字符串（值已在 `parse.rs` 里去过两头的空白，这里只判空）。
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
            "ping.timeout",
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
        ("bot_logs_dir", &runtime.bot_logs_dir),
    ] {
        require_plain_name(format!("runtime.{field}"), value)?;
    }

    // 这两个值会作为 `bash -c` 的单引号参数，必须排除 shell 特殊字符。
    for (field, value) in [
        ("token_file", &runtime.token_file),
        ("bot_logs_dir", &runtime.bot_logs_dir),
    ] {
        if value.contains(SHELL_UNSAFE) {
            return Err(invalid(
                format!("runtime.{field}"),
                text::ConfigRequirement::ShellSafe,
            ));
        }
    }

    if runtime.max_reply_log_lines == 0 {
        return Err(invalid(
            "runtime.max_reply_log_lines",
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
    if startup_queue.bot_login_interval_ms == 0 {
        return Err(invalid(
            "startup_queue.bot_login_interval",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }
    if startup_queue.batch_login_interval_ms == 0 {
        return Err(invalid(
            "startup_queue.batch_login_interval",
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
    if timing.check_grace_ms == 0 {
        return Err(invalid(
            "timing.check_grace",
            text::ConfigRequirement::GreaterThanZero,
        ));
    }

    // 探活间隔和首次握手超时按判定窗口派生，窗口必须大于零。
    if timing.watchdog_window_ms == 0 {
        return Err(invalid(
            "timing.watchdog_window",
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
            "command_echo.wait",
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

/// 检查版本号格式（如 `1.20`、`1.20.4`），不验证 JAR 是否支持该版本。
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

    use std::path::PathBuf;

    /// 配置示例；测试仅依赖其结构。
    const EXAMPLE: &str =
        include_str!("../../../minecraft-fishingbot-manager-for-discord.conf.example.toml");

    /// 替换示例配置中的唯一字段。
    fn set_field(text: &str, field: &str, value: &str) -> String {
        let prefix = format!("{field} =");
        let mut found = false;
        let modified = text
            .lines()
            .map(|line| {
                if line.trim_start().starts_with(&prefix) {
                    assert!(!found, "duplicate config field: {field}");
                    found = true;
                    format!("{field} = {value}")
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(found, "missing config field: {field}");
        modified
    }

    /// 创建临时目录，并按示例配置中的 jar 文件名放置占位文件；随后将 `base_dir` 指向该目录。
    ///
    /// 临时目录由 [`crate::test_support::temp_test_dir`] 创建：名称包含进程号，使用前会清理旧目录。
    fn staged_example(tag: &str) -> (PathBuf, String) {
        let dir = crate::test_support::temp_test_dir(&format!("validate-{tag}"));
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");

        // 占位 JAR 名称与示例配置保持一致。
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

    /// 执行解析与校验流程（包含语言校验与安装）。
    fn load_text(text: &str) -> Result<Config, String> {
        let config = Config::from_toml_text(text)?;
        validate(&config)?;
        Ok(config)
    }

    /// 路径满足条件时，示例配置应通过校验。
    #[test]
    fn example_config_passes_validation() {
        let (dir, text) = staged_example("ok");

        let config = load_text(&text).expect("示例配置应当通过校验");
        assert_eq!(config.paths.base_dir, dir);
        assert!(config.paths.jar.is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anti_afk_parameters_are_validated_at_startup() {
        let (dir, text) = staged_example("anti-afk");
        for (field, value) in [
            ("command", "\"\""),
            ("command", "\"   \""),
            ("command", "\"!UNKNOWN!\""),
            ("command", "\"!SHIFT+A!\""),
            ("execution_interval", "0"),
            ("input_delay", "-0.1"),
            ("input_delay", "inf"),
            ("input_delay", "nan"),
            ("input_delay", "1e100"),
        ] {
            let modified = set_field(&text, field, value);
            let error = load_text(&modified).expect_err("invalid anti-AFK value must fail");
            assert!(error.contains(&format!("anti_afk.{field}")), "{error}");
        }

        for value in ["0.0", "0.03", "1.5"] {
            let modified = set_field(&text, "input_delay", value);
            load_text(&modified).expect("valid input delay must be accepted");
        }

        let modified = set_field(&text, "command", "\"status!ENTER!\"");
        let config = load_text(&modified).expect("valid screen input must be accepted");
        let pulse = config
            .anti_afk
            .input_pulse_config(&config.runtime.screen_prefix)
            .expect("validated config must construct the runtime input");
        assert_eq!(pulse.target_mode, input_pulse::TargetMode::ScreenSession);
        assert_eq!(pulse.target_match, config.runtime.screen_prefix);
        assert_eq!(pulse.command, "status!ENTER!");
        assert_eq!(pulse.execution_interval_seconds, 180);
        assert_eq!(pulse.input_delay_seconds, 0.05);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn autocomplete_choices_must_be_within_the_platform_limit() {
        let (dir, text) = staged_example("autocomplete-limit");

        for value in ["0", "26"] {
            let text = set_field(&text, "max_autocomplete_choices", value);
            let error = load_text(&text).expect_err("超出 1..=25 的候选上限应当被拒绝");
            assert!(
                error.contains("runtime.max_autocomplete_choices"),
                "报错应指出字段名: {error}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_base_dir_is_reported() {
        let (dir, text) = staged_example("missing-base");
        // 使用绝对路径，以测试目录存在性而非路径格式。
        let missing = dir.join("not-there").to_string_lossy().replace('\\', "/");
        let text = set_field(&text, "base_dir", &format!("\"{missing}\""));

        let err = load_text(&text).expect_err("目录不存在应当报错");

        assert!(err.contains("paths.base_dir"), "报错应指出字段: {err}");
        assert!(
            err.contains("does not exist or is not a directory"),
            "报错应说明原因: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsafe_java_opts_is_reported() {
        let (dir, text) = staged_example("java-opts");
        let text = set_field(&text, "java_opts", "\"-Xmx128m; rm -rf /\"");

        let err = load_text(&text).expect_err("危险字符应当报错");

        assert!(err.contains("runtime.java_opts"), "报错应指出字段: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shell_unsafe_runtime_names_are_reported() {
        for (field, value) in [
            ("token_file", "\"refresh';$HOME;'\""),
            ("bot_logs_dir", "\"logs';$HOME;'\""),
        ] {
            let (dir, text) = staged_example(&format!("shell-unsafe-{field}"));
            let text = set_field(&text, field, value);

            let err = load_text(&text).expect_err("shell 特殊字符应当报错");

            assert!(
                err.contains(&format!("runtime.{field}")),
                "报错应指出字段 {field}: {err}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn overlapping_command_echoes_are_reported() {
        let (dir, text) = staged_example("echo-overlap");
        let text = set_field(&text, "unpause_echo", "\"Fishing paused\"");

        let err = load_text(&text).expect_err("两条回显相同应当报错");

        assert!(
            err.contains("command_echo.unpause_echo"),
            "报错应指出字段: {err}"
        );
        assert!(
            err.contains("must not be equal to or overlap"),
            "报错应说明原因: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsafe_screen_prefix_is_reported() {
        let (dir, text) = staged_example("screen-prefix");
        let text = set_field(&text, "screen_prefix", "\"fb _\"");

        let err = load_text(&text).expect_err("空白字符应当报错");

        assert!(
            err.contains("runtime.screen_prefix"),
            "报错应指出字段: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn zero_runtime_and_timing_values_are_reported() {
        let (dir, text) = staged_example("zero-values");
        for (field, expected) in [
            ("max_reply_log_lines", "runtime.max_reply_log_lines"),
            ("timeout", "ping.timeout"),
            ("watchdog_window", "timing.watchdog_window"),
            ("check_grace", "timing.check_grace"),
        ] {
            let modified = set_field(&text, field, "0");
            let error = load_text(&modified).expect_err("zero value must fail");
            assert!(error.contains(expected), "{expected}: {error}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_language_is_reported() {
        let (dir, text) = staged_example("unknown-language");
        // JAR 支持德语，但本项目未实现对应文案。
        let text = set_field(&text, "language", "\"GERMAN\"");

        let err = load_text(&text).expect_err("不认识的语言应当报错");

        assert!(err.contains("i18n.language"), "报错应指出字段: {err}");
        assert!(err.contains("ENGLISH"), "报错应列出可选值: {err}");
        assert!(
            err.contains("CHINESE_SIMPLIFIED"),
            "报错应列出可选值: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn zero_start_batch_timing_is_reported() {
        for (field, tag) in [
            ("max_batch_size", "zero-start-batch-size"),
            ("bot_login_interval", "zero-start-interval"),
            ("max_pending_requests", "zero-start-queue-capacity"),
        ] {
            let (dir, text) = staged_example(tag);
            let text = set_field(&text, field, "0");

            let err = load_text(&text).expect_err("启动队列设置为 0 应当报错");

            assert!(
                err.contains(&format!("startup_queue.{field}")),
                "报错应指出字段 {field}: {err}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// 管理员名单键必须是 Discord 用户 ID。
    #[test]
    fn admin_key_must_be_user_id() {
        let admins = admins_from("\"abc\" = \"名字\"");

        let err = validate_admins(&admins).expect_err("非数字键应当报错");

        assert!(
            err.contains("is not a Discord user ID"),
            "报错应说明键要是 Discord 用户 ID: {err}"
        );
    }

    /// 管理员名称不得为空；空白名称已在解析时去除空格。
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

    /// 通过 TOML 构造管理员名单；`Admins` 字段私有，该路径更接近实际加载流程。
    fn admins_from(text: &str) -> Admins {
        toml::from_str(text).expect("测试用的管理员名单应当能解析")
    }
}
