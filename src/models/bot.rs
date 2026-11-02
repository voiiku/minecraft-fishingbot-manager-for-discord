//! 机器人目录及凭据状态。

use std::path::PathBuf;

/// 机器人目录中 Token 文件的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotTokenStatus {
    /// Token 文件存在，不保证其有效。
    Ready,
    /// Token 文件不存在。
    Missing,
}

impl BotTokenStatus {
    /// 是否已经可以启动。
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// 机器人目录条目及创建时的 Token 状态。
#[derive(Debug, Clone)]
pub struct BotDirEntry {
    /// 所属 Discord 用户 ID。
    pub owner_id: u64,
    /// 机器人目录名称。
    pub bot_name: String,
    /// 机器人目录路径。
    pub bot_dir_path: PathBuf,
    /// 创建条目时的 Token 状态。
    pub bot_token_status: BotTokenStatus,
}
