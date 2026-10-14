//! `/show status`：汇总当前运行中机器人的状态。
//!
//! 表头与数据行由 `utils::running_bots` 的同一组列定义生成。

use crate::text;
use crate::utils::{Report, RunningBotInfo, all_running_bot_infos, status_table_header};

/// 汇总运行中机器人的状态（`/show status` 的数据来源就是它）。
///
/// 汇总已配置 Token 且正在运行的机器人。
pub(crate) fn all_running_bots_status() -> Result<String, String> {
    let i18n_text = text::get();
    let bot_infos = all_running_bot_infos()?;

    let mut report = Report::new();
    report.set_title(i18n_text.status_title());

    // 没有运行中的机器人时不输出空表。
    if bot_infos.is_empty() {
        report
            .basics
            .push(i18n_text.status_no_running_bots().to_string());
        return Ok(report.render());
    }

    report
        .basics
        .push(i18n_text.common_total_count(bot_infos.len()));
    // 表头与数据行使用同一组列定义。
    report.basics.push(status_table_header());
    // 消费快照并渲染数据行。
    report
        .basics
        .extend(bot_infos.into_iter().map(RunningBotInfo::into_row));

    Ok(report.render())
}
