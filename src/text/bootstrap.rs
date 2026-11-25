//! 首次上线前的英文错误、配置引导提示与内部不变量说明。

use std::fmt::Display;

use super::Language;

/// 配置字段的校验要求。
#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfigRequirement<'a> {
    NonEmpty,
    PlainName,
    PortRange,
    GreaterThanZero,
    NonNegativeDuration,
    InputPulseCommand { reason: &'a str },
    DiscordTokenEmpty,
    NoSingleQuote,
    ShellSafe,
    BaseDirIsDir,
    JarIsFile,
    AdminsKeyUserId { user_id: &'a str },
    AdminsNameEmpty { user_id: &'a str },
    ReplyCharsPositive,
    AutocompleteChoicesInRange { max: usize },
    LogFilenameFormat,
    TimeFormat,
    WatchdogTimingPositive,
    OneOf(&'a [&'a str]),
    McVersionOrAutomatic,
}

pub(crate) fn config_field_invalid(
    field: impl Display,
    requirement: ConfigRequirement<'_>,
) -> String {
    format!("config item {field} {}", requirement_text(requirement))
}

pub(crate) fn config_field_value_invalid(
    field: impl Display,
    value: impl Display,
    requirement: ConfigRequirement<'_>,
) -> String {
    format!(
        "config item {field} = {value} {}",
        requirement_text(requirement)
    )
}

fn requirement_text(requirement: ConfigRequirement<'_>) -> String {
    match requirement {
        ConfigRequirement::NonEmpty => "must not be empty".to_string(),
        ConfigRequirement::PlainName => {
            "must not contain whitespace or slashes, and must not be . or ..".to_string()
        }
        ConfigRequirement::PortRange => "must be a port number between 1 and 65535".to_string(),
        ConfigRequirement::GreaterThanZero => "must be greater than 0".to_string(),
        ConfigRequirement::NonNegativeDuration => {
            "must be a finite, non-negative number of seconds within the supported range"
                .to_string()
        }
        ConfigRequirement::InputPulseCommand { reason } => {
            format!("must be input-pulse input supported in screen mode: {reason}")
        }
        ConfigRequirement::DiscordTokenEmpty => {
            "must not be empty (Developer Portal -> your app -> Bot -> Reset Token)".to_string()
        }
        ConfigRequirement::NoSingleQuote => "must not contain a single quote".to_string(),
        ConfigRequirement::ShellSafe => {
            "must not contain quotes, backticks, $, ;, &, |, <, > or newlines".to_string()
        }
        ConfigRequirement::BaseDirIsDir => "does not exist or is not a directory".to_string(),
        ConfigRequirement::JarIsFile => {
            "does not exist or is not a file (relative paths are resolved against base_dir)"
                .to_string()
        }
        ConfigRequirement::AdminsKeyUserId { user_id } => format!(
            "the key {user_id:?} is not a Discord user ID: write it as plain digits, e.g. \
             \"123456789012345678\" = \"name\""
        ),
        ConfigRequirement::AdminsNameEmpty { user_id } => {
            format!("the name of user {user_id} must not be empty")
        }
        ConfigRequirement::ReplyCharsPositive => "must be greater than 0".to_string(),
        ConfigRequirement::AutocompleteChoicesInRange { max } => {
            format!("must be between 1 and {max} (Discord's autocomplete limit)")
        }
        ConfigRequirement::LogFilenameFormat => {
            "must format a complete local date and time as one file name".to_string()
        }
        ConfigRequirement::TimeFormat => "must be a valid time format".to_string(),
        ConfigRequirement::WatchdogTimingPositive => "must be greater than 0".to_string(),
        ConfigRequirement::OneOf(allowed) => format!("must be one of: {}", allowed.join(", ")),
        ConfigRequirement::McVersionOrAutomatic => {
            "must be a version string like 1.20.4, or AUTOMATIC to detect it".to_string()
        }
    }
}

pub(crate) fn fishingbot_config_remove_failed(path: &str, reason: &str) -> String {
    format!("failed to remove the old game-side config {path}: {reason}")
}

pub(crate) fn fishingbot_config_write_failed(path: &str, reason: &str) -> String {
    format!("failed to write the game-side config {path}: {reason}")
}

pub(crate) fn fishingbot_config_serialize_failed(reason: &str) -> String {
    format!("failed to serialize the game-side config: {reason}")
}

pub(crate) fn fishingbot_config_template_invalid(reason: &str) -> String {
    format!("the bundled game-side config template is unusable: {reason}")
}

pub(crate) fn fishingbot_config_template_path_missing(path: &str) -> String {
    format!("tried to find {path} in the game-side config template, but that path does not exist")
}

/// 配置还没装载就取用（`config::Config::get()`）：进程入口的调用顺序写错了，不是部署者的配置问题。
///
/// 内部不变量消息，只有代码写错才会出现；那时没有语言可选，固定英文。
pub const CONFIG_NOT_LOADED: &str = "config not loaded yet: call minecraft_fishingbot_manager_for_discord::config::Config::load() \
     at the process entry point before using any other API";

/// `i18n.language` 的取值非法：此时还没有语言实现，报错固定英文（格式与其它配置项一致）。
pub fn language_invalid(message: &str) -> String {
    format!("config item i18n.language {message}")
}

/// 首次运行：模板已生成，提示改完再启动。
pub fn config_template_created(path: &str) -> String {
    format!(
        "No configuration file was found. A template was created at {path}.\n\
         Edit the template and restart the program."
    )
}

/// 配置文件已存在：提示一句找到的那份路径，随后读取并校验。
pub fn config_file_found(path: &str) -> String {
    format!("Found the existing configuration file {path}. Reading and validating it.")
}

/// 读配置文件失败。
pub fn config_read_failed(path: &str, reason: &str) -> String {
    format!("Failed to read the configuration file {path}: {reason}")
}

/// 解析配置文件失败（字段缺失、类型不符或名字拼错都在这里报出）。
///
/// `reason` 直接来自 toml/serde，可能自带换行（语法错误会带出错片段），所以只去掉末尾的多余空行。
pub fn config_parse_failed(path: &str, reason: &str) -> String {
    format!(
        "Failed to parse the configuration file {path}:\n{}",
        reason.trim_end()
    )
}

/// 解析 `[paths]` 时的路径取值非法；配置尚未发布，固定英文。
pub fn config_path_value_invalid(field: &str, value: &str, requirement: &str) -> String {
    format!("Invalid config item {field}: `{value}`; {requirement}.")
}

/// 生成配置模板失败：所在目录不存在。
pub fn config_template_no_parent_dir(path: &str) -> String {
    format!(
        "The directory for the configuration file {path} does not exist, so the template cannot be created. Ensure that the executable's directory exists and is writable."
    )
}

/// 写配置模板失败。
pub fn config_template_write_failed(path: &str, reason: &str) -> String {
    format!("Failed to create the configuration template {path}: {reason}")
}

/// 取不到可执行文件路径（配置文件与程序日志都靠它定位）。
pub fn exe_path_failed(reason: &str) -> String {
    format!(
        "Could not determine the executable path, so the configuration and log directories cannot be located: {reason}"
    )
}

/// 可执行文件没有上级目录。
pub fn exe_path_no_parent(path: &str) -> String {
    format!(
        "The executable path {path} has no parent directory, so the configuration and log directories cannot be located."
    )
}

/// `[i18n] language` 拒绝未知取值（可选值由 [`Language::supported_names`] 给出，与语言无关）。
pub fn language_unknown(language_name: &str) -> String {
    format!(
        "'{language_name}' is not a supported language. Expected one of: {}.",
        Language::supported_names().join(" / ")
    )
}
