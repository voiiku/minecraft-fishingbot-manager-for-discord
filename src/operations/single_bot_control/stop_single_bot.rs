use crate::config::Config;
use crate::operations::PreparedControl;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;
use crate::runtime::{
    bot_instances::{screen::is_bot_screen_running, stop::stop_single_bot_session},
    bot_startup::StartupQueue,
};

pub fn stop_single_bot(name: &str, caller_id: u64) -> Result<String, String> {
    prepare_stop_single_bot(name, caller_id)?.execute()
}

pub(crate) fn prepare_stop_single_bot(
    name: &str,
    caller_id: u64,
) -> Result<PreparedControl, String> {
    PreparedControl::for_bot(
        name,
        caller_id,
        |text| text.command_started_notice(&format!("/stop {name}")),
        execute,
    )
}

fn execute(bot_dir_entry: BotDirEntry) -> Result<String, String> {
    let text = Config::get().text();
    let bot_name: &str = &bot_dir_entry.bot_name;

    // 请求可能已出队，取消失败时继续检查会话。
    let cancelled =
        StartupQueue::get_or_init().remove_bots_from_queue(std::slice::from_ref(&bot_dir_entry));
    if cancelled.iter().any(|(_, removed)| *removed) {
        return Ok(text.bot_stop_cancelled_queue(bot_name));
    }

    if !is_bot_screen_running(bot_name)? {
        return Err(text.bot_stop_not_running(bot_name));
    }

    stop_single_bot_session(&bot_dir_entry)?;

    log::info!("{}", text.bot_stopped(bot_name));
    Ok(text.bot_stop_succeeded(bot_name))
}
