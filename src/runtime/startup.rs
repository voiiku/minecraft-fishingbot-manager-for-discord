//! 机器人按批次排队启动，结果由账本统一复核；启动线程异常退出后自动重启。

mod ledger;
mod queue;
mod starter;
mod verify;

use crate::utils::{BotStartOutcome, BotStartResults};
pub(crate) use ledger::StartLedger;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config;
use crate::text;
use crate::utils::{BotDirEntry, create_bot_log_file, reset_fishingbot_raw_logs_dir};

/// 进程内唯一的启动队列。
static STARTUP_QUEUE: OnceLock<StartupQueue> = OnceLock::new();

/// 结果超时取队列满载时最慢启动耗时的倍数。
const RESULT_TIMEOUT_MARGIN_FACTOR: u64 = 2;

/// 进程内共享的启动队列。
pub struct StartupQueue {
    /// `[startup_queue] max_batch_size`：每批最多启动数量。
    max_batch_size: usize,
    /// `[startup_queue] bot_login_interval`：批内启动间隔（内部毫秒）。
    bot_login_interval_ms: u64,
    /// `[startup_queue] batch_login_interval`：批与批之间的间隔（内部毫秒）。
    batch_login_interval_ms: u64,
    /// `[startup_queue] max_pending_requests`：队列容量。
    max_pending_requests: usize,
    /// `timing.check_grace`：启动后的会话存活宽限期（内部毫秒）。
    check_grace_ms: u64,
    /// 待启动列表，按先进先出顺序处理。
    waiting_bots: Mutex<VecDeque<QueuedBot>>,
    /// 最近一次取出批次的时刻，作为批间间隔的起点。
    last_batch_dequeued_at: Mutex<Instant>,
    /// 队列变更时唤醒启动线程。
    added: Condvar,
    /// 停止启动请求；启动线程消费后清除，避免影响后续批次。
    stop_requested: AtomicBool,
    /// 启动线程的创建结果；创建失败时后续调用返回错误。
    starter: OnceLock<Result<(), String>>,
}

/// 启动队列条目及所属账本；未记录结果时由 [`Drop`] 补记取消，避免调用方持续等待。
#[derive(Debug)]
pub(super) struct QueuedBot {
    bot_dir_entry: BotDirEntry,
    ledger: Arc<StartLedger>,
    /// 结果是否已记录；未记录时由 [`Drop`] 补记取消。
    written: bool,
}

impl QueuedBot {
    pub(super) fn new(bot_dir_entry: BotDirEntry, ledger: Arc<StartLedger>) -> Self {
        Self {
            bot_dir_entry,
            ledger,
            written: false,
        }
    }

    pub(super) fn bot(&self) -> &BotDirEntry {
        &self.bot_dir_entry
    }

    /// 记录启动结果，避免 [`Drop`] 重复记账。
    pub(super) fn record(&mut self, outcome: BotStartOutcome) {
        self.ledger.record(self.bot_dir_entry.clone(), outcome);
        self.written = true;
    }
}

impl Drop for QueuedBot {
    fn drop(&mut self) {
        if !self.written {
            self.ledger.record_cancelled(&self.bot_dir_entry);
        }
    }
}

impl StartupQueue {
    /// 获取进程内唯一的启动队列，首次调用时创建队列及启动线程。
    pub fn get_or_init() -> &'static Self {
        let startup_queue = STARTUP_QUEUE.get_or_init(|| {
            let settings = config::Config::get();
            let startup_queue = &settings.startup_queue;

            Self {
                max_batch_size: startup_queue.max_batch_size,
                bot_login_interval_ms: startup_queue.bot_login_interval_ms,
                batch_login_interval_ms: startup_queue.batch_login_interval_ms,
                max_pending_requests: startup_queue.max_pending_requests,
                check_grace_ms: settings.timing.check_grace_ms,
                // 首批的批间等待从队列初始化时刻开始计算。
                waiting_bots: Mutex::new(VecDeque::new()),
                last_batch_dequeued_at: Mutex::new(Instant::now()),
                added: Condvar::new(),
                stop_requested: AtomicBool::new(false),
                starter: OnceLock::new(),
            }
        });

