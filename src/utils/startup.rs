//! 启动会话：待启动的机器人排队进入 [`StartupQueue`]，启动线程按批取走、逐台拉起，最后统一复核一次。
//!
//! 本文件是模块统一的对外接口，需要暴露的方法都实现于此。
//! 启动线程是常驻的（意外崩溃会自动重启，见 `starter.rs::run_starter_forever`）。
//! 内部实现按职责划分：队列内部工具函数在 `queue.rs`，启动流程在 `starter.rs`，账本与复核在 `ledger.rs`，启动结果转回复在 `super::reply.rs`。

mod ledger;
mod queue;
mod starter;
mod verify;

use super::reply::BotStartOutcome;
pub(crate) use super::reply::BotStartResults;
pub(crate) use ledger::StartLedger;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config;
use crate::text;
use crate::utils::bot_dirs::BotDirEntry;
use crate::utils::bot_logs::{create_bot_log_file, reset_fishingbot_logs_dir};

/// 进程内唯一的启动队列。
static STARTUP_QUEUE: OnceLock<StartupQueue> = OnceLock::new();

/// 兜底超时的余量：取"队列排满时最慢的一次启动"的两倍（该倍量用于兜住拉起本身的开销与抖动）。
const RESULT_TIMEOUT_MARGIN_FACTOR: u64 = 2;

/// 启动队列：外部命令只与此对象交互，复杂度封装在内部。
///
/// 前五个字段来自配置取值；
/// 后五个为运行状态，均在 [`StartupQueue::get_or_init`] 中一次填好。
pub struct StartupQueue {
    /// `[startup_queue] max_batch_size`：一批最多拉起几台。
    max_batch_size: usize,
    /// `[startup_queue] bot_login_interval_ms`：一批之内相邻两台的间隔。
    bot_login_interval_ms: u64,
    /// `[startup_queue] batch_login_interval_ms`：批与批之间的间隔。
    batch_login_interval_ms: u64,
    /// `[startup_queue] max_pending_requests`：队列最多暂存几台。
    max_pending_requests: usize,
    /// `timing.check_grace_ms`：每次启动请求拉起后的会话存活宽限期（复核据此判定拉起成败）。
    check_grace_ms: u64,
    /// 待启动列表（先进先出；每台带着自己属于哪个账本，见 `queue.rs`）。
    waiting_bots: Mutex<VecDeque<QueuedBot>>,
    /// 最近一次取出一批的时刻（批间间隔从它算起，见 `queue.rs`）。
    latest_poped_timestamp: Mutex<Instant>,
    /// 队列被写入时唤醒启动线程（见 `queue.rs`）。
    added: Condvar,
    /// 已收到停止拉起请求（"停止全部机器人"的第一步，见 `starter.rs`）。
    /// 启动线程看到就取走（`take_stop_request`），所以只影响这一次，不会拖到后面的批次。
    stop_requested: AtomicBool,
    /// 启动线程的结果：起不来时每次调用都直接失败（见 `starter.rs`）。
    starter: OnceLock<Result<(), String>>,
}

/// 队列里的一个待启动条目：机器人与它所属的账本（结果写回该账本，见 `ledger.rs`）。
///
/// 结果由处理这一台的代码通过 [`QueuedBot::record`] 写回账本。
/// 条目在未写回结果的情况下离开作用域（清空队列、撤单、启动线程 panic 展开、提前返回）时，
/// [`Drop`] 会按"取消"补写一条，以免等待结果的调用方（`/start`、`/startall`）永久阻塞。
#[derive(Debug)]
pub(super) struct QueuedBot {
    /// 这一台机器人。
    bot_dir_entry: BotDirEntry,
    /// 结果写回哪个账本。
    ledger: Arc<StartLedger>,
    /// 结果是否已写回账本（`false` 时由 [`Drop`] 按"取消"补写）。
    written: bool,
}

impl QueuedBot {
    /// 造一个待启动条目。
    pub(super) fn new(bot_dir_entry: BotDirEntry, ledger: Arc<StartLedger>) -> Self {
        Self {
            bot_dir_entry,
            ledger,
            written: false,
        }
    }

    /// 这一台机器人。
    pub(super) fn bot(&self) -> &BotDirEntry {
        &self.bot_dir_entry
    }

    /// 把这一台的结论写回账本（写回后 [`Drop`] 不再补写）。
    pub(super) fn record(&mut self, outcome: BotStartOutcome) {
        self.ledger.record(self.bot_dir_entry.clone(), outcome);
        self.written = true;
    }
}

impl Drop for QueuedBot {
    fn drop(&mut self) {
        // 未写回结果时按"取消"补写：清空队列、撤单、panic 展开都会走到这里。
        if !self.written {
            self.ledger.record_cancelled(&self.bot_dir_entry);
        }
    }
}

