use crate::config::Config;
use crate::operations::PreparedControl;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;
use crate::runtime::{
    bot_instances::{restart::restart_bot_sessions, screen::is_bot_screen_running},
    bot_startup::StartupQueue,
};

pub fn restart_single_bot(name: &str, caller_id: u64) -> Result<String, String> {
    prepare_restart_single_bot(name, caller_id)?.execute()
}

pub(crate) fn prepare_restart_single_bot(
    name: &str,
    caller_id: u64,
) -> Result<PreparedControl, String> {
    PreparedControl::for_bot(
        name,
        caller_id,
        |text| text.command_start_queued_notice(&format!("/restart {name}")),
        execute,
    )
}

fn execute(bot_dir_entry: BotDirEntry) -> Result<String, String> {
    let text = Config::get().text();
    let bot_name: &str = &bot_dir_entry.bot_name;

    if !is_bot_screen_running(bot_name)? {
        if StartupQueue::get_or_init().bot_in_queue(&bot_dir_entry) {
            return Err(text.bot_start_already_queued(bot_name));
        }

        return Err(text.restart_not_running(bot_name));
    }

    log::info!("{}", text.bot_restart_requested(bot_name));
    let outcome = restart_bot_sessions(std::slice::from_ref(&bot_dir_entry))?;
    if !outcome.failures.is_empty() {
        return Err(outcome.failures.join(" / "));
    }
    // 启动请求可能被并发的停止操作取消。
    if !outcome.cancelled.is_empty() {
        return Err(text.start_cancelled(bot_name));
    }

    let (_, status) = outcome
        .verified
        .first()
        .ok_or_else(|| text.start_failed(bot_name))?;
    Ok(if status.joined {
        text.bot_restart_online_confirmed(bot_name)
    } else {
        text.bot_restart_not_joined(bot_name)
    })
}
