//! screen 会话状态。

/// screen 会话；死残壳仍视为存在，以便停止流程清理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSession {
    /// 会话对应的 screen 进程 PID。
    pub pid: u32,
    /// 会话是否被标记为死残壳。
    pub is_dead: bool,
}