impl StartupQueue {
    /// 取进程内唯一的启动队列：首次调用按配置建它，并把启动线程拉起来。
    pub fn get_or_init() -> &'static Self {
        let startup_queue = STARTUP_QUEUE.get_or_init(|| {
            // 启动节奏的取值散在 `[startup_queue]` 与 `timing` 两张表里，取值合法性已在加载配置时校验。
            let settings = config::Config::get();
            let startup_queue = &settings.startup_queue;

            Self {
                max_batch_size: startup_queue.max_batch_size,
                bot_login_interval_ms: startup_queue.bot_login_interval_ms,
                batch_login_interval_ms: startup_queue.batch_login_interval_ms,
                max_pending_requests: startup_queue.max_pending_requests,
                check_grace_ms: settings.timing.check_grace_ms,
                // 运行状态初值：等待列表为空、无待唤醒、线程未启动；取批时刻从当前时间起算，首条命令无需等待。
                waiting_bots: Mutex::new(VecDeque::new()),
                latest_poped_timestamp: Mutex::new(Instant::now()),
                added: Condvar::new(),
                stop_requested: AtomicBool::new(false),
                starter: OnceLock::new(),
            }
        });

        // 启动线程只起一次；起不来的原因留给每次调用自己报。
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

    /// 拉起一台机器人来登录：建日志 → 准备原始日志目录 → `screen -dmS`，返回本次的日志路径。
    ///
    /// 不进队列、不复核（`/bot login` 与队列逐台拉起共用这份实现）；失败时返回能直接进回复的一行。
    pub(crate) fn add_bot_for_login(
        bot_name: &str,
        bot_dir_path: &Path,
    ) -> Result<PathBuf, String> {
        // 日志建不出来就不拉会话；失败原因（带路径）写进程序日志，回复里只给结论。
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

        // FishingBot 自己的原始日志写向 /dev/null（磁盘上只留过滤后的日志）。
        let fishingbot_logs_dir = reset_fishingbot_logs_dir(bot_dir_path);

        // JVM 的输出过滤后追加进上面建好的日志文件。
        starter::launch_bot_session(bot_name, bot_dir_path, &log_path, &fishingbot_logs_dir)?;

        Ok(log_path)
    }

