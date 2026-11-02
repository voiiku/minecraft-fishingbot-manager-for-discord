//! 批量暂停或恢复钓鱼，逐台检查 JAR 回显。

use crate::handlers::{pause_fishing, unpause_fishing};
use crate::text;
use crate::utils::{PauseCommand, Report, token_ready_bot_entries_with_screen_sessions};

pub(crate) fn set_all_bots_fishing_pause(pause_command: PauseCommand) -> Result<String, String> {
    let bots = token_ready_bot_entries_with_screen_sessions()?;
    let i18n_text = text::get();
    let mut report = Report::new();
    report.set_title(match pause_command {
        PauseCommand::SetPause => i18n_text.pauseall_title(),
        PauseCommand::SetUnpause => i18n_text.unpauseall_title(),
    });

    for bot in &bots {
        let result = match pause_command {
            PauseCommand::SetPause => pause_fishing(bot),
            PauseCommand::SetUnpause => unpause_fishing(bot),
        };
        match result {
            Ok(message) => report.succeeded.push(message),
            Err(message) => report.failed.push(message),
        }
    }

    report.set_counts(&[
        (i18n_text.count_running_accounts(), bots.len()),
        (
            i18n_text.count_processed_successfully(),
            report.succeeded.len(),
        ),
        (i18n_text.count_processing_failed(), report.failed.len()),
    ]);
    Ok(report.render())
}
