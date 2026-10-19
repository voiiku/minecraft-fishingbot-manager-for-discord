//! 配置加载完成前使用的英文提示，以及进程入口调用顺序错误时的内部不变量说明。

use super::Language;

/// 配置还没装载就取用（`config::Config::get()`）：进程入口的调用顺序写错了，不是部署者的配置问题。
///
/// 内部不变量消息，只有代码写错才会出现；那时没有语言可选，固定英文。
pub const CONFIG_NOT_LOADED: &str = "config not loaded yet: call minecraft_fishingbot_manager_for_discord::config::Config::get_or_init() \
     at the process entry point before using any other API";

/// 语言还没安装就取文案：进程入口没有走 `config::Config::get_or_init()`，属于调用顺序错误。
pub const TEXT_NOT_INSTALLED: &str = "text language not installed yet: config::Config::get_or_init() installs it before any text is requested";

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

/// 解析 `[paths]` 时的路径取值非法；此时语言尚未安装，固定英文。
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
