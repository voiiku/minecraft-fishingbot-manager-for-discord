//! 覆盖生成游戏侧配置（FishingBot 的 `conf.json`）
//!
//! 生成的是完整配置，在仓库的模板[`FISHINGBOT_CONFIG_TEMPLATE`]基础上覆盖 `[fishingbot]` 中的配置，其余键不变。
//! FishingBot.jar 内置配置补全，但会在日志中发出警告，所以自动补全只是兜底方案。

use crate::config::{self, FishingBotConfig};
use crate::text::{self, Language};
use serde_json::{Value, json};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// 随仓库发布的游戏侧配置模板（`fishingbot-config.example.json`，完整的 FishingBot 配置）。
/// 当新版本更改配置结构时，此模板应该随之更新（或依靠jar的自动配置补全兜底）。
const FISHINGBOT_CONFIG_TEMPLATE: &str = include_str!("../../fishingbot-config.example.json");

/// 按配置生成游戏侧配置（只在进程启动时调用一次）。
/// 失败时调用者应当按永久错误处理（退出码 1）。
pub fn regenerate_fishingbot_config() -> Result<(), String> {
    let settings = config::Config::get();

    write_fishingbot_config(
        &settings.paths.fishingbot_config,
        &settings.fishingbot,
        &settings.i18n.language,
    )
}

/// 生成要覆盖配置模板的键与取值。
/// 生成流程使用它覆盖模板
fn managed_values(fishingbot: &FishingBotConfig, language: Language) -> Vec<(&'static str, Value)> {
    let start_text = &fishingbot.start_text;

    vec![
        ("server.ip", json!(&fishingbot.server_ip)),
        ("server.port", json!(fishingbot.server_port)),
        (
            "server.default-protocol",
            json!(&fishingbot.default_protocol),
        ),
        ("auto.auto-reconnect", json!(fishingbot.auto_reconnect)),
        (
            "auto.auto-reconnect-time",
            json!(fishingbot.auto_reconnect_time),
        ),
        (
            "announces.announce-type-chat",
            json!(&fishingbot.announce_type_chat),
        ),
        (
            "announces.announce-type-console",
            json!(&fishingbot.announce_type_console),
        ),
        ("start-text.enabled", json!(!start_text.is_empty())),
        ("start-text.text", json!(start_text)),
        ("misc.language", json!(language.name())),
    ]
}

/// 生成游戏侧配置文件：先删除已经存在的，再写入新的。
/// 路径与取值全由调用方提供（实际上可以自行读取而不用提供，只是方便测试）。
fn write_fishingbot_config(
    path: &Path,
    fishingbot: &FishingBotConfig,
    language_name: &str,
) -> Result<(), String> {
    remove_old_fishingbot_config(path)?;

    let contents = fishingbot_config_json(fishingbot, language_name)?;
    fs::write(path, contents).map_err(|e| {
        text::get().log_text(text::LogText::FishingbotConfigWriteFailed {
            path: &path.display().to_string(),
            reason: &e.to_string(),
        })
    })?;

    log::info!(
        "{}",
        text::get().log_text(text::LogText::FishingbotConfigWritten {
            path: &path.display().to_string(),
        })
    );

    Ok(())
}

/// 删除旧的游戏侧配置。
fn remove_old_fishingbot_config(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => {
            log::info!(
                "{}",
                text::get().log_text(text::LogText::FishingbotConfigRemoved {
                    path: &path.display().to_string(),
                })
            );

            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(
            text::get().log_text(text::LogText::FishingbotConfigRemoveFailed {
                path: &path.display().to_string(),
                reason: &e.to_string(),
            }),
        ),
    }
}

/// 生成即将写进 `conf.json` 的 JSON 文本，基于模板，只覆盖 `[fishingbot]` 中那些键（由managed_values生成）。
fn fishingbot_config_json(
    fishingbot: &FishingBotConfig,
    language_name: &str,
) -> Result<String, String> {
    let language = Language::from_name(language_name).unwrap_or_default();
    // 将模板文本转化为可解析的结构体
    let mut fishingbot_config: Value =
        serde_json::from_str(FISHINGBOT_CONFIG_TEMPLATE).map_err(|e| {
            text::get().log_text(text::LogText::FishingbotConfigTemplateInvalid {
                reason: &e.to_string(),
            })
        })?;
    // 按字段写入值
    for (path, value) in managed_values(fishingbot, language) {
        set_json_path(&mut fishingbot_config, path, value)?;
    }
    // 将修改完的文本返回
    serde_json::to_string_pretty(&fishingbot_config).map_err(|e| {
        text::get().log_text(text::LogText::FishingbotConfigSerializeFailed {
            reason: &e.to_string(),
        })
    })
}

