//! 组装报告，并将业务结果转换为回复文本。

use crate::config;
pub use crate::models::Report;
use crate::text;

impl Report {
    /// 用按给定顺序排列的计数行覆盖基础信息。
    pub fn set_counts(&mut self, counts: &[(&str, usize)]) {
        self.basics = counts
            .iter()
            .map(|(label, count)| text::get().count_line(label, *count))
            .collect();
    }

    /// 按标题、基础信息、各类明细和原文内容的顺序渲染。
    /// 超过 `runtime.max_reply_chars` 时按行截断并注明省略行数；长度以 UTF-16 码元计算。
    pub fn render(&self) -> String {
        let mut blocks: Vec<String> = Vec::new();

        let mut header: Vec<String> = Vec::new();
        if let Some(title) = &self.title {
            header.push(format!("{title}:"));
            header.push("=======================".to_string());
        }
        header.extend(self.basics.iter().map(|line| format!("- {line}")));
        if !header.is_empty() {
            blocks.push(header.join("\n"));
        }

        let i18n_text = text::get();
        Self::push_list_block(
            &mut blocks,
            i18n_text.report_succeeded_label(),
            &self.succeeded,
        );
        Self::push_list_block(&mut blocks, i18n_text.report_skipped_label(), &self.skipped);
        Self::push_list_block(&mut blocks, i18n_text.report_queued_label(), &self.queued);
        Self::push_list_block(&mut blocks, i18n_text.report_failed_label(), &self.failed);
        if let Some(content) = &self.content {
            blocks.push(format!("{}:\n{content}", i18n_text.report_content_label()));
        }

        let reply_text = blocks.join("\n\n");

        let max_utf16_units = config::Config::get().runtime.max_reply_chars;
        if reply_text.utf16_len() <= max_utf16_units {
            return reply_text;
        }

        let lines: Vec<&str> = reply_text.lines().collect();
        let mut kept_lines: Vec<&str> = Vec::new();
        let mut used_chars = 0usize;
        for line in &lines {
            let line_chars = line.utf16_len() + 1;
            let omitted_line_count = lines.len() - kept_lines.len() - 1;
            let omitted_note = i18n_text.report_omitted_lines_note(omitted_line_count);
            if used_chars + line_chars + omitted_note.utf16_len() > max_utf16_units {
                break;
            }
            kept_lines.push(line);
            used_chars += line_chars;
        }

        // 上限过小时，省略说明本身可能超过该上限。
        let dropped_line_count = lines.len() - kept_lines.len();
        format!(
            "{}\n{}",
            kept_lines.join("\n"),
            i18n_text.report_omitted_lines_note(dropped_line_count)
        )
    }

    /// 追加非空的明细小节。
    fn push_list_block(blocks: &mut Vec<String>, label: &str, lines: &[String]) {
        if lines.is_empty() {
            return;
        }

        let mut block = vec![format!("{label}:")];
        block.extend(lines.iter().map(|line| format!("- {line}")));
        blocks.push(block.join("\n"));
    }
}

/// 回复长度的标准： `reply_text.utf16_len()`。
/// Discord 的 2000 上限按 UTF-16 码元计（一个 emoji / 非 BMP 字符占 2 个）。
trait Utf16Len {
    fn utf16_len(&self) -> usize;
}

impl Utf16Len for str {
    fn utf16_len(&self) -> usize {
        self.encode_utf16().count()
    }
}

/// 将业务结果转换为回复文本；成功和失败内容均会发送给用户，不得包含内部敏感信息。
pub trait IntoReplyText {
    /// 转换为可直接回复的文本。
    fn to_reply_text(self) -> String;
}

impl IntoReplyText for Result<String, String> {
    fn to_reply_text(self) -> String {
        self.unwrap_or_else(|msg| msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 非 BMP 字符占两个 UTF-16 码元。
    #[test]
    fn length_counts_utf16_units() {
        assert_eq!("abc".utf16_len(), 3);
        assert_eq!("汉字".utf16_len(), 2);
        assert_eq!("🐟".utf16_len(), 2); // 一个码点、两个 UTF-16 码元
    }
}
