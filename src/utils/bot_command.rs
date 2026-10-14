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

#[cfg(test)]
mod tests {
    use super::*;

    /// 多词指令用单空格连接；一个片段都没有时按"空指令"拒绝。
    #[test]
    fn bot_command_is_joined_by_single_space() {
        assert_eq!(
            ["/home", "fishing"].to_clean_bot_command(),
            Ok("/home fishing".to_string())
        );

        let no_args: &[&str] = &[];
        assert!(no_args.to_clean_bot_command().is_err());
    }

    /// **原样透传**：单个片段（`/command`、`/commandall` 就是这种情况）不做任何加工 ——
    /// 不剥引号、不合并连续空格、不 trim。用户输入什么就注入什么。
    #[test]
    fn bot_command_keeps_user_input_verbatim() {
        for raw in [
            "/home fishing",
            "/msg  dua  spaces",
            "\"/home fishing\"",
            " say hello ",
        ] {
            assert_eq!(
                [raw].to_clean_bot_command(),
                Ok(raw.to_string()),
                "{raw:?} 应当原样发出"
            );
        }
    }
}
