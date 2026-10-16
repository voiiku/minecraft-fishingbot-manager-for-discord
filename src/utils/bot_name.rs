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

#[cfg(test)]
mod tests {
    use super::*;

    /// 首尾空白不影响结果，大写统一转小写（与帮助文案里的"不区分大小写"一致）。
    #[test]
    fn trims_and_lowercases() {
        assert_eq!("  Duck_02 ".to_clean_bot_name(), Ok("duck_02".to_string()));
    }

    /// 连字符与下划线是合法字符，不能被当成非法输入拦下。
    #[test]
    fn hyphen_and_underscore_are_allowed() {
        assert_eq!("duck-02_x".to_clean_bot_name(), Ok("duck-02_x".to_string()));
    }

    /// 特殊字符不再被过滤掉，而是报错：用户会看到"哪个字符不行"，而不是拿到一个他没写过的名字。
    /// 这些输入同时覆盖了路径穿越（`/`、`.`）与命令注入（`;`、空格）两类。
    #[test]
    fn illegal_characters_are_rejected() {
        let illegal_inputs = [
            "中文名字",
            "duck 02",
            "duck.02",
            "../../etc/passwd",
            "duck;rm -rf /",
        ];

        for raw_bot_name in illegal_inputs {
            let message = raw_bot_name
                .to_clean_bot_name()
                .expect_err("含非法字符的名字应当报错");
            let illegal_char = raw_bot_name
                .chars()
                .find(|c| !is_bot_name_char(*c))
                .expect("这些用例里都含非法字符");

            // 期望值从语言表取（这句话随语言变）：里面要带上名字与那个非法字符。
            assert_eq!(
                message,
                text::get().bot_name_illegal_char(raw_bot_name.trim(), illegal_char),
                "{raw_bot_name} 的报错应指出非法字符"
            );
        }
    }

    /// 空名字（或只有空白）单独报"不能为空"，与"含非法字符"的文案区分开。
    #[test]
    fn empty_name_is_rejected() {
        for raw_bot_name in ["", "   "] {
            let message = raw_bot_name
                .to_clean_bot_name()
                .expect_err("空名字应当报错");
            // 期望值同样从语言表取："不能为空"与"含非法字符"两句措辞的差别由语言文件负责。
            assert_eq!(
                message,
                text::get().bot_name_empty(),
                "{raw_bot_name:?} -> {message}"
            );
        }
    }

    /// `&str` 与 `String` 都直接可用：trait 实现在 `str` 上，`String` 靠 deref 命中。
    #[test]
    fn str_and_string_both_work() {
        let owned_bot_name = String::from(" Duck_02 ");

        assert_eq!(
            owned_bot_name.to_clean_bot_name(),
            Ok("duck_02".to_string())
        );
        assert_eq!(
            owned_bot_name.as_str().to_clean_bot_name(),
            Ok("duck_02".to_string())
        );
    }
}
