use crate::config::Config;
use crate::runtime::{
    bot_instances::{
        bot_dirs::{BotDirEntry, all_bot_dir_entries, bot_dir_entries_of_owner},
        bot_sessions::split_bot_entries_by_is_running,
    },
    bot_startup::StartupQueue,
};

/// 返回调用者有权访问的机器人状态，按运行中、排队中、已停止排列。
pub(crate) fn list_bot_candidates(caller_id: u64) -> Result<Vec<(String, bool, bool)>, String> {
    let entries = if Config::get().admins.is_admin(caller_id) {
        all_bot_dir_entries()?
    } else {
        bot_dir_entries_of_owner(caller_id)?
    };
    let (running, not_running) = split_bot_entries_by_is_running(&entries)?;
    let mut queued = Vec::new();
    let mut stopped = Vec::new();
    for (entry, in_queue) in StartupQueue::get_or_init().bots_in_queue(&not_running) {
        if in_queue {
            queued.push(entry);
        } else {
            stopped.push(entry);
        }
    }

    Ok(bot_names_sorted(running)
        .into_iter()
        .map(|name| (name, true, false))
        .chain(
            bot_names_sorted(queued)
                .into_iter()
                .map(|name| (name, false, true)),
        )
        .chain(
            bot_names_sorted(stopped)
                .into_iter()
                .map(|name| (name, false, false)),
        )
        .collect())
}

fn bot_names_sorted(entries: Vec<BotDirEntry>) -> Vec<String> {
    let mut names: Vec<String> = entries.into_iter().map(|entry| entry.bot_name).collect();
    names.sort();
    names
}
