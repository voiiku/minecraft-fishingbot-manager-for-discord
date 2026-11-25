//! FishingBot 配置模板的字段构造与路径更新。

use crate::config::FishingBotConfig;
use serde_json::{Value, json};

pub(crate) const FISHINGBOT_CONFIG_TEMPLATE: &str =
    include_str!("../../../fishingbot-config.example.json");

pub(crate) fn managed_values(
    fishingbot: &FishingBotConfig,
    language: &str,
) -> Vec<(&'static str, Value)> {
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
        ("misc.language", json!(language)),
    ]
}

pub(crate) fn set_json_path<'a>(
    root: &mut Value,
    path: &'a str,
    value: Value,
) -> Result<(), &'a str> {
    let (parent_path, key) = path.rsplit_once('.').ok_or(path)?;
    let mut cursor = root;
    for segment in parent_path.split('.') {
        cursor = cursor.get_mut(segment).ok_or(path)?;
    }

    let object = cursor.as_object_mut().ok_or(path)?;
    object.insert(key.to_string(), value);
    Ok(())
}
