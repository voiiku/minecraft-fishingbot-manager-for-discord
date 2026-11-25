use crate::config::Config;
use crate::operations::PreparedControl;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;
use crate::runtime::bot_instances::screen::{is_bot_screen_running, send_command_to_single_bot};
use crate::utils::console_command::ToCleanBotCommand;

pub fn send_single_bot_command(
    name: &str,
    raw_bot_command_args: &[&str],
    caller_id: u64,
) -> Result<String, String> {
    prepare_send_single_bot_command(name, raw_bot_command_args, caller_id)?.execute()
}

pub(crate) fn prepare_send_single_bot_command(
    name: &str,
    raw_bot_command_args: &[&str],
    caller_id: u64,
) -> Result<PreparedControl, String> {
    let command = raw_bot_command_args.to_clean_bot_command();
    PreparedControl::for_bot(
        name,
        caller_id,
        |text| text.command_started_notice(&format!("/command {name}")),
        move |bot| execute(bot, command),
    )
}

fn execute(bot_dir_entry: BotDirEntry, command: Option<String>) -> Result<String, String> {
    let text = Config::get().text();
    let bot_name: &str = &bot_dir_entry.bot_name;
    let bot_command = command.ok_or_else(|| text.bot_command_empty().to_string())?;

    let session_exists =
        is_bot_screen_running(bot_name).map_err(|message| text.bot_line(bot_name, &message))?;
    if !session_exists {
        return Err(text.bot_command_not_running(bot_name));
    }

    send_command_to_single_bot(bot_name, &bot_command)?;

    Ok(text.bot_command_sent(bot_name, &bot_command))
}
