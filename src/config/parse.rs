//! 解析 TOML 配置为内存中的结构体，去掉首尾空格并展开路径，取值校验由 [`super::validate`] 负责。

use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

use crate::text;

use super::schema::Paths;
use super::{CONFIG_FILE_NAME, Config};

impl Config {
    /// 解析配置文本并转换为运行时配置。
    /// 先解析成值树、递归去掉两头的空白，再按目标类型自动映射到 `Config`，这样加字段不必补注解。
    pub(super) fn from_toml_text(text: &str) -> Result<Self, String> {
        let mut value: toml::Value =
            toml::from_str(text).map_err(|error| Self::parse_failed(&error.to_string()))?;
        trim_string_values(&mut value);

        // 把值树按目标类型自动映射到 `Config`结构体，失败文本直接返回给用户。
        value
            .try_into()
            .map_err(|error| Self::parse_failed(&error.to_string()))
    }

    /// 解析失败的统一文案：文件名 + TOML/Serde 自己的文本（含表名与字段名）。
    fn parse_failed(reason: &str) -> String {
        text::bootstrap::config_parse_failed(CONFIG_FILE_NAME, reason)
    }
}

impl<'de> Deserialize<'de> for Paths {
    /// 读文件里的 `[paths]` 表：`base_dir` 必须绝对，另外两个键必须是相对 `base_dir` 的名字；
    /// 展开成运行时用的绝对路径（`bots_dir` 由代码派生）。
    ///
    /// 因为配置文件里只有 `base_dir` 是绝对路径，剩下的都是相对路径，为了避免每次调用时现拼，
    /// 所以 [`Config`] 里存的是展开后的绝对路径。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// 文件里的 `[paths]` 表。
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Table {
            base_dir: PathBuf,
            jar: PathBuf,
            fishingbot_config_file_name: PathBuf,
        }

        let table = Table::deserialize(deserializer)?;

        if !table.base_dir.is_absolute() {
            return Err(serde::de::Error::custom(text::get().log_text(
                text::LogText::ConfigFieldValueInvalid {
                    field: "paths.base_dir",
                    value: &table.base_dir.display().to_string(),
                    requirement: text::ConfigRequirement::BaseDirAbsolute,
                },
            )));
        }

        let base_dir = table.base_dir;
        let bots_dir = base_dir.join("bots");
        let jar =
            base_dir.join(require_relative("jar", table.jar).map_err(serde::de::Error::custom)?);
        let fishingbot_config = base_dir.join(
            require_relative(
                "fishingbot_config_file_name",
                table.fishingbot_config_file_name,
            )
            .map_err(serde::de::Error::custom)?,
        );

        Ok(Paths {
            base_dir,
            bots_dir,
            jar,
            fishingbot_config,
        })
    }
}

/// 确认路径是相对 `base_dir` 的名字（不是绝对路径）。
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 示例配置可解析，路径字段按配置规则展开。
    #[test]
    fn example_config_parses() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let config = Config::from_toml_text(text).expect("示例配置应当能解析");

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

    /// 解析错误必须包含配置文件名与出错字段名。
    #[test]
    fn parse_error_mentions_field() {
        let text = "[ping]\nhost = \"example.com\"\nport = \"不是端口\"\n";
        let err = Config::from_toml_text(text).unwrap_err();

        assert!(
            err.contains("minecraft-fishingbot-manager-for-discord.conf.toml"),
            "报错应指出文件名: {err}"
        );
        assert!(err.contains("port"), "报错应指出字段名: {err}");
    }

    /// 未知字段（例如升级后残留的旧字段）必须报告字段名与所属表。
    #[test]
    fn unknown_field_is_reported_with_its_table() {
        let text = include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml")
            .replace("[discord]", "[discord]\nguild_id = \"0\"");

        let err = Config::from_toml_text(&text).unwrap_err();

        assert!(err.contains("guild_id"), "{err}");
        assert!(err.contains("in `discord`"), "报错应指出所在表: {err}");
        assert!(err.ends_with('`'), "报错末尾不应留下空行: {err:?}");
    }

    /// 解析阶段递归去除字符串两端空白，包括表、数组与数组中的表。
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

    /// 实际配置解析同样去除值两端空白，调用方无需再次 `trim()`。
    #[test]
    fn config_values_come_out_trimmed() {
        let example =
            include_str!("../../minecraft-fishingbot-manager-for-discord.conf.example.toml");
        let text = example.replace(
            "token = \"Paste Your Discord Bot Token Here\"",
            "token = \"  Paste Your Discord Bot Token Here  \"",
        );
        assert_ne!(
            text, example,
            "示例配置中的 token 行已变更，需要同步更新测试"
        );

        let config = Config::from_toml_text(&text).expect("示例配置应当能解析");

        assert_eq!(config.discord.token, "Paste Your Discord Bot Token Here");
    }
}
