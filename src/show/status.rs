//! `/show status`：汇总机器人状态（在跑的与还在启动队列里排队的）。
//!
//! 表头一行在本模块就地拼（列名依次取自 `text`，用 `" | "` 连接），
//! 数据行由 `utils::running_bots` 的 [`all_running_bot_status_rows`] 给出，两者列序一致。

use crate::text;
use crate::utils::{
    Report, StartupQueue, all_bot_dir_entries, all_running_bot_status_rows,
    split_bot_entries_by_token_status,
};

/// 汇总机器人状态（`/show status` 的数据来源就是它）。
///
/// 汇总已经配置 Token 的机器人：在跑的进状态表，还在启动队列里排队的在报告下方 `[排队中]` 一栏。
pub(crate) fn all_running_bots_status() -> Result<String, String> {
    let i18n_text = text::get();
    let status_rows = all_running_bot_status_rows()?;

    // 是否排队以启动队列为准（与 `/start`、`/stop` 同一口径）：仅查询一次，且与状态表使用同一批机器人。
    let (token_ready_bot_entries, _) = split_bot_entries_by_token_status(&all_bot_dir_entries()?);
    let queued_bot_names: Vec<String> = StartupQueue::get_or_init()
        .bots_in_queue(&token_ready_bot_entries)
        .into_iter()
        .filter_map(|(bot_dir_entry, in_queue)| in_queue.then_some(bot_dir_entry.bot_name))
        .collect();

    let mut report = Report::new();
    report.set_title(i18n_text.status_title());

    // 既没有运行中、也没有排队中的机器人时不输出空表。
    if status_rows.is_empty() && queued_bot_names.is_empty() {
        report
            .basics
            .push(i18n_text.status_no_running_bots().to_string());
        return Ok(report.render());
    }

    // 在跑的机器人：表头就地拼，数据行与它同序（同一组列名、同一个分隔符）。
    if !status_rows.is_empty() {
        report
            .basics
            .push(i18n_text.common_total_count(status_rows.len()));
        report.basics.push(
            [
                i18n_text.status_column_bot_name(),
                i18n_text.status_column_connection(),
                i18n_text.status_column_pause(),
                i18n_text.status_column_uptime(),
            ]
            .join(" | "),
        );
        report.basics.extend(status_rows);
    }

    // 排队中的机器人尚无会话，无法进入上表（没有在线 / 暂停 / 时长可报），在报告下方单独列出。
    report.queued.extend(queued_bot_names);

    Ok(report.render())
}
