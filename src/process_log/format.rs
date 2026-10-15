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
        if index > 0 {
            rendered.push('\n');
        }
        if index == 0 {
            rendered.push_str(&prefix);
        } else {
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

    /// 单行记录：`[时间] [级别] 正文` + 一个换行。
    #[test]
    fn single_line_record_gets_one_prefix() {
        assert_eq!(
            format_record("2026-09-30 15:30:12", Level::Info, "已加载配置").as_deref(),
            Some("[2026-09-30 15:30:12] [INFO] 已加载配置\n")
        );
    }

    /// 多行记录（`log::info!("{}", report)` 打一整块 `Report`）：每行都在，
    /// 续行缩进两格，这样第二行起不会"裸奔"，翻日志时仍能看出时间与级别。
    #[test]
    fn multi_line_record_indents_continuation_lines() {
        assert_eq!(
            format_record("T", Level::Warn, "标题:\n- 一行\n- 二行").as_deref(),
            Some("[T] [WARN] 标题:\n  - 一行\n  - 二行\n")
        );
    }

    /// 空消息直接丢弃：不给它留"时间 + 级别"，日志里就不会出现来历不明的空行。
    #[test]
    fn empty_message_is_dropped() {
        assert_eq!(format_record("T", Level::Error, ""), None);
    }

    /// 正文里的 CRLF 不该把 `\r` 带进日志文件（`lines()` 已经帮我们去掉了）。
    #[test]
    fn carriage_returns_are_dropped() {
        assert_eq!(
            format_record("T", Level::Info, "a\r\nb").as_deref(),
            Some("[T] [INFO] a\n  b\n")
        );
    }
}
