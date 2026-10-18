//! 命令的账本与复核的内部逻辑。
//!
//! 调用者建账本（[`StartLedger::new`]）并把它交给队列，启动线程取走条目后把结果写回来；
//! 待核对的机器人为 0 时唤醒调用者，调用者取走结果自己做复核（[`verify_batch`]）。

use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::text;

use super::StartupQueue;

use crate::utils::bot_dirs::BotDirEntry;

use crate::utils::reply::{BotStartOutcome, BotStartResults};

/// 账本的内部状态。
#[derive(Debug, Default)]
struct LedgerState {
    /// 仍在等待结果的机器人名（为空即本次调用已齐）。
    pending_bot_names: HashSet<String>,
    /// 已经写回的结果（按跑完顺序）。
    results: BotStartResults,
}

/// 一次启动调用的账本：调用者建它并登记要核对的机器人，队列把结果写回来。
#[derive(Debug)]
pub struct StartLedger {
    /// 这次调用要核对的机器人（提交时记下，之后不变）。
    bots: Vec<BotDirEntry>,
    state: Mutex<LedgerState>,
    /// 待核对清空时唤醒等待的调用者。
    done: Condvar,
}

impl StartLedger {
    /// 建一个账本：要核对的机器人就是这几台。
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

    /// 这次要核对的机器人（队列照它入队）。
    pub(super) fn bots(&self) -> &[BotDirEntry] {
        &self.bots
    }

    /// 写回一台的结果；都写齐了就唤醒等待的调用者。
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

    /// 写回一台的失败明细（文案由调用方按现场给）。
    pub(super) fn record_failure(&self, bot_dir_entry: &BotDirEntry, failure_line: String) {
        self.record(
            bot_dir_entry.clone(),
            BotStartOutcome::Failure(failure_line),
        );
    }

    /// 写回一台的"取消"结果：尚未拉起便被"停止全部机器人"撤下（见 [`BotStartOutcome::Cancelled`]）。
    pub(super) fn record_cancelled(&self, bot_dir_entry: &BotDirEntry) {
        self.record(bot_dir_entry.clone(), BotStartOutcome::Cancelled);
    }

    /// 这次调用是不是都写齐了（测试用；线上流程走 [`Self::wait_until_done`]）。
    #[cfg(test)]
    pub(super) fn is_done(&self) -> bool {
        self.state_mut().pending_bot_names.is_empty()
    }

    /// 等待本次调用收齐结果（每台都会写回：启动线程常驻，单台出错也会写回失败明细）。
    ///
    /// 等待时长由队列按节奏参数算出（[`StartupQueue::result_timeout`]）；超时则把未回结果的按
    /// "未确认"收尾后返回，避免个别路径滞留导致命令线程永久阻塞（看门狗与退出收尾同样等待它）。
    pub(crate) fn wait_until_done(&self) {
        self.wait_until_done_with(StartupQueue::get_or_init().result_timeout())
    }

    /// [`Self::wait_until_done`] 的实现：超时值可注入（测试用小值）。
    fn wait_until_done_with(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut state = self.state_mut();

        while !state.pending_bot_names.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (next_state, wait_result) = self
                .done
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next_state;

            // 超时且未收齐：把剩余条目按"未确认"写回，本次调用即告结束。
            if wait_result.timed_out() && !state.pending_bot_names.is_empty() {
                self.record_unconfirmed(&mut state, timeout);
            }
        }
    }

    /// 超时收尾：未回结果的各记一条失败明细（注明机器人与等待时长）。
    fn record_unconfirmed(&self, state: &mut LedgerState, timeout: Duration) {
        let i18n_text = text::get();
        let unconfirmed_bots: Vec<BotDirEntry> = self
            .bots
            .iter()
            .filter(|bot_dir_entry| state.pending_bot_names.contains(&bot_dir_entry.bot_name))
            .cloned()
            .collect();

        for bot_dir_entry in unconfirmed_bots {
            let timeout_seconds = timeout.as_secs();
            log::warn!(
                "{}",
                i18n_text.log_text(text::LogText::StartResultTimeout {
                    bot_name: &bot_dir_entry.bot_name,
                    seconds: timeout_seconds,
                })
            );

            state.pending_bot_names.remove(&bot_dir_entry.bot_name);
            state.results.push(
                bot_dir_entry.clone(),
                BotStartOutcome::Failure(
                    i18n_text.start_result_timeout(&bot_dir_entry.bot_name, timeout_seconds),
                ),
            );
        }
    }

    /// 取走结果（之后由调用者自己复核）。
    pub(crate) fn take_results(&self) -> BotStartResults {
        std::mem::take(&mut self.state_mut().results)
    }

    /// 取状态锁（中毒的锁也要能用：这里只放数据，不存"必须一致"的不变量）。
    fn state_mut(&self) -> MutexGuard<'_, LedgerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::utils::bot_dirs::BotTokenStatus;

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

        ledger.wait_until_done_with(Duration::from_millis(20));

        assert!(ledger.is_done(), "超时后也必须完成账本");
        let results = ledger.take_results();
        assert_eq!(
            names(&results),
            ["duck", "goose"],
            "没等到的那台也要有一条结果"
        );
        assert_eq!(results.failure_lines().len(), 1, "没等到的那台记成失败明细");
    }
}
