//! 按用途组织机器人、会话与操作结果类型。

mod bot;
mod outcome;
mod permission;
mod report;
mod session;
mod status;

pub use bot::{BotDirEntry, BotTokenStatus};
#[cfg(test)]
pub(crate) use outcome::BotStartEntry;
pub use outcome::{BotStartOutcome, BotStartResults};
pub(crate) use outcome::{RestartOutcome, StopOutcome};
pub use permission::RequiredPermission;
pub use report::Report;
pub use session::ScreenSession;
pub use status::{BotPauseStatus, ConnectionStatus, PauseCommand, PauseStatus};
