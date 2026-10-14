//! 回复的组装与渲染逻辑：报告容器 [`Report`] 与回复的长度限制。
//!
//! 需要输出报告的指令都用 [`Report`] 组装、由 [`Report::render`] 渲染，
//! 只回一行的单个机器人指令用 [`Report::has_failed_lines`] 与 [`Report::to_failure_text`] 判断结果。
//!
//! 渲染受到 `runtime.max_reply_chars` 长度限制：Discord 对单条消息的硬上限（2000 字符），
//! 长度按 UTF-16 码元数（见 [`Utf16Len::utf16_len`]），与 Discord 的口径一致。
//! 超长会发送失败，所以超长时必需截断并写明省略了多少行。
//!
//! 新增输出类型时，在结构体里加一个可选字段并同步更新 [`Report::new`] 与 [`Report::render`] 即可，
//!
//! 提供 [`IntoReplyText`]，把业务函数的 `Result<String, String>` 直接转成回复文本。
//!
//! 关于`/show status` ：因为/show status 只展示运行中的机器人状态，所以它定义在 [`super::running_bots`] 中

use crate::config;
use crate::text;

/// 通用报告体：标题、基础信息/列表行、各类明细、整块内容，全部可选。
/// 约定：数组长度为0即视为未提供；单值字段用 None 表示未提供，渲染时未提供的部分不会出现。
#[derive(Debug)]
pub struct Report {
    /// 标题；`None` 时省略标题与分隔线。
    pub title: Option<String>,
    /// 基础信息行与列表行：计数、环境指标、表头、名单、状态行等。
    ///
    /// 名单、状态表这类“一行一条”的数据放这里 —— 它们算报告开头就该看到的信息，
    /// 单独开一节会把表头与数据行割裂开，因此不再另设明细字段。
    pub basics: Vec<String>,
    /// 成功明细行。
    pub succeeded: Vec<String>,
    /// 跳过明细行，如 已运行 的部分。
    pub skipped: Vec<String>,
    /// 失败明细行。
    pub failed: Vec<String>,
    /// 整块原样内容：日志、配置、帮助文本等。
    pub content: Option<String>,
}

impl Report {
    /// 新创建报告（全空）。
    /// 新增输出类型时改这里与 [`Report::render`]。
    pub fn new() -> Self {
        Self {
            title: None,
            basics: Vec::new(),
            succeeded: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
            content: None,
        }
    }

    /// 设置标题：`new()` 出来的是空报告，标题从这里补（有的要等流程跑完才知道）。
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// 用一组 标签 + 数量 覆盖基础信息行，统一格式为「标签: N 个」。
    /// 批量指令的计数行都使用它：一次性完全传递，传递顺序与实际回复一致；
    /// 非计数的基础信息行（表头、名单、说明）直接往 `basics` 里 push，不使用本函数。
    pub fn set_counts(&mut self, counts: &[(&str, usize)]) {
        self.basics = counts
            .iter()
            .map(|(label, count)| text::get().count_line(label, *count))
            .collect();
    }

