//! 机器人名称的基础校验与规范化。

/// 名称为空或包含不允许的字符。
#[derive(Debug, PartialEq, Eq)]
pub enum BotNameError<'a> {
    Empty,
    InvalidCharacter { name: &'a str, character: char },
}

/// 规范化机器人名称，不负责生成本地化错误文案。
pub trait ToCleanBotName {
    /// 去除首尾空白并转为小写；仅允许 ASCII 字母、数字、下划线和连字符。
    fn to_clean_bot_name(&self) -> Result<String, BotNameError<'_>>;
}

impl ToCleanBotName for str {
    fn to_clean_bot_name(&self) -> Result<String, BotNameError<'_>> {
        let name = self.trim();
        if name.is_empty() {
            return Err(BotNameError::Empty);
        }

        if let Some(character) = name.chars().find(|character| {
            !character.is_ascii_alphanumeric() && !matches!(*character, '_' | '-')
        }) {
            return Err(BotNameError::InvalidCharacter { name, character });
        }

        Ok(name.to_ascii_lowercase())
    }
}
