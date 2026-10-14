//! 解析 TOML 配置、去掉字符串值两头的空白并展开路径；取值校验由 [`super::validate`] 负责。

use std::path::{Path, PathBuf};

use crate::text;

use super::schema::{Config, FileConfig, FilePaths, Paths};

/// 解析配置文本并转换为运行时结构。
pub(super) fn parse(text: &str, path: &Path) -> Result<Config, String> {
    let file = parse_file(text, path)?;

    file.into_config()
}

/// 把 TOML 文本解析成 [`FileConfig`]。
/// 先解析成值树、递归去掉两头的空白，再按目标类型映射到结构体。这样加字段的时不必补注解。
fn parse_file(text: &str, path: &Path) -> Result<FileConfig, String> {
    let mut value: toml::Value =
        toml::from_str(text).map_err(|error| config_parse_failed(path, &error.to_string()))?;
    trim_string_values(&mut value);

    value
        .try_into()
        .map_err(|error| config_parse_failed(path, &mapping_failed_reason(text, &error)))
}

/// 值树映射失败的详情。
fn mapping_failed_reason(text: &str, error: &toml::de::Error) -> String {
    toml::from_str::<FileConfig>(text)
        .err()
        .map_or_else(|| error.to_string(), |spanned| spanned.to_string())
}

/// 递归去掉值树里所有字符串两头的空白（表、数组、数组里的表都算）。
///
/// 真要保留两头的空白就写转义（TOML 里 `"\u0020"`）：正常写法都是手滑多敲的空格或换行缩进。
fn trim_string_values(value: &mut toml::Value) {
    match value {
        toml::Value::String(raw) => *raw = raw.trim().to_string(),
        toml::Value::Array(items) => items.iter_mut().for_each(trim_string_values),
        toml::Value::Table(table) => table
            .iter_mut()
            .for_each(|(_, item)| trim_string_values(item)),
        _ => {}
    }
}

/// 解析失败的统一文案：文件名 + TOML/Serde 自己的文本（含字段名）。
fn config_parse_failed(path: &Path, reason: &str) -> String {
    text::bootstrap::config_parse_failed(&path.display().to_string(), reason)
}

impl FileConfig {
    /// 转换为运行时配置并展开路径。
    pub(super) fn into_config(self) -> Result<Config, String> {
        Ok(Config {
            discord: self.discord,
            paths: self.paths.into_paths()?,
            fishingbot: self.fishingbot,
            admins: self.admins,
            keywords: self.keywords,
            command_echo: self.command_echo,
            i18n: self.i18n,
            runtime: self.runtime,
            startup_queue: self.startup_queue,
            timing: self.timing,
            raw_game_logs: self.raw_game_logs,
            ping: self.ping,
            log: self.log,
            text: self.text,
        })
    }
}

impl FilePaths {
    /// 校验路径并展开为运行时路径。
    pub(super) fn into_paths(self) -> Result<Paths, String> {
        if !self.base_dir.is_absolute() {
            return Err(
                text::get().log_text(text::LogText::ConfigFieldValueInvalid {
                    field: "paths.base_dir",
                    value: &self.base_dir.display().to_string(),
                    requirement: text::ConfigRequirement::BaseDirAbsolute,
                }),
            );
        }

        let base_dir = self.base_dir;
        let bots_dir = base_dir.join("bots");
        let jar = base_dir.join(require_relative("jar", self.jar)?);
        let fishingbot_config = base_dir.join(require_relative(
            "fishingbot_config_file_name",
            self.fishingbot_config_file_name,
        )?);

        Ok(Paths {
            base_dir,
            bots_dir,
            jar,
            fishingbot_config,
        })
    }
}

/// 确认路径为相对路径。
fn require_relative(field: &str, path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Err(
            text::get().log_text(text::LogText::ConfigFieldValueInvalid {
                field: &format!("paths.{field}"),
                value: &path.display().to_string(),
                requirement: text::ConfigRequirement::PathRelative,
            }),
        );
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 示例配置可解析，路径按规则展开。
    #[test]
    fn example_config_parses() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let path = Path::new("/srv/fishbot/minecraft-fishingbot-manager-for-discord.conf.toml");
        let config = parse(text, path).expect("示例配置应当能解析");

        assert!(!config.ping.default_ping_host.trim().is_empty());
        assert!(config.ping.default_ping_port > 0);
        assert!(config.ping.timeout_ms > 0);
        assert!(!config.keywords.filter_regex.trim().is_empty());
        assert!(!config.text.help.trim().is_empty());

        // 路径应展开为运行时路径。
        assert_eq!(config.paths.bots_dir, config.paths.base_dir.join("bots"));
        assert!(!config.paths.jar.as_os_str().is_empty());
        assert!(!config.paths.fishingbot_config.as_os_str().is_empty());
    }

    /// 解析错误包含文件名与字段名。
    #[test]
    fn parse_error_mentions_field() {
        let text = "[ping]\nhost = \"example.com\"\nport = \"不是端口\"\n";
        let err = parse(
            text,
            Path::new("minecraft-fishingbot-manager-for-discord.conf.toml"),
        )
        .unwrap_err();

        assert!(
            err.contains("minecraft-fishingbot-manager-for-discord.conf.toml"),
            "报错应指出文件名: {err}"
        );
        assert!(err.contains("port"), "报错应指出字段名: {err}");
    }

    /// 拼错字段名（例如升级后配置里留下的旧字段）会报出字段名与所在行，而不是一句"未知字段"。
    #[test]
    fn unknown_field_is_reported_with_its_line() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml")
            .replace("[discord]", "[discord]\nguild_id = \"0\"");

        let err = parse(
            &text,
            Path::new("minecraft-fishingbot-manager-for-discord.conf.toml"),
        )
        .unwrap_err();

        assert!(err.contains("guild_id"), "{err}");
        assert!(err.contains("line"), "报错应指出行号: {err}");
        assert!(err.ends_with('`'), "报错末尾不应留下空行: {err:?}");
    }

    /// 值两头的空白在解析时就抹掉：表、数组、嵌在数组里的表都算。
    #[test]
    fn string_values_are_trimmed() {
        let mut value: toml::Value = toml::from_str(
            "top = \"  a  \"\nlist = [\" b \", { deep = \"  c  \" }]\n[table]\nnested = \" d \"\n",
        )
        .expect("裸 TOML 值树应当能解析");

        trim_string_values(&mut value);

        assert_eq!(value["top"].as_str(), Some("a"));
        assert_eq!(value["list"][0].as_str(), Some("b"));
        assert_eq!(value["list"][1]["deep"].as_str(), Some("c"));
        assert_eq!(value["table"]["nested"].as_str(), Some("d"));
    }

    /// 真配置里也一样：值两头多敲的空白不会带进代码，所以读配置的人不必自己 `trim()`。
    #[test]
    fn config_values_come_out_trimmed() {
        let example =
            include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let text = example.replace(
            "token = \"Paste Your Discord Bot Token Here\"",
            "token = \"  Paste Your Discord Bot Token Here  \"",
        );
        assert_ne!(text, example, "示例配置里那行 token 变了，这条用例要跟着改");

        let config = parse(
            &text,
            Path::new("minecraft-fishingbot-manager-for-discord.conf.toml"),
        )
        .expect("示例配置应当能解析");

        assert_eq!(config.discord.token, "Paste Your Discord Bot Token Here");
    }
}
