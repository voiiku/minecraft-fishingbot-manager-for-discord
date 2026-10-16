//! `/show status`：汇总机器人状态（在跑的与还在启动队列里排队的）。
//!
//! 表头与数据行由 `utils::running_bots` 的同一组列定义生成。

use crate::text;
use crate::utils::{
    Report, RunningBotInfo, StartupQueue, all_bot_dir_entries, all_running_bot_infos,
    split_bot_entries_by_token_status, status_table_header,
};

/// 汇总机器人状态（`/show status` 的数据来源就是它）。
///
/// 汇总已经配置 Token 的机器人：在跑的进状态表，还在启动队列里排队的在报告下方 `[排队中]` 一栏。
pub(crate) fn all_running_bots_status() -> Result<String, String> {
    let i18n_text = text::get();
    let bot_infos = all_running_bot_infos()?;

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
    if bot_infos.is_empty() && queued_bot_names.is_empty() {
        report
            .basics
            .push(i18n_text.status_no_running_bots().to_string());
        return Ok(report.render());
    }

    // 在跑的机器人：表头与数据行使用同一组列定义。
    if !bot_infos.is_empty() {
        report
            .basics
            .push(i18n_text.common_total_count(bot_infos.len()));
        report.basics.push(status_table_header());
        // 消费快照并渲染数据行。
        report
            .basics
            .extend(bot_infos.into_iter().map(RunningBotInfo::into_row));
    }

    // 排队中的机器人尚无会话，无法进入上表（没有在线 / 暂停 / 时长可报），在报告下方单独列出。
    report.queued.extend(queued_bot_names);

    Ok(report.render())
}
