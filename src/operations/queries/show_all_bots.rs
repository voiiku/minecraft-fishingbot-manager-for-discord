use crate::runtime::bot_instances::bot_dirs::all_bot_dir_entries;
use crate::text::report::Report;

/// 列出所有机器人及其 Token 状态。
pub fn show_all_bots() -> Result<String, String> {
    let i18n_text = crate::config::Config::get().text();
    // 读取机器人目录。
    let bot_dir_entries = all_bot_dir_entries()?;

    let mut report = Report::new(
        crate::config::Config::get().i18n.language,
        crate::config::Config::get().runtime.max_reply_chars,
    );
    report.set_title(i18n_text.allbots_title());
    // 计数行只有这里输出「扫描机器人目录」：它说明这次看到多少台，其余指令的报告不带这个字段。
    report.set_counts(&[(i18n_text.count_scanned_dirs(), bot_dir_entries.len())]);

    if bot_dir_entries.is_empty() {
        report.basics.push(i18n_text.allbots_empty().to_string());
        return Ok(report.render());
    }

    // 统计 Token 状态；明细保持目录顺序。
    let ready = bot_dir_entries
        .iter()
        .filter(|bot| bot.bot_token_status.is_ready())
        .count();

    report
        .basics
        .push(i18n_text.allbots_ready_token_count(ready));
    report
        .basics
        .push(i18n_text.allbots_missing_token_count(bot_dir_entries.len() - ready));
    report
        .basics
        .extend(bot_dir_entries.iter().map(|bot_dir_entry| {
            i18n_text.allbots_row(
                &bot_dir_entry.bot_name,
                bot_dir_entry.owner_id,
                &bot_dir_entry.bot_token_status.to_string(),
            )
        }));

    Ok(report.render())
}
