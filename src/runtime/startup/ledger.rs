//! 启动结果账本：启动线程记录结果，全部完成后唤醒调用方复核。

use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::text;

use super::StartupQueue;

use crate::utils::BotDirEntry;

use super::{BotStartOutcome, BotStartResults};

/// 账本的内部状态。
#[derive(Debug, Default)]
struct LedgerState {
    /// 等待结果的机器人名称。
    pending_bot_names: HashSet<String>,
    /// 已记录的结果，按完成顺序排列。
    results: BotStartResults,
}

/// 一次启动请求的结果账本。
#[derive(Debug)]
pub struct StartLedger {
    /// 本次请求的机器人。
    bots: Vec<BotDirEntry>,
    state: Mutex<LedgerState>,
    /// 全部结果完成时唤醒调用方。
    done: Condvar,
}

impl StartLedger {
    /// 创建指定机器人的启动结果账本。
    pub(crate) fn new(bots: &[BotDirEntry]) -> Arc<Self> {
        let pending_bot_names = bots.iter().map(|bot| bot.bot_name.clone()).collect();

        Arc::new(Self {
            bots: bots.to_vec(),
            state: Mutex::new(LedgerState {
                pending_bot_names,
                results: BotStartResults::default(),
            }),
            done: Condvar::new(),
        })
    }

    /// 返回本次请求的机器人。
    pub(super) fn bots(&self) -> &[BotDirEntry] {
        &self.bots
    }

    /// 记录单个结果，全部完成时唤醒调用方。
    pub(super) fn record(&self, bot_dir_entry: BotDirEntry, outcome: BotStartOutcome) {
        let mut state = self.state_mut();
        state.pending_bot_names.remove(&bot_dir_entry.bot_name);
        state.results.push(bot_dir_entry, outcome);
        let is_done = state.pending_bot_names.is_empty();
        drop(state);

        if is_done {
            self.done.notify_all();
        }
    }

    /// 记录启动失败明细。
    pub(super) fn record_failure(&self, bot_dir_entry: &BotDirEntry, failure_line: String) {
        self.record(
            bot_dir_entry.clone(),
            BotStartOutcome::Failure(failure_line),
        );
    }

    /// 记录尚未启动的取消结果。
    pub(super) fn record_cancelled(&self, bot_dir_entry: &BotDirEntry) {
        self.record(bot_dir_entry.clone(), BotStartOutcome::Cancelled);
    }

    /// 检查结果是否全部记录。
    #[cfg(test)]
    pub(super) fn is_done(&self) -> bool {
        self.state_mut().pending_bot_names.is_empty()
    }

    /// 等待全部结果；超时后将未完成的机器人标记为未确认。
    pub(crate) fn wait_until_done(&self) {
        self.wait_until_done_or_record_timeouts(StartupQueue::get_or_init().result_timeout())
    }

    fn wait_until_done_or_record_timeouts(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut state = self.state_mut();

        while !state.pending_bot_names.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next_state, wait_result) = self
                .done
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;