/// 按点分路径向 JSON 中写值。
/// 路径上的每一层都必须已经存在（模板是完整的，我们只改值、不新增结构）
/// 不存在就报错，（说明模板与代码不匹配，可能是上游改了键名）。
fn set_json_path(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let Some((parent_path, key)) = path.rsplit_once('.') else {
        return Err(template_mismatch(path));
    };

    let mut cursor = root;
    for segment in parent_path.split('.') {
        cursor = cursor
            .get_mut(segment)
            .ok_or_else(|| template_mismatch(path))?;
    }

    let object = cursor
        .as_object_mut()
        .ok_or_else(|| template_mismatch(path))?;
    object.insert(key.to_string(), value);

    Ok(())
}

/// 模板里找不到某个托管路径时的报错（点名是哪条路径）。
fn template_mismatch(path: &str) -> String {
    text::get().log_text(text::LogText::FishingbotConfigTemplatePathMissing { path })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::temp_test_dir;

    /// 测试用的托管配置：值都是固定字面量，与 `[fishingbot]` 的解析无关，只用来断言生成结果。
    fn sample_fishingbot_config() -> FishingBotConfig {
        FishingBotConfig {
            server_ip: "127.0.0.1".to_string(),
            server_port: 25565,
            default_protocol: "AUTOMATIC".to_string(),
            auto_reconnect: true,
            auto_reconnect_time: 30,
            start_text: vec!["/home fishing".to_string()],
            announce_type_console: "NONE".to_string(),
            announce_type_chat: "NONE".to_string(),
        }
    }

    /// 模板是完整配置，且每个托管路径都在里面。
    ///
    /// 上游改了键名或删了键时这条先红 —— 那时把新 jar 生成的默认 `conf.json` 覆盖成模板，
    /// 再同步 `managed_values` 与生成代码。
    #[test]
    fn template_holds_every_managed_path() {
        let template = template_value();

        for path in managed_paths() {
            assert!(
                value_at(&template, path).is_some(),
                "模板里没有 {path}：上游改了配置结构？换上新模板并同步生成代码"
            );
        }
    }

    /// 生成出来的文件 = 模板 + 我们那几个键：除了托管路径，其余键必须与模板逐值相同
    ///（这就是"写出来的是完整配置、不依赖 jar 补齐"的保证）。
    #[test]
    fn generated_config_differs_from_template_only_in_managed_paths() {
        let template = template_value();
        let generated = serde_json::from_str(
            &fishingbot_config_json(&sample_fishingbot_config(), "ENGLISH").unwrap(),
        )
        .expect("生成的是合法 JSON");

        for (path, template_value) in leaf_paths(&template) {
            if managed_paths().contains(&path.as_str()) {
                continue;
            }

            assert_eq!(
                value_at(&generated, &path),
                Some(&template_value),
                "{path} 不在托管清单里，却与模板不一样"
            );
        }

        // 反过来：托管路径的取值来自配置表（这里用测试字面量，见 `sample_fishingbot_config`）。
        assert_eq!(value_at(&generated, "server.ip").unwrap(), "127.0.0.1");
        assert_eq!(value_at(&generated, "server.port").unwrap(), 25565);
        assert_eq!(
            value_at(&generated, "server.default-protocol").unwrap(),
            "AUTOMATIC"
        );
        assert_eq!(value_at(&generated, "auto.auto-reconnect").unwrap(), true);
        assert_eq!(
            value_at(&generated, "auto.auto-reconnect-time").unwrap(),
            30
        );
        assert_eq!(value_at(&generated, "misc.language").unwrap(), "ENGLISH");
        assert_eq!(
            value_at(&generated, "start-text.text").unwrap(),
            &json!(["/home fishing"])
        );
        // 模板里"我们不管"的键保持模板原样（挑两个有代表性的：account.mail 与 logs.log-count）。
        assert_eq!(value_at(&generated, "account.mail").unwrap(), "");
        assert_eq!(value_at(&generated, "logs.log-count").unwrap(), 1);
    }

    /// `[i18n] language` 的取值原样写进 `misc.language`（中文简体要写 `CHINESE_SIMPLIFIED`）。
    #[test]
    fn chinese_language_maps_to_the_upstream_constant() {
        let generated = serde_json::from_str(
            &fishingbot_config_json(&sample_fishingbot_config(), "CHINESE_SIMPLIFIED")
                .expect("序列化"),
        )
        .expect("生成的是合法 JSON");

        assert_eq!(
            value_at(&generated, "misc.language").unwrap(),
            "CHINESE_SIMPLIFIED"
        );
    }

    /// `[fishingbot]` 改了什么，生成的文件里就跟着变；空 `start_text` 把 `enabled` 关掉。
    #[test]
    fn values_come_from_the_config_table() {
        let fishingbot = FishingBotConfig {
            server_ip: "play.example.com".to_string(),
            server_port: 25599,
            default_protocol: "1.20.4".to_string(),
            auto_reconnect: false,
            auto_reconnect_time: 7,
            start_text: Vec::new(),
            announce_type_console: "ALL".to_string(),
            announce_type_chat: "ONLY_BOOKS".to_string(),
        };

        let generated =
            serde_json::from_str(&fishingbot_config_json(&fishingbot, "ENGLISH").expect("序列化"))
                .expect("生成的是合法 JSON");

        assert_eq!(
            value_at(&generated, "server.ip").unwrap(),
            "play.example.com"
        );
        assert_eq!(value_at(&generated, "server.port").unwrap(), 25599);
        assert_eq!(
            value_at(&generated, "server.default-protocol").unwrap(),
            "1.20.4"
        );
        assert_eq!(value_at(&generated, "auto.auto-reconnect").unwrap(), false);
        assert_eq!(value_at(&generated, "auto.auto-reconnect-time").unwrap(), 7);
        assert_eq!(
            value_at(&generated, "announces.announce-type-console").unwrap(),
            "ALL"
        );
        assert_eq!(
            value_at(&generated, "announces.announce-type-chat").unwrap(),
            "ONLY_BOOKS"
        );
        assert_eq!(value_at(&generated, "start-text.enabled").unwrap(), false);
        assert_eq!(value_at(&generated, "start-text.text").unwrap(), &json!([]));
    }

    /// 删旧文件：存在就删掉（并记一行），不存在不算错 —— 生成流程靠它做到"不留旧键"。
    #[test]
    fn old_fishingbot_config_is_removed_and_missing_is_fine() {
        let dir = temp_test_dir("remove-old-game-config");
        // `temp_test_dir` 只给一个路径，目录要自己建（这里要往里面写文件）。
        fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("conf.json");

        // 不存在：什么都不做，也不算失败。
        assert!(remove_old_fishingbot_config(&path).is_ok());

        fs::write(&path, "{\"server\":{\"ip\":\"old\"}}").expect("写旧文件");
        assert!(remove_old_fishingbot_config(&path).is_ok());
        assert!(!path.exists(), "旧文件要被删掉");

        // 路径是个目录时不能当成"没找到"混过去：那是真错误。
        assert!(
            remove_old_fishingbot_config(&dir).is_err(),
            "把目录当文件删要报错"
        );

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 生成流程里"干活"那半：先删旧、再整份覆盖写 —— 写出来的是模板 + 托管键，旧文件里的键不会留下。
    #[test]
    fn write_step_replaces_the_old_file_with_a_complete_config() {
        let dir = temp_test_dir("write-game-config");
        fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("conf.json");

        // 旧文件里放一个模板里没有的键：整份替换之后它必须消失。
        fs::write(&path, "{\"legacy\":{\"keep\":true}}").expect("写旧文件");

        write_fishingbot_config(&path, &sample_fishingbot_config(), "ENGLISH").expect("写配置");

        let written = fs::read_to_string(&path).expect("读回写出来的配置");
        assert_eq!(
            written,
            fishingbot_config_json(&sample_fishingbot_config(), "ENGLISH").expect("生成")
        );

        let written: Value = serde_json::from_str(&written).expect("写出来的是合法 JSON");
        assert_eq!(value_at(&written, "server.ip").unwrap(), "127.0.0.1");
        assert!(
            value_at(&written, "legacy").is_none(),
            "旧文件里的键不该留下：整份是模板 + 托管键"
        );

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 模板里找不到托管路径时，报错点名那条路径（生成流程靠它指出"上游改了键名"）。
    #[test]
    fn missing_template_path_reports_the_path() {
        let mut template = json!({ "server": {} });

        let err = set_json_path(&mut template, "misc.language", json!("ENGLISH"))
            .expect_err("模板里没有 misc 表，应当报错");

        assert!(err.contains("misc.language"), "报错应点名路径: {err}");
    }

    /// 模板本身：解析成 `Value`（按路径取值，与键顺序无关）。
    fn template_value() -> Value {
        serde_json::from_str(FISHINGBOT_CONFIG_TEMPLATE).expect("模板是合法 JSON")
    }

    /// 托管路径（测试用）：与生成流程同源，清单不会漂移。
    fn managed_paths() -> Vec<&'static str> {
        managed_values(&sample_fishingbot_config(), Language::English)
            .iter()
            .map(|(path, _)| *path)
            .collect()
    }

    /// 按点分路径取一个值（测试用）。
    fn value_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
        let mut cursor = root;
        for segment in path.split('.') {
            cursor = cursor.get(segment)?;
        }

        Some(cursor)
    }

    /// 摊平所有叶子（数组整体算一个叶子），用于"除了托管路径，其余与模板一致"那条断言。
    fn leaf_paths(root: &Value) -> Vec<(String, Value)> {
        let mut collected = Vec::new();
        collect_leaves(root, "", &mut collected);

        collected
    }

    fn collect_leaves(node: &Value, prefix: &str, collected: &mut Vec<(String, Value)>) {
        match node {
            Value::Object(object) => {
                for (key, child) in object {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    collect_leaves(child, &path, collected);
                }
            }
            // 数组整体当一个叶子：托管的 `start-text.text` 就是数组。
            leaf => collected.push((prefix.to_string(), leaf.clone())),
        }
    }
}
