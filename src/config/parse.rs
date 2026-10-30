//! 解析 TOML 配置为内存中的结构体，去掉首尾空格并展开路径，取值校验由 [`super::validate`] 负责。

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Deserializer};

use crate::text;

use super::schema::Paths;
use super::{CONFIG_FILE_NAME, Config};

/// 将配置中的秒数转换成内部毫秒值，不接受小于一毫秒的精度。
pub(super) fn seconds_to_millis<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Seconds {
        Whole(u64),
        Fraction(f64),
    }

    match Seconds::deserialize(deserializer)? {
        Seconds::Whole(seconds) => seconds
            .checked_mul(1000)
            .ok_or_else(|| serde::de::Error::custom("seconds exceed the supported range")),
        Seconds::Fraction(seconds) => {
            let duration = Duration::try_from_secs_f64(seconds).map_err(|_| {
                serde::de::Error::custom("seconds must be finite, non-negative and in range")
            })?;
            let milliseconds =
                duration.as_millis() + u128::from(duration.subsec_nanos() % 1_000_000 >= 500_000);
            if seconds != milliseconds as f64 / 1000.0 {
                return Err(serde::de::Error::custom(
                    "seconds must be an exact multiple of 0.001",
                ));
            }
            u64::try_from(milliseconds)
                .map_err(|_| serde::de::Error::custom("seconds exceed the supported range"))
        }
    }
}

impl Config {
    /// 解析配置并递归去除字符串首尾空白。
    pub(super) fn from_toml_text(text: &str) -> Result<Self, String> {
        let mut value: toml::Value =
            toml::from_str(text).map_err(|error| Self::parse_failed(&error.to_string()))?;
        trim_string_values(&mut value);

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
    /// 解析 `[paths]` 表：`base_dir` 必须为绝对路径，其余两个文件名必须为相对路径。
    /// 解析后展开为运行时绝对路径；`bots_dir` 由代码派生。
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
            return Err(serde::de::Error::custom(
                text::bootstrap::config_path_value_invalid(
                    "paths.base_dir",
                    &table.base_dir.display().to_string(),
                    "must be an absolute path",
                ),
            ));
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
        return Err(text::bootstrap::config_path_value_invalid(
            &format!("paths.{field}"),
            &path.display().to_string(),
            "must be a relative path",
        ));
    }

    Ok(path)
}

/// 递归去除所有字符串的首尾空白；TOML 转义也会在此之前被解析。
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

    #[test]
    fn seconds_to_millis_preserves_every_millisecond_in_ten_seconds() {
        #[derive(Deserialize)]
        struct TestTime {
            #[serde(deserialize_with = "seconds_to_millis")]
            value: u64,
        }

        for milliseconds in 0..10_000 {
            let seconds = format!("{}.{:03}", milliseconds / 1000, milliseconds % 1000);
            let input = format!("value = {seconds}");
            let config: TestTime = toml::from_str(&input).expect("exact milliseconds must parse");
            assert_eq!(config.value, milliseconds, "{input}");
        }
    }

    #[test]
    fn seconds_to_millis_checks_range_and_precision() {
        #[derive(Deserialize)]
        struct TestTime {
            #[serde(deserialize_with = "seconds_to_millis")]
            value: u64,
        }

        for (seconds, expected) in [
            ("0", 0),
            ("20", 20_000),
            ("0.001", 1),
            ("0.3", 300),
            ("8388608.001", 8_388_608_001),
            ("123456789012.001", 123_456_789_012_001),
            ("18446744073709551", 18_446_744_073_709_551_000),
        ] {
            let config: TestTime =
                toml::from_str(&format!("value = {seconds}")).expect("valid seconds");
            assert_eq!(config.value, expected, "{seconds}");
        }
        for seconds in [
            "-1",
            "-0.001",
            "inf",
            "nan",
            "0.0005",
            "0.0000000001",
            "1e100",
            "18446744073709552",
        ] {
            assert!(
                toml::from_str::<TestTime>(&format!("value = {seconds}")).is_err(),
                "{seconds} must be rejected"
            );
        }
    }

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
            "top = \"  a  \"\nescaped = \"\\u0020e\\u0020\"\nlist = [\" b \", { deep = \"  c  \" }]\n[table]\nnested = \" d \"\n",
        )
        .expect("裸 TOML 值树应当能解析");

        trim_string_values(&mut value);

        assert_eq!(value["top"].as_str(), Some("a"));
        assert_eq!(value["escaped"].as_str(), Some("e"));
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
