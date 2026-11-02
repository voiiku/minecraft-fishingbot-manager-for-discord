//! 机器人连接、暂停状态与暂停指令。

/// 机器人与游戏服务器的连接状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// 日志尚未出现状态关键词。
    Initializing,
    /// 已进入游戏服务器。
    Online,
    /// 进程存活，机器人正在重连。
    Reconnecting,
    /// JVM 已退出，会话仅剩死残壳。
    Stopped,
}

/// 机器人的暂停状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseStatus {
    /// 正在钓鱼。
    Fishing,
    /// 已暂停钓鱼。
    Paused,
}

/// 机器人暂停状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotPauseStatus {
    pause: PauseStatus,
}

impl Default for BotPauseStatus {
    fn default() -> Self {
        Self {
            pause: PauseStatus::Fishing,
        }
    }
}

impl BotPauseStatus {
    /// 返回当前暂停状态。
    pub fn pause_status(&self) -> PauseStatus {
        self.pause
    }

    /// 将状态设为暂停。
    pub fn set_paused(&mut self) {
        self.pause = PauseStatus::Paused;
    }

    /// 将状态设为钓鱼中。
    pub fn set_fishing(&mut self) {
        self.pause = PauseStatus::Fishing;
    }
}

/// 需要确认回显的暂停与恢复指令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCommand {
    /// 暂停钓鱼。
    SetPause,
    /// 恢复钓鱼。
    SetUnpause,
}

impl PauseCommand {
    /// 返回待注入的游戏指令。
    pub fn command(self) -> &'static str {
        match self {
            Self::SetPause => "/pause",
            Self::SetUnpause => "/unpause",
        }
    }

    /// 返回相反的暂停指令。
    pub(crate) fn other(self) -> Self {
        match self {
            Self::SetPause => Self::SetUnpause,
            Self::SetUnpause => Self::SetPause,
        }
    }

    /// 返回指令生效后的目标状态。
    pub(crate) fn target_pause_status(self) -> PauseStatus {
        match self {
            Self::SetPause => PauseStatus::Paused,
            Self::SetUnpause => PauseStatus::Fishing,
        }
    }

    /// 更新已确认的暂停状态。
    pub(crate) fn apply(self, status: &mut BotPauseStatus) {
        match self {
            Self::SetPause => status.set_paused(),
            Self::SetUnpause => status.set_fishing(),
        }
    }
}
