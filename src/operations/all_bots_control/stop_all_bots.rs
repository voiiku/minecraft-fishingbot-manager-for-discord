use crate::config::Config;
use crate::operations::{PreparedControl, require_admin};
use crate::runtime::{
    bot_instances::{
        bot_dirs::{BotDirEntry, all_bot_dir_entries},
        stop::stop_all_bot_sessions,
    },
    bot_startup::StartupQueue,
};

pub fn stop_all_bots(caller_id: u64) -> Result<String, String> {
    prepare_stop_all_bots(caller_id)?.execute()
}

pub(crate) fn prepare_stop_all_bots(caller_id: u64) -> Result<PreparedControl, String> {
    require_admin(caller_id)?;
    let notice = Config::get().text().command_started_notice("/stopall");
    Ok(PreparedControl::new(notice, execute))
}

fn execute() -> Result<String, String> {
    let i18n_text = Config::get().text();
    let startup_queue = StartupQueue::get_or_init();

    // 清空队列前保留待启动名单，供报告使用。
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
