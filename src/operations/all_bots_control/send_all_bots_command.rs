use crate::config::Config;
use crate::operations::{PreparedControl, require_admin};
use crate::runtime::bot_instances::{
    bot_sessions::token_ready_bot_names_with_screen_sessions, screen::send_command_to_single_bot,
};
use crate::{text::report::Report, utils::console_command::ToCleanBotCommand};

pub fn send_all_bots_command(
    raw_bot_command_args: &[&str],
    caller_id: u64,
) -> Result<String, String> {
    prepare_send_all_bots_command(raw_bot_command_args, caller_id)?.execute()
}

pub(crate) fn prepare_send_all_bots_command(
    raw_bot_command_args: &[&str],
    caller_id: u64,
) -> Result<PreparedControl, String> {
    require_admin(caller_id)?;
    let notice = Config::get()
        .text()
        .command_started_notice(&format!("/commandall {}", raw_bot_command_args.join(" ")));
    let command = raw_bot_command_args.to_clean_bot_command();
    Ok(PreparedControl::new(notice, move || execute(command)))
}

fn execute(command: Option<String>) -> Result<String, String> {
    let bot_command =
        command.ok_or_else(|| Config::get().text().bot_command_empty().to_string())?;

    let bot_names = token_ready_bot_names_with_screen_sessions()?;
    let running_bot_count = bot_names.len();

    let mut succeeded_bot_count = 0usize;
    let mut failed_messages: Vec<String> = Vec::new();
    for bot_name in bot_names {
        match send_command_to_single_bot(&bot_name, &bot_command) {
            Ok(_) => succeeded_bot_count += 1,
            Err(message) => failed_messages.push(message),
        }
    }

    let i18n_text = Config::get().text();
    let mut report = Report::new(
        Config::get().i18n.language,
        Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.commandall_title());
    report.set_counts(&[
        (i18n_text.count_running_accounts(), running_bot_count),
        (i18n_text.count_sent_ok(), succeeded_bot_count),
        (i18n_text.count_send_failed(), failed_messages.len()),
    ]);
    report
        .basics
        .insert(1, i18n_text.commandall_sending(&bot_command));
    report.failed = failed_messages;

    Ok(report.render())
}
