//! 日志记录的格式与渲染：时间戳、级别前缀与续行缩进。

use chrono::Local;
use log::Level;

/// 日志时间戳格式。
const LOG_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// 日志续行缩进。
const CONTINUATION_INDENT: &str = "  ";

/// 当前时间的时间戳文本。
pub(super) fn timestamp() -> String {
    Local::now().format(LOG_TIME_FORMAT).to_string()
}

/// 渲染一条日志记录，空消息丢弃，续行缩进两个空格。
pub(super) fn format_record(timestamp: &str, level: Level, message: &str) -> Option<String> {
    if message.is_empty() {
        return None;
    }

    let prefix = format!("[{timestamp}] [{level}] ");
    let mut rendered = String::new();

    for (index, chunk) in message.lines().enumerate() {
        if index == 0 {
            rendered.push_str(&prefix);
        } else {
            rendered.push('\n');
            rendered.push_str(CONTINUATION_INDENT);
        }
        rendered.push_str(chunk);
    }
    rendered.push('\n');

    Some(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_preserve_content_and_indent_continuations() {
        for (timestamp, level, message, expected) in [
            (
                "2026-09-30 15:30:12",
                Level::Info,
                "已加载配置",
                Some("[2026-09-30 15:30:12] [INFO] 已加载配置\n"),
            ),
            (
                "T",
                Level::Warn,
                "标题:\n- 一行\n- 二行",
                Some("[T] [WARN] 标题:\n  - 一行\n  - 二行\n"),
            ),
            ("T", Level::Error, "", None),
            ("T", Level::Info, "a\r\nb", Some("[T] [INFO] a\n  b\n")),
        ] {
            assert_eq!(
                format_record(timestamp, level, message).as_deref(),
                expected,
                "{message:?}"
            );
        }
    }
}
