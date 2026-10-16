//! `/stopall`：批量停止所有运行中的机器人（仅管理员）。

use crate::text;
use crate::utils::{BotDirEntry, StartupQueue, all_bot_dir_entries, stop_all_bot_sessions};

/// 停止所有运行中的机器人，包含缺少 Token 的会话。
pub(crate) fn stop_all_bots() -> Result<String, String> {
    let i18n_text = text::get();
    let startup_queue = StartupQueue::get_or_init();

    // 先取排队待启动的机器人名：它们尚无会话，清空队列由 `stop_all_bot_sessions()` 完成（见该函数），
    // 这里取名字用于在报告中列出被取消的启动。
    let queued_bots: Vec<BotDirEntry> = startup_queue
        .bots_in_queue(&all_bot_dir_entries()?)
        .into_iter()
        .filter_map(|(bot_dir_entry, in_queue)| in_queue.then_some(bot_dir_entry))
        .collect();

    let mut report = stop_all_bot_sessions()?.into_report();
    report.set_title(i18n_text.stopall_title());
    report.skipped.extend(
        queued_bots
            .iter()
            .map(|bot_dir_entry| i18n_text.bot_stop_cancelled_queue(&bot_dir_entry.bot_name)),
    );

    Ok(report.render())
}
