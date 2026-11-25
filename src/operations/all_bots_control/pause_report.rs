use crate::config::Config;
use crate::{
    runtime::bot_instances::{
        bot_sessions::token_ready_bot_entries_with_screen_sessions,
        fishing_pause::set_fishing_pause,
    },
    text::report::Report,
};

pub(super) fn set_all_bots_fishing_pause(paused: bool) -> Result<String, String> {
    let bots = token_ready_bot_entries_with_screen_sessions()?;
    let i18n = Config::get().text();
    let mut report = Report::new(
        Config::get().i18n.language,
        Config::get().runtime.max_reply_chars,
    );
    report.set_title(match paused {
        true => i18n.pauseall_title(),
        false => i18n.unpauseall_title(),
    });
    for bot in &bots {
        match set_fishing_pause(bot, paused) {
            Ok(message) => report.succeeded.push(message),
            Err(message) => report.failed.push(message),
        }
    }
    report.set_counts(&[
        (i18n.count_running_accounts(), bots.len()),
        (i18n.count_processed_successfully(), report.succeeded.len()),
        (i18n.count_processing_failed(), report.failed.len()),
    ]);
    Ok(report.render())
}
