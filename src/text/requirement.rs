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
    /// 非负、有限且可表示的秒数。
    NonNegativeDuration,
    /// input-pulse 输入动作解析或 screen 模式校验失败。
    InputPulseCommand { reason: &'a str },
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
    /// 网关看门狗的判定窗口（`timing.watchdog_window`）必须大于 0。
    WatchdogTimingPositive,
    /// `command_echo.pause_echo` 与 `command_echo.unpause_echo` 不能相同或互相包含。
    EchoTextsOverlap,
    /// 取值必须是给定清单里的一个（`[fishingbot]` 的那两个播报开关，上游是枚举）。
    OneOf(&'a [&'a str]),
    /// `fishingbot.default_protocol` 必须写 `AUTOMATIC` 或版本号（形如 `1.20.4`）。
    McVersionOrAutomatic,
}