    /// 命令方入口（`/start`、`/startall`、`/restart` 共用）：
    /// 把账本里登记的机器人排进启动队列（入队即返回，不等结果）。
    ///
    /// 启动线程没起来、或队列容量不够没收下的，直接往账本里记失败明细。
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
            // 队列满了：没收下的那部分当场记成失败（回复里能看到），其余照常排队。
            let i18n_text = text::get();
            for mut queued_bot in rejected {
                let failure_line = i18n_text
                    .start_launch_failed(&queued_bot.bot().bot_name, starter::QUEUE_FULL_REASON);
                queued_bot.record(BotStartOutcome::Failure(failure_line));
            }
        }
    }

    /// 命令方的完整入口（`/start`、`/startall`、`/restart` 与 `/restartall` 共用）：
    /// 建账本 → 入队 → 等自己的条目都被拉起过一遍 → 取结果并交由队列复核（会话存活满宽限期即算拉起成功）。
    ///
    /// 编排只写这一处：调用方拿到结果后自己决定怎么报（成功 / 跳过 / 取消 / 失败的明细都在里面）。
    pub(crate) fn start_bots(&self, bots: &[BotDirEntry]) -> BotStartResults {
        let ledger = StartLedger::new(bots);
        self.add_bots_to_queue(&ledger);
        ledger.wait_until_done();

        let mut results = ledger.take_results();
        self.verify(&mut results);

        results
    }

    // 复核

    /// 复核这次调用的结果（会话存活满宽限期，见 `verify.rs`），没过的写回结果里。
    pub(crate) fn verify(&self, results: &mut BotStartResults) {
        verify::verify_batch(results, self.check_grace());
    }

    // 队列查询与撤单

    /// 当前在队列中等待的机器人数量（命令层据此估算耗时：它们排在本次请求之前）。
    pub(crate) fn pending_bot_count(&self) -> usize {
        self.waiting_bots_mut().len()
    }

    /// 队列里有没有这台机器人（外部命令判断"正在排队启动"用它）。
    pub(crate) fn bot_in_queue(&self, bot: &BotDirEntry) -> bool {
        self.waiting_bots_mut()
            .iter()
            .any(|queued_bot| queued_bot.bot().bot_name == bot.bot_name)
    }

    /// 逐台问：队列里有没有它们（返回顺序与入参一致）。
    ///
    /// 命令层只读的查询也用它（`/show status` 的表后那一行、bin 侧的候选列表），所以是 `pub`。
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

    /// 把这几台从队列里撤下来（`/stop` 用），返回每台是否真的撤掉了。
    ///
    /// 机器人可能刚好被拉起、已经离开队列，所以撤不掉不算失败，只报 `false`。
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

    /// 清空队列（"全部停止"收尾用：`stop_all_bot_sessions` 会先调它；清空不可能失败）。
    pub(crate) fn remove_all_bots_from_queue(&self) {
        self.waiting_bots_mut().clear();
    }

    /// 请求启动线程停止拉起（"停止全部机器人"的第一步）：置位并唤醒启动线程。
    ///
    /// 置位时持有待启动列表的锁，因此正在等待的启动线程不会丢失这次唤醒
    /// （它的等待条件是"队列非空或已收到停止请求"，见 `queue.rs`）。
    pub(crate) fn request_stop(&self) {
        let _waiting_bots = self.waiting_bots_mut();
        self.stop_requested.store(true, Ordering::SeqCst);
        self.added.notify_all();
    }

    /// 取走停止拉起的请求（取到即清除，仅影响本次）。
    pub(super) fn take_stop_request(&self) -> bool {
        self.stop_requested.swap(false, Ordering::SeqCst)
    }

    /// 是否存在尚未取走的停止拉起请求（作为等待条件之一，见 `queue.rs`）。
    pub(super) fn has_stop_request(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    // 上报给命令层的取值

    /// 一批最多拉起几台（`/startall` 的回复里要报）。
    pub(crate) fn max_batch_size(&self) -> usize {
        self.max_batch_size
    }

    /// 批内相邻两台的登录间隔（`/startall` 的回复里要报）。
    pub(crate) fn bot_login_interval_ms(&self) -> u64 {
        self.bot_login_interval_ms
    }

    /// 等启动结果的兜底超时（账本用它，见 [`StartLedger::wait_until_done`]）：按节奏参数算出来，不单独配置。
    ///
    /// 取"队列排满时最慢的一次启动"（[`Self::slowest_start_duration`]）再乘
    /// [`RESULT_TIMEOUT_MARGIN_FACTOR`]，兜住拉起本身的开销与抖动。
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
    pub(crate) fn batch_login_interval_ms(&self) -> u64 {
        self.batch_login_interval_ms
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
            latest_poped_timestamp: Mutex::new(Instant::now()),
            added: Condvar::new(),
            stop_requested: AtomicBool::new(false),
            starter: OnceLock::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 上报给用户的取值就是配置里的取值（回复里的"每批最多 N 台、批内 M 毫秒、批间 K 毫秒"）。
    #[test]
    fn the_queue_reports_the_configured_values() {
        let queue = StartupQueue::new_for_test(4, 15_000, 45_000, 300, 32);

        assert_eq!(queue.max_batch_size(), 4);
        assert_eq!(queue.bot_login_interval_ms(), 15_000);
        assert_eq!(queue.batch_login_interval_ms(), 45_000);
    }

    /// 兜底超时是算出来的：队列排满时最慢的一次启动的两倍（不单独配置，按节奏参数推导）。
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

    /// 一个测试用条目（不装配置也能造：非法名字让"拼会话名"这一步先失败）。
    fn bot(bot_name: &str) -> BotDirEntry {
        BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: PathBuf::from("/nonexistent"),
            bot_token_status: crate::utils::bot_dirs::BotTokenStatus::Missing,
        }
    }

    /// 条目离开作用域时把账本收齐：写回过的那一份照原样进账本，没写过的补一条"取消"。
    #[test]
    fn dropping_an_entry_fills_the_ledger_exactly_once() {
        let recorded_ledger = StartLedger::new(&[bot("duck")]);
        let mut recorded_entry = QueuedBot::new(bot("duck"), Arc::clone(&recorded_ledger));
        recorded_entry.record(BotStartOutcome::Succeeded);
        drop(recorded_entry);

        assert!(recorded_ledger.is_done(), "写回之后账本该齐了");
        let recorded_results = recorded_ledger.take_results();
        assert_eq!(
            recorded_results.iter().next().map(|(_, outcome)| outcome),
            Some(&BotStartOutcome::Succeeded),
            "写回的就是那一份结论"
        );
        assert!(
            !recorded_results.has_cancelled_bots(),
            "写回过的不再补第二条"
        );

        let dropped_ledger = StartLedger::new(&[bot("goose")]);
        drop(QueuedBot::new(bot("goose"), Arc::clone(&dropped_ledger)));

        assert!(dropped_ledger.is_done(), "直接被丢弃也要把账本收齐");
        assert!(dropped_ledger.take_results().has_cancelled_bots());
    }
}
