use crate::config::Config;
use crate::operations::PreparedControl;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;
use crate::runtime::{
    bot_instances::screen::is_bot_screen_running,
    bot_startup::{StartIssue, StartupQueue, start_status_line},
};

pub fn start_single_bot(name: &str, caller_id: u64) -> Result<String, String> {
    prepare_start_single_bot(name, caller_id)?.execute()
}

pub(crate) fn prepare_start_single_bot(
    name: &str,
    caller_id: u64,
) -> Result<PreparedControl, String> {
    PreparedControl::for_bot(
        name,
        caller_id,
        |text| text.command_start_queued_notice(&format!("/start {name}")),
        execute,
    )
}

fn execute(bot_dir_entry: BotDirEntry) -> Result<String, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;
    let i18n_text = Config::get().text();

    if !bot_dir_entry.bot_token_status.is_ready() {
        return Err(i18n_text.bot_start_missing_token(bot_name));
    }
    if is_bot_screen_running(bot_name)? {
        return Err(i18n_text.bot_start_already_running(bot_name));
    }
    let startup_queue = StartupQueue::get_or_init();

    if startup_queue.bot_in_queue(&bot_dir_entry) {
        return Err(i18n_text.bot_start_already_queued(bot_name));
    }

    let mut results = startup_queue.start_bots(std::slice::from_ref(&bot_dir_entry));
    match results.pop().map(|(_, result)| result) {
        Some(Ok(status)) => Ok(start_status_line(bot_name, status)),
        Some(Err(StartIssue::AlreadyRunning)) => Err(i18n_text.bot_start_already_running(bot_name)),
        Some(Err(StartIssue::Cancelled)) => Err(i18n_text.start_cancelled(bot_name)),
        Some(Err(StartIssue::Failure(error))) => Err(error),
        None => Err(i18n_text.start_failed(bot_name)),
    }
}
