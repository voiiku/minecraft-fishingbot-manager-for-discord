//! 机器人名称规范化；名称对应磁盘目录，路径规则见 [`super::bot_dirs`]。

use crate::text;

/// 规范化机器人名称。
pub trait ToCleanBotName {
    /// 去除首尾空白并转为小写；仅允许 ASCII 字母、数字、下划线和连字符。
    /// 名称仅用于标识目录，游戏角色由 Token 决定。
    fn to_clean_bot_name(&self) -> Result<String, String>;
}

impl ToCleanBotName for str {
    fn to_clean_bot_name(&self) -> Result<String, String> {
        let trimmed_bot_name = self.trim();

        if trimmed_bot_name.is_empty() {
            return Err(text::get().bot_name_empty().to_string());
        }

        if let Some(illegal_char) = trimmed_bot_name.chars().find(|c| !is_bot_name_char(*c)) {
            return Err(text::get().bot_name_illegal_char(trimmed_bot_name, illegal_char));
        }

        Ok(trimmed_bot_name.to_ascii_lowercase())
    }
}

fn is_bot_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
