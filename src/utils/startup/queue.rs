//! 启动队列的内部工具函数。

use std::collections::VecDeque;
use std::sync::MutexGuard;
use std::time::Instant;

use super::{QueuedBot, StartupQueue};

impl StartupQueue {
    /// 向队列加入待启动的条目；容量不够时收下前面那部分（`[startup_queue] max_pending_requests`）。
    ///
    /// 返回没收下的那些（由调用方记进账本）；全部收下时返回 `Ok(())`。
    pub(super) fn add_waiting_bots(&self, entries: Vec<QueuedBot>) -> Result<(), Vec<QueuedBot>> {
        let rejected = {
            let mut waiting_bots = self.waiting_bots_mut();
            // 先来的先占位，超出的原样退回。
            let free_slots = self.max_pending_requests.saturating_sub(waiting_bots.len());
            let mut accepted = entries;
            let rejected = accepted.split_off(free_slots.min(accepted.len()));
            waiting_bots.extend(accepted);

            rejected
        };

        // 队列刚被写入 → 唤醒启动线程。
        self.added.notify_all();

        if rejected.is_empty() {
            Ok(())
        } else {
            Err(rejected)
        }
    }

    /// 等待队列非空或收到停止拉起请求（启动线程用；两者之一成立即返回）。
    ///
    /// 停止请求同样是唤醒条件：否则队列为空时"停止全部机器人"无法唤醒启动线程，
    /// 该请求会滞留到下一次 `/startall` 才生效（见 [`StartupQueue::request_stop`]）。
    pub(super) fn wait_until_work(&self) {
        let mut waiting_bots = self.waiting_bots_mut();
        while waiting_bots.is_empty() && !self.has_stop_request() {
            waiting_bots = self
                .added
                .wait(waiting_bots)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// 从队首取出最多 `bot_count` 台；取出即记下时刻（批间间隔从它算起）。
    pub(super) fn pop_bots_from_head(&self, bot_count: usize) -> Vec<QueuedBot> {
        let popped: Vec<QueuedBot> = {
            let mut waiting_bots = self.waiting_bots_mut();
            let take_count = bot_count.min(waiting_bots.len());

            waiting_bots.drain(..take_count).collect()
        };

        // 空取不算"取出一批"，不动时刻。
        if !popped.is_empty() {
            *self.latest_poped_timestamp_mut() = Instant::now();
        }

        popped
    }

    /// 最近一次取出的时刻（即上一批开始启动的时刻）。
    pub(super) fn latest_poped_timestamp(&self) -> Instant {
        *self.latest_poped_timestamp_mut()
    }

    /// 取待启动列表的锁（中毒的锁也要能用：这里只放数据，不存"必须一致"的不变量）。
    pub(super) fn waiting_bots_mut(&self) -> MutexGuard<'_, VecDeque<QueuedBot>> {
        self.waiting_bots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 取"最近一次取批时刻"的锁（同上）。
    fn latest_poped_timestamp_mut(&self) -> MutexGuard<'_, Instant> {
        self.latest_poped_timestamp
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;

    use crate::utils::bot_dirs::BotTokenStatus;

    use crate::utils::bot_dirs::BotDirEntry;
    use crate::utils::startup::StartLedger;

    use super::*;

    /// 队列只做队列交互，取值随意给（这批用例不考察节奏）。
    fn queue() -> StartupQueue {
        queue_with_capacity(32)
    }

    /// 同上，但把暂存容量收紧到 `max_pending_requests`。
    fn queue_with_capacity(max_pending_requests: usize) -> StartupQueue {
        StartupQueue::new_for_test(3, 20_000, 30_000, 300, max_pending_requests)
    }

    fn bot(bot_name: &str) -> BotDirEntry {
        BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: PathBuf::from("/nonexistent"),
            bot_token_status: BotTokenStatus::Missing,
        }
    }

    fn bot_names<'a>(bots: impl IntoIterator<Item = &'a BotDirEntry>) -> Vec<String> {
        bots.into_iter().map(|bot| bot.bot_name.clone()).collect()
    }

    /// 队列里现在排着的那几台（顺序就是先进先出顺序）。
    fn waiting_bots(queue: &StartupQueue) -> Vec<String> {
        bot_names(
            queue
                .waiting_bots_mut()
                .iter()
                .map(|queued_bot| queued_bot.bot()),
        )
    }

    /// 一个待启动条目（机器人与一个只等它的账本）。
    fn queued(bot_name: &str) -> QueuedBot {
        let bot_dir_entry = bot(bot_name);
        let ledger = StartLedger::new(std::slice::from_ref(&bot_dir_entry));

        QueuedBot::new(bot_dir_entry, ledger)
    }

    /// 直接把队列内容换成这几个名字。
    fn set_waiting_bots(queue: &StartupQueue, bot_names: &[&str]) {
        let entries: Vec<QueuedBot> = bot_names.iter().map(|name| queued(name)).collect();

        *queue.waiting_bots_mut() = entries.into();
    }

