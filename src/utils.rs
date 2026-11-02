//! 项目公共工具函数，以及共享运行时组件的再导出。
/// 单个附件上限，低于 Discord 的 10 MiB 普通上传上限。
pub const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;

mod bot_command;
mod bot_dirs;
mod bot_logs;
mod bot_name;
mod bot_status;
mod fishingbot_config;
mod fs;
mod permission;
mod raw_logs;
mod reply;
mod restart;
mod running_bots;
mod screen;
mod shutdown;
mod start_results;
mod uptime;

pub use crate::runtime::ProcessLog;
pub use crate::runtime::startup::StartupQueue;
pub use bot_command::ToCleanBotCommand;
#[cfg(test)]
pub(crate) use bot_dirs::BotTokenStatus;
pub use bot_dirs::{
    BotDirEntry, all_bot_dir_entries, bot_dir_entries_of_owner, create_bot_dir, is_bot_name_taken,
    split_bot_entries_by_token_status, token_file_path,
};
pub(crate) use bot_logs::create_bot_log_file;
pub use bot_logs::{create_timestamped_log_file, latest_log_path};
pub use bot_name::ToCleanBotName;
pub(crate) use bot_status::{BotPauseStatusMemory, ConnectionStatus};
pub use bot_status::{PauseCommand, confirm_pause_command};
pub use fishingbot_config::regenerate_fishingbot_config;
pub use fs::{read_last_lines, remove_dir_all_if_exists};
pub use permission::{RequiredPermission, has_permission};
pub(crate) use raw_logs::reset_fishingbot_raw_logs_dir;
pub use reply::{IntoReplyText, Report};
pub(crate) use restart::restart_bot_sessions;
pub use running_bots::{
    all_running_bot_names, refusal_if_not_stopped, split_bot_entries_by_is_running,
    token_ready_bot_names_with_screen_sessions,
};
pub(crate) use running_bots::{status_row, token_ready_bot_entries_with_screen_sessions};
pub use screen::{is_bot_screen_running, send_command_to_single_bot};
pub(crate) use screen::{screen_session_snapshot, session_name};
pub(crate) use shutdown::{stop_all_bot_sessions, stop_bot_sessions, stop_single_bot_session};
#[cfg(test)]
pub(crate) use start_results::BotStartEntry;
pub(crate) use start_results::{BotStartOutcome, BotStartResults};
pub use uptime::{format_uptime, process_elapsed_seconds};
