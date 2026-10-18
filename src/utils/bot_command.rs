//! 游戏内指令相关的基础实现（`/command` 与 `/commandall` 共用）。

use crate::text;

/// 将原始字符串切片转换为游戏内指令：`raw_bot_command_args.to_clean_bot_command()`。
pub trait ToCleanBotCommand {
    /// 片段用单空格连接；一个片段都没有（拼完为空）就 `Err`，文案可直接回复。
    fn to_clean_bot_command(&self) -> Result<String, String>;
}

impl ToCleanBotCommand for [&str] {
    fn to_clean_bot_command(&self) -> Result<String, String> {
        let bot_command = self.join(" ");

        if bot_command.is_empty() {
            return Err(text::get().bot_command_empty().to_string());
        }

        Ok(bot_command)
    }
}
