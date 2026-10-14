//! 配置校验错误中的要求类型。

/// 配置字段的校验要求。
#[derive(Debug, Clone, Copy)]
pub enum ConfigRequirement<'a> {
    /// 必填项不能为空。
    NonEmpty,
    /// 会被当成"单个名字"用的取值（会话名前缀、Token 文件名、日志目录名）。
    PlainName,
    /// 端口号范围。
    PortRange,
    /// 通用：必须大于 0。
    GreaterThanZero,
    /// Discord token 不能为空。
    DiscordTokenEmpty,
    /// 取值里不能有单引号（会破坏 `grep -E` 的单引号参数）。
    NoSingleQuote,
    /// `runtime.java_opts` 不能含 shell 特殊字符。
    ShellSafe,
    /// `paths.base_dir` 必须是存在的目录。
    BaseDirIsDir,
    /// `paths.jar` 必须是存在的文件。
    JarIsFile,
    /// `paths.base_dir` 必须是绝对路径。
    BaseDirAbsolute,
    /// 其余路径必须是相对路径。
    PathRelative,
    /// `[admins]` 的键必须是 Discord 用户 ID。
    AdminsKeyUserId { user_id: &'a str },
    /// `[admins]` 里某个用户的名字不能为空。
    AdminsNameEmpty { user_id: &'a str },
    /// `runtime.max_reply_chars` 必须大于 0。
    ReplyCharsPositive,
    /// `runtime.max_autocomplete_choices` 必须在 1..=25 之间（Discord 对自动候选条数的硬上限）。
    AutocompleteChoicesInRange,
    /// 网关看门狗的时序项（`timing.watchdog_window_ms` / `probe_interval_ms` /
    /// `first_handshake_timeout_ms`）必须大于 0。
    WatchdogTimingPositive,
    /// `timing.probe_interval_ms` 不能大于 `timing.watchdog_window_ms`。
    ProbeIntervalWithinWindow,
    /// `command_echo.pause_echo` 与 `command_echo.unpause_echo` 不能相同或互相包含。
    EchoTextsOverlap,
    /// 取值必须是给定清单里的一个（`[fishingbot]` 的那两个播报开关，上游是枚举）。
    OneOf(&'a [&'a str]),
    /// `fishingbot.default_protocol` 必须写 `AUTOMATIC` 或版本号（形如 `1.20.4`）。
    McVersionOrAutomatic,
    /// `[i18n] language` 写了个不认识的取值。
    ///
    /// 语言名无效；错误文案由 [`crate::text::Language::from_name`] 生成。
    LanguageUnknown { message: &'a str },
}
