use crate::runtime::{
    anti_afk::is_anti_afk_running,
    bot_instances::{
        bot_dirs::{BotDirEntry, token_ready_bot_dir_entries},
        bot_status::query_bot_status,
        screen::{ScreenSession, screen_session_snapshot, session_name},
    },
    bot_startup::StartupQueue,
};
use crate::utils::uptime::{format_uptime, process_elapsed_seconds};
use crate::{config, text::report::Report};

fn status_row(bot: &BotDirEntry, session: &ScreenSession) -> String {
    let t = crate::config::Config::get().text();
    let separator = config::Config::get().constants.status_column_separator;
    let missing = |reason, field| [reason, field, field, field, field].join(separator);
    let fields = if session.is_dead {
        missing(t.bot_status_stopped(), t.bot_status_field_unavailable())
    } else {
        match query_bot_status(bot, None) {
            Ok(Some(status)) => {
                let joined = if status.joined {
                    t.bot_status_joined()
                } else {
                    t.bot_status_not_joined()
                };
                let paused = if status.paused {
                    t.bot_pause_paused()
                } else {
                    t.bot_pause_unpaused()
                };
                [
                    joined.to_string(),
                    paused.to_string(),
                    status.no_rod_available.to_string(),
                    status.has_bobber.to_string(),
                    status.last_fish.to_string(),
                ]
                .join(separator)
            }
            Ok(None) => missing(t.bot_status_unavailable(), t.bot_status_field_unavailable()),
            Err(reason) => missing(&reason, t.bot_status_field_unknown()),
        }
    };
    let uptime = format_uptime(process_elapsed_seconds(session.pid).unwrap_or(0));
    [bot.bot_name.as_str(), &fields, &uptime].join(separator)
}

/// 汇总机器人状态（`/show status`）。
pub fn show_status() -> Result<String, String> {
    let i18n_text = crate::config::Config::get().text();
    let active_sessions = screen_session_snapshot()?;
    let token_ready_bot_entries = token_ready_bot_dir_entries()?;
    let status_rows: Vec<String> = token_ready_bot_entries
        .iter()
        .filter_map(|entry| {
            let session = active_sessions.get(&session_name(&entry.bot_name).ok()?)?;
            Some(status_row(entry, session))
        })
        .collect();
    let queued_bot_names: Vec<String> = StartupQueue::get_or_init()
        .bots_in_queue(&token_ready_bot_entries)
        .into_iter()
        .filter_map(|(bot_dir_entry, in_queue)| in_queue.then_some(bot_dir_entry.bot_name))
        .collect();

    let mut report = Report::new(
        crate::config::Config::get().i18n.language,
        crate::config::Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.status_title());
    report.basics.push(
        i18n_text
            .anti_afk_status(is_anti_afk_running()?)
            .to_string(),
    );

    if status_rows.is_empty() && queued_bot_names.is_empty() {
        report
            .basics
            .push(i18n_text.status_no_running_bots().to_string());
        return Ok(report.render());
    }

    if !status_rows.is_empty() {
        report
            .basics
            .push(i18n_text.common_total_count(status_rows.len()));
        report.basics.push(
            [
                i18n_text.status_column_bot_name(),
                i18n_text.status_column_joined(),
                i18n_text.status_column_pause(),
                i18n_text.status_column_no_rod(),
                i18n_text.status_column_bobber(),
                i18n_text.status_column_last_fish(),
                i18n_text.status_column_uptime(),
            ]
            .join(config::Config::get().constants.status_column_separator),
        );
        report.basics.extend(status_rows);
    }

    report.queued.extend(queued_bot_names);

    Ok(report.render())
}