            if wait_result.timed_out() && !state.pending_bot_names.is_empty() {
                self.record_pending_start_timeouts(&mut state, timeout);
            }
        }
    }

    /// 为未完成的机器人记录超时结果。
    fn record_pending_start_timeouts(&self, state: &mut LedgerState, timeout: Duration) {
        let i18n_text = text::get();
        let timed_out_bots: Vec<&BotDirEntry> = self
            .bots
            .iter()
            .filter(|bot_dir_entry| state.pending_bot_names.contains(&bot_dir_entry.bot_name))
            .collect();

        for bot_dir_entry in timed_out_bots {
            log::warn!(
                "{}",
                i18n_text.log_text(text::LogText::StartResultTimeout {
                    bot_name: &bot_dir_entry.bot_name,
                    timeout,
                })
            );

            state.pending_bot_names.remove(&bot_dir_entry.bot_name);
            state.results.push(
                bot_dir_entry.clone(),
                BotStartOutcome::Failure(
                    i18n_text.start_result_timeout(&bot_dir_entry.bot_name, timeout),
                ),
            );
        }
    }

    /// 取出结果，由调用方复核。
    pub(crate) fn take_results(&self) -> BotStartResults {
        std::mem::take(&mut self.state_mut().results)
    }

    /// 获取账本状态；锁中毒时保留其中的数据。
    fn state_mut(&self) -> MutexGuard<'_, LedgerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::utils::BotTokenStatus;

    use super::*;

    /// 构造测试条目，不读取配置。
    fn bot(bot_name: &str) -> BotDirEntry {
        BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: PathBuf::from("/nonexistent"),
            bot_token_status: BotTokenStatus::Missing,
        }
    }

    /// 按写入顺序返回结果中的机器人名称。
    fn names(results: &BotStartResults) -> Vec<&str> {
        results
            .iter()
            .map(|(bot_dir_entry, _)| bot_dir_entry.bot_name.as_str())
            .collect()
    }

    /// 所有机器人写回后才完成；完成后可按写入顺序取结果，取走后清空。
    #[test]
    fn a_ledger_is_done_only_after_every_bot_is_recorded() {
        let ledger = StartLedger::new(&[bot("duck"), bot("goose")]);

        ledger.record(bot("duck"), BotStartOutcome::Succeeded);
        assert!(!ledger.is_done(), "goose 尚未写回");

        ledger.record(bot("goose"), BotStartOutcome::Skipped);
        assert!(ledger.is_done());

        let results = ledger.take_results();
        assert_eq!(names(&results), ["duck", "goose"]);

        let taken_again = ledger.take_results();
        assert!(taken_again.iter().next().is_none(), "取走后结果应为空");
    }

    /// 两个账本互不混淆：结果只落在登记该机器人的账本里。
    #[test]
    fn two_ledgers_keep_their_own_results() {
        let startall = StartLedger::new(&[bot("duck"), bot("goose")]);
        let start = StartLedger::new(&[bot("crane")]);

        start.record(bot("crane"), BotStartOutcome::Succeeded);
        startall.record(bot("goose"), BotStartOutcome::Succeeded);
        assert!(!startall.is_done(), "duck 尚未写回");
        startall.record(bot("duck"), BotStartOutcome::Skipped);

        assert_eq!(names(&start.take_results()), ["crane"]);
        assert_eq!(names(&startall.take_results()), ["goose", "duck"]);
    }

    /// 启动前已确定的失败（例如队列已满、线程启动失败）直接写入失败明细。
    #[test]
    fn recorded_failures_show_up_in_the_failure_lines() {
        let ledger = StartLedger::new(&[bot("duck")]);

        ledger.record_failure(&bot("duck"), "队列已满".to_string());

        assert!(ledger.is_done());
        let results = ledger.take_results();
        assert!(results.has_failed_lines());
        assert_eq!(results.failure_lines(), ["队列已满"]);
    }

    /// 取消结果不属于失败或“已在运行”；它进入跳过明细，并可单独计数。
    #[test]
    fn recorded_cancellations_are_counted_separately() {
        let ledger = StartLedger::new(&[bot("duck")]);

        ledger.record_cancelled(&bot("duck"));

        assert!(ledger.is_done(), "取消也必须写齐账本，否则等待方会持续阻塞");
        let results = ledger.take_results();
        assert!(!results.has_failed_lines(), "取消不属于失败");
        assert!(!results.has_skipped_running_bots(), "取消不等于已在运行");
        assert!(results.has_cancelled_bots());
        assert_eq!(results.cancelled_bot_names(), ["duck"]);
    }

    /// 超时后，未返回结果的条目录入失败明细，等待方不会无限阻塞。
    #[test]
    fn waiting_gives_up_after_the_timeout_and_records_the_rest() {
        let ledger = StartLedger::new(&[bot("duck"), bot("goose")]);
        ledger.record(bot("duck"), BotStartOutcome::Succeeded);

        ledger.wait_until_done_or_record_timeouts(Duration::from_millis(20));

        assert!(ledger.is_done(), "超时后也必须完成账本");
        let results = ledger.take_results();
        assert_eq!(
            names(&results),
            ["duck", "goose"],
            "没等到的那台也要有一条结果"
        );
        assert_eq!(
            results.failure_lines(),
            [text::get().start_result_timeout("goose", Duration::from_millis(20))]
        );
    }
}
