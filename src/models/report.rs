//! 报告数据。

/// 通用报告；空列表和未提供的字段在渲染时省略。
#[derive(Debug)]
pub struct Report {
    /// 标题；`None` 时省略标题与分隔线。
    pub title: Option<String>,
    /// 基础信息行与列表行。
    pub basics: Vec<String>,
    /// 已完成处理的明细，状态可能仍未确认。
    pub succeeded: Vec<String>,
    /// 跳过明细。
    pub skipped: Vec<String>,
    /// 排队明细。
    pub queued: Vec<String>,
    /// 失败或未完成处理的明细。
    pub failed: Vec<String>,
    /// 原样内容。
    pub content: Option<String>,
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

impl Report {
    /// 创建空报告。
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
}