    /// 容量不够时：先来的收下，超出的原样退回（调用方要在回复里说明这部分）。
    #[test]
    fn bots_beyond_the_capacity_are_handed_back() {
        let queue = queue_with_capacity(2);

        let rejected = queue
            .add_waiting_bots(vec![queued("duck"), queued("goose"), queued("crane")])
            .expect_err("超出容量的部分要退回来");

        assert_eq!(waiting_bots(&queue), ["duck", "goose"]);
        assert_eq!(
            bot_names(rejected.iter().map(|queued_bot| queued_bot.bot())),
            ["crane"]
        );
    }

    /// 已经排了人时按剩余空位收，剩下的照旧退回。
    #[test]
    fn already_waiting_bots_consume_the_capacity() {
        let queue = queue_with_capacity(2);
        queue
            .add_waiting_bots(vec![queued("duck")])
            .expect("第一次收得下");

        let rejected = queue
            .add_waiting_bots(vec![queued("goose"), queued("crane")])
            .expect_err("只剩一个空位");

        assert_eq!(waiting_bots(&queue), ["duck", "goose"]);
        assert_eq!(
            bot_names(rejected.iter().map(|queued_bot| queued_bot.bot())),
            ["crane"]
        );
    }

    /// 从队首取：先进先出；不够就少给，不留空位。
    #[test]
    fn popping_takes_the_oldest_bots_first() {
        let queue = queue();
        assert!(queue.pop_bots_from_head(2).is_empty(), "空队列取不到东西");

        set_waiting_bots(&queue, &["duck", "goose", "crane"]);

        assert_eq!(
            bot_names(
                queue
                    .pop_bots_from_head(2)
                    .iter()
                    .map(|queued_bot| queued_bot.bot())
            ),
            ["duck", "goose"]
        );
        assert_eq!(queue.waiting_bots_mut().len(), 1);
        assert_eq!(
            bot_names(
                queue
                    .pop_bots_from_head(5)
                    .iter()
                    .map(|queued_bot| queued_bot.bot())
            ),
            ["crane"],
            "只剩一台就只给一台"
        );
        assert!(queue.pop_bots_from_head(1).is_empty());
    }

    /// 取出的时刻会被记下来；只有真的取到东西时更新（空取不算一批）。
    #[test]
    fn only_a_real_pop_updates_the_timestamp() {
        let queue = queue();
        let loaded_at = queue.latest_poped_timestamp();

        queue.pop_bots_from_head(1);
        assert_eq!(queue.latest_poped_timestamp(), loaded_at, "空取不该动时刻");

        set_waiting_bots(&queue, &["duck"]);
        queue.pop_bots_from_head(1);
        let first_pop = queue.latest_poped_timestamp();
        assert!(first_pop >= loaded_at);

        set_waiting_bots(&queue, &["goose"]);
        queue.pop_bots_from_head(1);
        let second_pop = queue.latest_poped_timestamp();
        assert!(second_pop >= first_pop, "再取一次要更新");
    }

    /// 查询：队列里有没有这几台（外部命令判断"正在排队启动"用它）。
    #[test]
    fn the_queue_reports_which_bots_are_waiting() {
        let queue = queue();
        set_waiting_bots(&queue, &["duck", "goose"]);

        assert!(queue.bot_in_queue(&bot("duck")));
        assert!(!queue.bot_in_queue(&bot("crane")));

        let answers = queue.bots_in_queue(&[bot("duck"), bot("crane"), bot("goose")]);
        let flags: Vec<bool> = answers.into_iter().map(|(_, in_queue)| in_queue).collect();

        assert_eq!(flags, [true, false, true]);
    }

    /// 撤单：撤到的报 `true`，已经不在队列里的（可能刚被拉起）报 `false`，整体不失败。
    #[test]
    fn removing_bots_from_the_queue_answers_per_bot() {
        let queue = queue();
        set_waiting_bots(&queue, &["duck", "goose"]);

        let answers = queue.remove_bots_from_queue(&[bot("duck"), bot("crane")]);
        let flags: Vec<bool> = answers.into_iter().map(|(_, removed)| removed).collect();

        assert_eq!(flags, [true, false], "crane 本来就不在队列里");
        assert_eq!(waiting_bots(&queue), ["goose"]);
    }

    /// 清空队列（`/stopall`）。
    #[test]
    fn clearing_the_queue_empties_it() {
        let queue = queue();
        set_waiting_bots(&queue, &["duck", "goose"]);

        queue.remove_all_bots_from_queue();

        assert!(waiting_bots(&queue).is_empty());
    }

    /// 停止请求也能叫醒等待中的启动线程（队列空着时只靠它），并且取走一次就没了。
    #[test]
    fn a_stop_request_wakes_the_waiting_starter_once() {
        let queue = Arc::new(queue());
        let waiting_queue = Arc::clone(&queue);
        let waiter = thread::spawn(move || {
            waiting_queue.wait_until_work();
            waiting_queue.take_stop_request()
        });

        // 队列是空的：这一次唤醒只可能来自停止请求。
        queue.request_stop();

        assert!(waiter.join().expect("等待线程不该挂"), "取到停止请求");
        assert!(!queue.take_stop_request(), "取走之后不再有");
    }
}