    /// 把report渲染成回复文本（`ctx.say` 直接发的就是它）：
    ///
    /// 1. 标题块：`title` 有值时：标题一行 + 一条分隔线；否则为空
    /// 2. 基础信息与列表行（计数、表头、名单、状态行…）：逐行加 `- `，与下面的明细行保持一致；
    /// 3. 三个明细小节：成功 / 跳过 / 失败，各自非空才出现（见 [`Report::push_list_block`]）；
    /// 4. 整块内容：`标签:` 一行 + 原文原样输出（日志、配置、帮助文本这类不能被逐行加前缀）；
    /// 5. 各块之间用空行分隔，拼成完整回复；
    /// 6. 截断：超过 `runtime.max_reply_chars` 就从末尾开始按行丢弃，并注明省略了多少行
    ///    Discord 对单条消息有硬上限（2000个utf16字符），超长会发送失败（用户那边看起来"什么都没发生"）。
    ///    截断时先把截断提示的长度算进去，所以上限装得下"第一行 + 说明"，结果就一定在上限之内。
    ///    长度口径见 [`Utf16Len::utf16_len`]（UTF-16 是 Discord 的口径）。
    pub fn render(&self) -> String {
        let mut blocks: Vec<String> = Vec::new();

        // 1. 标题块。
        let mut header: Vec<String> = Vec::new();
        // 如果已经存在标题，就加上分隔线；否则不加，避免空标题时出现。
        if let Some(title) = &self.title {
            header.push(format!("{title}:"));
            header.push("=======================".to_string());
        }
        // 2. 基础信息与列表行：每行前加 `- `，与明细行保持一致。
        header.extend(self.basics.iter().map(|line| format!("- {line}")));
        if !header.is_empty() {
            blocks.push(header.join("\n"));
        }

        // 3. 明细小节：成功 / 跳过 / 失败，各自非空才出现（标签自带括号，见 push_list_block）。
        Self::push_list_block(
            &mut blocks,
            text::get().report_succeeded_label(),
            &self.succeeded,
        );
        Self::push_list_block(
            &mut blocks,
            text::get().report_skipped_label(),
            &self.skipped,
        );
        Self::push_list_block(&mut blocks, text::get().report_failed_label(), &self.failed);
        // 4. 整块内容：先写前缀并换行，再把整块内容原样输出（不逐行加前缀）。
        if let Some(content) = &self.content {
            blocks.push(format!(
                "{}:\n{content}",
                text::get().report_content_label()
            ));
        }

        // 5. 各块之间用空行分隔，拼成完整回复。
        let reply_text = blocks.join("\n\n");

        // 6. 截断逻辑。
        let max_utf16_units = config::Config::get().runtime.max_reply_chars;
        if reply_text.utf16_len() <= max_utf16_units {
            return reply_text;
        }

        // 超长度：从末尾按行丢（关键信息都在头），注明省略了多少行。
        let lines: Vec<&str> = reply_text.lines().collect();
        let mut kept_lines: Vec<&str> = Vec::new();
        let mut used_chars = 0usize;
        for line in &lines {
            // 换行符也算进长度，否则多行拼起来又会重新超限。
            let line_chars = line.utf16_len() + 1;
            // 保留的说明占位
            let omitted_line_count = lines.len() - kept_lines.len() - 1;
            let omitted_note = text::get().report_omitted_lines_note(omitted_line_count);
            if used_chars + line_chars + omitted_note.utf16_len() > max_utf16_units {
                break;
            }
            kept_lines.push(line);
            used_chars += line_chars;
        }

        // 保留多少行、丢了多少行一起算清，末尾接上说明（上限小到连第一行都放不下属于坏配置，不考虑）。
        let dropped_line_count = lines.len() - kept_lines.len();
        format!(
            "{}\n{}",
            kept_lines.join("\n"),
            text::get().report_omitted_lines_note(dropped_line_count)
        )
    }

    /// 追加逐行列出的小节；内容为空时跳过。
    /// 标签自己带括号（中文是 `(成功明细)`、英文是 `Success`）：中英文的括号样式不同，由各语言的实现决定。
    /// 这里只负责拼标签和每一行的前缀‘-’。
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
///
/// 口径是 **UTF-16 code unit 数**：Discord 的 2000 上限按 UTF-16 码元计（一个 emoji / 非 BMP 字符占 2 个）。
trait Utf16Len {
    fn utf16_len(&self) -> usize;
}

impl Utf16Len for str {
    fn utf16_len(&self) -> usize {
        self.encode_utf16().count()
    }
}

/// 业务层返回的`Result<String, String>` 转换为回复文本的标准。
/// 业务函数返回的 无论Ok还是Err都会回复给用户，因此信息不得包含路径、会话名等内部细节。
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

    /// 长度口径是 UTF-16 码元（`str::utf16_len`）：非 BMP 字符（emoji）算 2，这样"刚好 2000"才真的发得出去。
    #[test]
    fn length_counts_utf16_units() {
        assert_eq!("abc".utf16_len(), 3);
        assert_eq!("汉字".utf16_len(), 2);
        assert_eq!("🐟".utf16_len(), 2); // 一个码点、两个 UTF-16 码元
    }
}
