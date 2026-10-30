//! 组装报告，并将业务结果转换为回复文本。

use crate::config;
use crate::text;
use crate::utils::bot_dirs::BotDirEntry;

/// 通用报告体：标题、基础信息/列表行、各类明细、整块内容，全部可选。
/// 约定：数组长度为0 / 值为 None 表示未提供，渲染时未提供的部分不会出现。
#[derive(Debug)]
pub struct Report {
    /// 标题；`None` 时省略标题与分隔线。
    pub title: Option<String>,
    /// 基础信息行与列表行：计数、环境指标、表头、名单、状态行等。
    pub basics: Vec<String>,
    /// 成功明细行。
    pub succeeded: Vec<String>,
    /// 跳过明细行，如 已运行 的部分。
    pub skipped: Vec<String>,
    /// 排队明细行，如 还在启动队列里排着的机器人（`/show status` 报告下方那一栏）。
    pub queued: Vec<String>,
    /// 失败明细行。
    pub failed: Vec<String>,
    /// 整块原样内容：日志、配置、帮助文本等。
    pub content: Option<String>,
}

impl Default for Report {
    /// 空报告（与 [`Report::new`] 同义）。
    fn default() -> Self {
        Self::new()
    }
}

impl Report {
    /// 新创建报告。
    pub fn new() -> Self {
        Self {
            title: None,
            basics: Vec::new(),
            succeeded: Vec::new(),
            skipped: Vec::new(),
            queued: Vec::new(),
            failed: Vec::new(),
            content: None,
        }
    }

    /// 为报告设置标题。
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

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

/// 业务结果转换为回复文本的标准。
/// `Result<String, String>` 原样透出；带明细的结果类型（例如 `startup::BotStartResults`）在这里渲染整段回复。
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

// 启动结果：类型、逐台结果、以及转报告 / 转回复文案都在这里。

pub(crate) type BotStartEntry = (BotDirEntry, BotStartOutcome);

/// 机器人启动结果：构造启动报告用的数据（机器人条目 + 启动结果）。
#[derive(Debug, Default)]
pub struct BotStartResults {
    entries: Vec<BotStartEntry>,
    /// 复核时日志已确认进服的机器人名（成功明细据此选说法）。
    online_confirmed_bot_names: Vec<String>,
}

/// 单台机器人的启动结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotStartOutcome {
    /// 拉起命令发出去了（复核没过会被改写成 [`BotStartOutcome::Failure`]）。
    Succeeded,
    /// 机器人已经在运行，所以跳过（不算失败）。
    Skipped,
    /// 尚未拉起即被取消：`/stopall`（以及看门狗的掉线收尾、退出收尾）撤销了本次启动，
    /// 因此不计入失败（见 [`crate::utils::stop_all_bot_sessions`]）。
    Cancelled,
    /// 启动遇到错误，带能直接进回复的一行。
    Failure(String),
}

impl BotStartResults {
    /// 向启动结果中添加一条新结果（全部启动后，循环调用）。
    pub(super) fn push(&mut self, bot_dir_entry: BotDirEntry, outcome: BotStartOutcome) {
        self.entries.push((bot_dir_entry, outcome));
    }

    /// 记下"复核时日志已确认进服"，成功明细据此选说法。
    pub(super) fn mark_online_confirmed(&mut self, bot_name: &str) {
        self.online_confirmed_bot_names.push(bot_name.to_string());
    }

    /// 逐条遍历（`reply.rs` 转报告时也要用）。
    pub(crate) fn iter(&self) -> impl Iterator<Item = &BotStartEntry> {
        self.entries.iter()
    }

    /// 复核没过写回这一台；找不到就说明结果已经回包了。
    pub(super) fn set_outcome(&mut self, bot_name: &str, outcome: BotStartOutcome) {
        if let Some((_, stored)) = self
            .entries
            .iter_mut()
            .find(|(bot_dir_entry, _)| bot_dir_entry.bot_name == bot_name)
        {
            *stored = outcome;
        }
    }

    /// 有没有失败（与 [`StopOutcome::has_failed_lines`] 对称）。
    pub(crate) fn has_failed_lines(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, BotStartOutcome::Failure(_)))
    }

    /// 是否有机器人在排队期间已由其他入口启动。
    pub(crate) fn has_skipped_running_bots(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, outcome)| matches!(outcome, BotStartOutcome::Skipped))
    }

    /// 拉起/复核失败的明细（按跑完顺序）。
    ///
    /// 给的是原始明细：`/restartall` 要把"停止阶段 + 启动阶段"的失败按顺序拼成一段。
    pub(crate) fn failure_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|(_, outcome)| match outcome {
                BotStartOutcome::Failure(failure_line) => Some(failure_line.clone()),
                _ => None,
            })
            .collect()
    }

    /// 排队期间已由其他入口启动的机器人名（按跑完顺序）。
    pub(crate) fn skipped_running_bot_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, outcome)| matches!(outcome, BotStartOutcome::Skipped))
            .map(|(bot_dir_entry, _)| bot_dir_entry.bot_name.as_str())
            .collect()
    }

    /// 有没有被取消的启动（"停止全部机器人"把这次启动撤了下来）。
    pub(crate) fn has_cancelled_bots(&self) -> bool {
        !self.cancelled_bot_names().is_empty()
    }

    /// 被取消的机器人名（按跑完顺序）。
    pub(crate) fn cancelled_bot_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, outcome)| matches!(outcome, BotStartOutcome::Cancelled))
            .map(|(bot_dir_entry, _)| bot_dir_entry.bot_name.as_str())
            .collect()
    }
}

// 启动结果转回复：启动模块只产出结果，回复文案与拼装都在这里（见 [`BotStartResults`]）。

impl BotStartResults {
    /// 报告：成功的与失败的都进报告；计数由调用方按指令口径 `set_counts`。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new();
        let i18n_text = text::get();

        for (bot_dir_entry, outcome) in self.iter() {
            match outcome {
                BotStartOutcome::Succeeded => report.succeeded.push(
                    if self
                        .online_confirmed_bot_names
                        .contains(&bot_dir_entry.bot_name)
                    {
                        i18n_text.start_command_sent_confirmed(&bot_dir_entry.bot_name)
                    } else {
                        i18n_text.start_command_sent_unconfirmed(&bot_dir_entry.bot_name)
                    },
                ),
                BotStartOutcome::Skipped => report
                    .skipped
                    .push(i18n_text.startall_skipped_running(&bot_dir_entry.bot_name)),
                BotStartOutcome::Cancelled => report
                    .skipped
                    .push(i18n_text.start_cancelled(&bot_dir_entry.bot_name)),
                BotStartOutcome::Failure(failure_line) => report.failed.push(failure_line.clone()),
            }
        }

        report
    }

    /// 失败明细拼成单行错误提示（单个机器人指令直接拿它当 `Err`）。
    pub(crate) fn to_failure_text(&self) -> String {
        self.failure_lines().join(" / ")
    }
}

impl IntoReplyText for BotStartResults {
    /// 整段回复：成功 / 跳过（含被取消的）/ 失败三份明细。
    ///
    /// 标题与计数由调用方按指令口径加，所以要自己组装时走 [`BotStartResults::into_report`]。
    fn to_reply_text(self) -> String {
        self.into_report().render()
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
