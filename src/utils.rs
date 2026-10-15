//! 项目公共工具函数。
//!
//! 此模块只依赖 `config`（读取配置）和`text`(文本)，不依赖更上层的业务模块。
/// 单个附件的字节上限：Discord 普通上传上限是 10 MiB，这里留出余量。
pub const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;

mod bot_command;
mod bot_dirs;
mod bot_log_files;
mod bot_name;
mod bot_status;
mod fishingbot_config;
mod fs;
mod permission;
mod reply;
mod restart;
mod running_bots;
mod screen;
mod shutdown;
mod startup;
mod uptime;

pub use bot_command::ToCleanBotCommand;
pub use bot_dirs::{
    BotDirEntry, all_bot_dir_entries, bot_dir_entries_of_owner, create_bot_dir, is_bot_name_taken,
    split_bot_entries_by_token_status, token_file_path,
};
pub use bot_log_files::{create_timestamped_log_file, latest_log_path};
pub use bot_name::ToCleanBotName;
pub use bot_status::{PauseCommand, confirm_pause_command};
pub use fishingbot_config::regenerate_fishingbot_config;
pub use fs::{read_last_lines, remove_dir_all_if_exists};
pub use permission::{RequiredPermission, has_permission};
pub use reply::{IntoReplyText, Report};
pub use restart::restart_bot_sessions;
pub use running_bots::{
    RunningBotInfo, all_running_bot_infos, all_running_bot_names, refusal_if_running,
    split_bot_entries_by_is_running, status_table_header,
};
pub use screen::{is_bot_screen_running, send_command_to_single_bot};
pub use shutdown::{stop_all_bot_sessions, stop_single_bot_session};
pub use startup::{prepare_and_launch_bot, start_bot_sessions};
pub use uptime::format_uptime;