        // 启动线程仅创建一次；创建错误由调用方报告。
        let _ = startup_queue.starter.get_or_init(|| {
            thread::Builder::new()
                .name("bot-starter".to_string())
                .spawn(|| starter::run_starter_forever(startup_queue))
                .map(|_worker: JoinHandle<()>| ())
                .map_err(|error| error.to_string())
        });

        startup_queue
    }
    // 命令入口

    /// 创建日志并启动登录会话，返回日志路径；不入队或复核。
    pub(crate) fn create_logs_and_launch_bot_session(
        bot_name: &str,
        bot_dir_path: &Path,
    ) -> Result<PathBuf, String> {
        // 创建日志失败时不启动会话；详细原因仅写入程序日志。
        let log_path = match create_bot_log_file(bot_dir_path) {
            Ok(log_path) => log_path,
            Err(message) => {
                log::error!(
                    "{}",
                    text::get().log_text(text::LogText::StartLogFileFailed {
                        bot_name,
                        reason: &message,
                    })
                );
                return Err(text::get().start_log_file_failed(bot_name));
            }
        };

        // FishingBot 原始日志指向丢弃设备，仅保留过滤后的日志。
        let fishingbot_logs_dir = reset_fishingbot_raw_logs_dir(bot_dir_path);

        starter::launch_bot_session(bot_name, bot_dir_path, &log_path, &fishingbot_logs_dir)?;

        Ok(log_path)
    }

    /// 提交账本中的机器人，线程创建失败或队列满时记录失败结果。
    pub(crate) fn add_bots_to_queue(&self, ledger: &Arc<StartLedger>) {
        if let Err(reason) = self.starter_result() {
            starter::record_launch_failures(ledger, ledger.bots(), &reason);
            return;
        }

        let entries: Vec<QueuedBot> = ledger
            .bots()
            .iter()
            .map(|bot_dir_entry| QueuedBot::new(bot_dir_entry.clone(), Arc::clone(ledger)))
            .collect();

        if let Err(rejected) = self.add_waiting_bots(entries) {
            let i18n_text = text::get();
            for mut queued_bot in rejected {
                let failure_line = i18n_text.start_launch_failed(
                    &queued_bot.bot().bot_name,
                    i18n_text.start_queue_full_reason(),
                );
                queued_bot.record(BotStartOutcome::Failure(failure_line));
            }
        }
    }

    /// 创建账本、提交启动请求并等待和复核结果；调用方负责报告。
    pub(crate) fn start_bots(&self, bots: &[BotDirEntry]) -> BotStartResults {
        let ledger = StartLedger::new(bots);
        self.add_bots_to_queue(&ledger);
        ledger.wait_until_done();

        let mut results = ledger.take_results();
        self.verify_start_results(&mut results);

        results
    }

    // 复核

    /// 复核启动结果，并更新失败的条目。
    pub(crate) fn verify_start_results(&self, results: &mut BotStartResults) {
        verify::verify_start_results(results, self.check_grace());
    }

    // 队列查询与撤单

    /// 当前队列中的机器人数量，用于估算等待时间。
    pub(crate) fn pending_bot_count(&self) -> usize {
        self.waiting_bots_mut().len()
    }

    /// 检查机器人是否在启动队列中。
    pub(crate) fn bot_in_queue(&self, bot: &BotDirEntry) -> bool {
        self.waiting_bots_mut()
            .iter()
            .any(|queued_bot| queued_bot.bot().bot_name == bot.bot_name)
    }

    /// 查询各机器人是否在队列中，返回顺序与输入一致。
    pub fn bots_in_queue(&self, bots: &[BotDirEntry]) -> Vec<(BotDirEntry, bool)> {
        let waiting_bots = self.waiting_bots_mut();

        bots.iter()
            .map(|bot| {
                let in_queue = waiting_bots
                    .iter()
                    .any(|queued_bot| queued_bot.bot().bot_name == bot.bot_name);

                (bot.clone(), in_queue)
            })
            .collect()
    }

    /// 从队列移除机器人，返回每项是否移除；已离开队列的条目返回 `false`。
    pub(crate) fn remove_bots_from_queue(&self, bots: &[BotDirEntry]) -> Vec<(BotDirEntry, bool)> {
        let mut waiting_bots = self.waiting_bots_mut();

        bots.iter()
            .map(|bot| {
                let position = waiting_bots
                    .iter()
                    .position(|queued_bot| queued_bot.bot().bot_name == bot.bot_name);
                let removed =
                    position.is_some_and(|position| waiting_bots.remove(position).is_some());

                (bot.clone(), removed)
            })
            .collect()
    }

    /// 清空待启动队列。
    pub(crate) fn remove_all_bots_from_queue(&self) {
        self.waiting_bots_mut().clear();
    }

    /// 请求启动线程停止；持有队列锁设置标志并通知，避免丢失唤醒。
    pub(crate) fn request_stop(&self) {
        let _waiting_bots = self.waiting_bots_mut();
        self.stop_requested.store(true, Ordering::SeqCst);
        self.added.notify_all();
    }

    /// 获取并清除停止请求。
    pub(super) fn take_stop_request(&self) -> bool {
        self.stop_requested.swap(false, Ordering::SeqCst)
    }

    /// 检查是否存在待处理的停止请求。
    pub(super) fn has_stop_request(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    // 上报给命令层的取值

    /// 每批最多启动的机器人数量。
    pub(crate) fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }

    /// 批内相邻机器人的登录间隔。
    pub(crate) fn bot_login_interval(&self) -> Duration {
        Duration::from_millis(self.bot_login_interval_ms)
    }

    /// 按满载队列的最长预计耗时与余量倍数计算结果超时。
    pub(crate) fn result_timeout(&self) -> Duration {
        Duration::from_millis(
            self.slowest_start_ms(self.max_pending_requests)
                .saturating_mul(RESULT_TIMEOUT_MARGIN_FACTOR),
        )
    }

    /// 这 `bot_count` 台排队时最慢的一次启动（命令层用于估算耗时，超时值也以此为基础）。
    ///
    /// `⌈bot_count / max_batch_size⌉ × max(batch_login_interval_ms, (max_batch_size - 1) × bot_login_interval_ms)
    ///  + check_grace_ms`
    pub(crate) fn slowest_start_duration(&self, bot_count: usize) -> Duration {
        Duration::from_millis(self.slowest_start_ms(bot_count))
    }

    /// [`Self::slowest_start_duration`] 的毫秒形式（超时值需在此基础上乘余量，故返回毫秒）。
    fn slowest_start_ms(&self, bot_count: usize) -> u64 {
        let batch_count = bot_count.div_ceil(self.max_batch_size);
        let slowest_batch = self.batch_login_interval_ms.max(
            self.bot_login_interval_ms
                .saturating_mul(self.max_batch_size.saturating_sub(1) as u64),
        );
        let check_grace = self.check_grace_ms;

        (batch_count as u64)
            .saturating_mul(slowest_batch)
            .saturating_add(check_grace)
    }

    /// 批与批之间的间隔（`/startall` 的回复里要报）。
    pub(crate) fn batch_login_interval(&self) -> Duration {
        Duration::from_millis(self.batch_login_interval_ms)
    }

    /// 测试用的构造：不读配置，直接给取值；运行状态与 [`Self::get_or_init`] 里那份一致。
    #[cfg(test)]
    pub(super) fn new_for_test(
        max_batch_size: usize,
        bot_login_interval_ms: u64,
        batch_login_interval_ms: u64,
        check_grace_ms: u64,
        max_pending_requests: usize,
    ) -> Self {
        Self {
            max_batch_size,
            bot_login_interval_ms,
            batch_login_interval_ms,
            max_pending_requests,
            check_grace_ms,
            waiting_bots: Mutex::new(VecDeque::new()),
            last_batch_dequeued_at: Mutex::new(Instant::now()),
            added: Condvar::new(),
            stop_requested: AtomicBool::new(false),
            starter: OnceLock::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 队列对外上报的节奏参数必须与配置一致（每批数量、批内间隔、批间间隔）。
    #[test]
    fn the_queue_reports_the_configured_values() {
        let queue = StartupQueue::new_for_test(4, 15_000, 45_000, 300, 32);

        assert_eq!(queue.max_batch_size(), 4);
        assert_eq!(queue.bot_login_interval(), Duration::from_secs(15));
        assert_eq!(queue.batch_login_interval(), Duration::from_secs(45));
    }

    /// 结果超时按队列排满时最慢一次启动耗时的两倍计算，不单独配置。
    #[test]
    fn the_result_timeout_covers_a_full_queue_with_margin() {
        // 批数 = ⌈32 / 3⌉ = 11，每批最慢 = max(批间 20s, 批内 2 × 20s) = 40s，宽限期 300ms（每次启动请求一次）。
        let queue = StartupQueue::new_for_test(3, 20_000, 20_000, 300, 32);
        let slowest_start_ms = 11 * 40_000 + 300;
        assert_eq!(
            queue.result_timeout(),
            Duration::from_millis(slowest_start_ms * 2)
        );

        // 每批一台时批内错峰不参与：只剩批间等待与复核等待。
        let single_bot_batch = StartupQueue::new_for_test(1, 20_000, 20_000, 300, 3);
        assert_eq!(
            single_bot_batch.result_timeout(),
            Duration::from_millis((3 * 20_000 + 300) * 2)
        );
    }

    /// 构造测试条目；不读取配置，非法名称可在拼会话名阶段直接失败。
    fn bot(bot_name: &str) -> BotDirEntry {
        BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: PathBuf::from("/nonexistent"),
            bot_token_status: crate::utils::BotTokenStatus::Missing,
        }
    }

    #[test]
    fn queue_full_uses_localized_failure_reason() {
        let queue = StartupQueue::new_for_test(1, 1000, 1000, 100, 1);
        let ledger = StartLedger::new(&[bot("duck"), bot("goose")]);
        queue.add_bots_to_queue(&ledger);

        let i18n_text = text::get();
        assert_eq!(
            ledger.take_results().failure_lines(),
            [i18n_text.start_launch_failed("goose", i18n_text.start_queue_full_reason())]
        );
    }

    /// 条目离开作用域时收齐账本：已写回的结果保持不变，未写回的补记为取消。
    #[test]
    fn dropping_an_entry_fills_the_ledger_exactly_once() {
        let recorded_ledger = StartLedger::new(&[bot("duck")]);
        let mut recorded_entry = QueuedBot::new(bot("duck"), Arc::clone(&recorded_ledger));
        recorded_entry.record(BotStartOutcome::Succeeded);
        drop(recorded_entry);

        assert!(recorded_ledger.is_done(), "写回后账本必须完成");
        let recorded_results = recorded_ledger.take_results();
        assert_eq!(
            recorded_results.iter().next().map(|(_, outcome)| outcome),
            Some(&BotStartOutcome::Succeeded),
            "账本应保留已写入的结果"
        );
        assert!(
            !recorded_results.has_cancelled_bots(),
            "已写回的结果不得再补记为取消"
        );

        let dropped_ledger = StartLedger::new(&[bot("goose")]);
        drop(QueuedBot::new(bot("goose"), Arc::clone(&dropped_ledger)));

        assert!(dropped_ledger.is_done(), "直接丢弃条目也必须完成账本");
        assert!(dropped_ledger.take_results().has_cancelled_bots());
    }
}
