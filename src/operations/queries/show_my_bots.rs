use crate::runtime::bot_instances::bot_dirs::bot_dir_entries_of_owner;
use crate::text::report::Report;

/// 列出调用者名下的机器人及其 Token 状态。
pub fn show_my_bots(caller_id: u64) -> Result<String, String> {
    let i18n_text = crate::config::Config::get().text();
    let bot_dir_entries = bot_dir_entries_of_owner(caller_id)?;
    let mut report = Report::new(
        crate::config::Config::get().i18n.language,
        crate::config::Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.mybots_title(caller_id));

    if bot_dir_entries.is_empty() {
        report.basics.push(i18n_text.mybots_empty().to_string());
        return Ok(report.render());
    }

    // Token 状态由目录条目提供。
    report
        .basics
        .push(i18n_text.common_total_count(bot_dir_entries.len()));
    report
        .basics
        .extend(bot_dir_entries.iter().map(|bot_dir_entry| {
            i18n_text.mybots_row(
                &bot_dir_entry.bot_name,
                &bot_dir_entry.bot_token_status.to_string(),
            )
        }));

    Ok(report.render())
}
