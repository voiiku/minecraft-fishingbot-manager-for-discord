//! 游戏内命令片段的拼接。

/// 将原始字符串切片转换为游戏内命令。
pub trait ToCleanBotCommand {
    /// 片段用单空格连接；拼完为空时返回 `None`。
    fn to_clean_bot_command(&self) -> Option<String>;
}

impl ToCleanBotCommand for [&str] {
    fn to_clean_bot_command(&self) -> Option<String> {
        let command = self.join(" ");
        (!command.is_empty()).then_some(command)
    }
}
