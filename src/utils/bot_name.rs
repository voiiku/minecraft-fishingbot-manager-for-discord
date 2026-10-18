//! 机器人名的标准：一个机器人名对应磁盘上的一个目录名。
//!
//! 路径规则见 [`super::bot_dirs`]。

use crate::text;

/// `&str` 与 `String` 直可接调用 .to_clean_bot_name() 方法
pub trait ToCleanBotName {
    /// 去首尾空白 + 转小写；只允许字母、数字、下划线与连字符。
    /// 出现别的字符直接返回 `Err`,直接回复给用户
    /// 名字只是昵称，只决定目录名，实际登录哪个游戏角色由 Token 决定。
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

/// 名中允许出现的字符白名单。
fn is_bot_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
