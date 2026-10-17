//! 组装与渲染回复报告。
//!
//! 需要输出报告的指令都用 [`Report`] 组装、由 [`Report::render`] 渲染，
//! 简短回复用结果类型上的 `has_failed_lines` 与 `to_failure_text`（启动那套定义在本文件）。
//!
//! 新增输出类型时，在结构体中加入可选字段并同步更新 [`Report::new`] 与 [`Report::render`] 即可，
//!
//! 提供 [`IntoReplyText`]：业务结果直接转成回复文本 —— `Result<String, String>` 原样透出，
//! 带明细的结果类型在这里渲染整段回复（例如 `startup::BotStartResults`：`into_report` 与 `to_reply_text` 定义在本文件）。
//!
//! 关于`/show status` 的回复文本：因为/show status 只展示运行中的机器人状态，所以它定义在 [`super::running_bots`] 中

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
    ///
    /// 名单、状态表这类“一行一条”的数据放这里 —— 它们算报告开头就该看到的信息，
    /// 单独开一节会把表头与数据行割裂开，因此不再另设明细字段。
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

    /// 用一组 标签 + 数量 覆盖基础信息行，统一格式为「标签: N 个」。
    /// 如第一行-成功：2个，第二行-失败：1个
    /// 批量指令的计数行使用它：一次性完全传递，传递顺序与实际回复一致。
    /// 非计数信息（表头、名单、说明）直接向 `basics` 中 push，不使用本函数。
    pub fn set_counts(&mut self, counts: &[(&str, usize)]) {
        self.basics = counts
            .iter()
            .map(|(label, count)| text::get().count_line(label, *count))
            .collect();
    }

    /// 把report渲染成回复文本（`ctx.say` 发送它）：
    ///
    /// 1. 标题块：`title` 有值时：标题一行 + 一条分隔线；否则为空
    /// 2. 基础信息与列表行（计数、表头、名单、状态行…）：逐行加 `- `，与下面的明细行保持一致；
    /// 3. 三个明细小节：成功 / 跳过 / 失败，各自非空才出现（见 `Report::push_list_block`）；
    /// 4. 整块内容：`标签:` 一行 + 原文原样输出（日志、配置、帮助文本这类不能被逐行加前缀）；
    /// 5. 各块之间用空行分隔，拼成完整回复；
    /// 6. 截断：超过 `runtime.max_reply_chars` 就从末尾开始按行丢弃，并注明省略了多少行
    ///    Discord 对单条消息有硬上限（2000个utf16字符），超长会发送失败（用户那边看起来"什么都没发生"）。
    ///    截断时先把截断提示的长度算进去，所以上限装得下"第一行 + 说明"，结果就一定在上限之内。
    ///    长度口径见 `Utf16Len::utf16_len`（UTF-16 是 Discord 的口径）。
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

        // 3. 明细小节：成功 / 跳过 / 排队 / 失败，各自非空才出现（标签自带括号，见 push_list_block）。
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
        Self::push_list_block(&mut blocks, text::get().report_queued_label(), &self.queued);
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
