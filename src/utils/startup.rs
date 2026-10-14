//! 启动：拉起会话的四步流程（`/start`、`/startall` 与重启里的拉起部分共用）。
//!
//! 批次启动是四个显式步骤（见 [`start_bot_sessions_now`]），shell 脚本只负责"跑 JVM → 过滤 → 追加日志"：
//!
//! 1. 建这次运行的日志文件（[`super::bot_log_files::create_bot_log_file`]）；
//! 2. 准备 FishingBot 自己的日志目录（[`reset_fishingbot_logs_dir`]）；
//! 3. 拉起 screen 会话（[`launch_bot_session`]）；
//! 4. 错峰 + 一次 `screen -list` 快照复核会话是否出现。
//!
//! 重启（停止 + 上面这套启动）在 [`super::restart`]：它同时依赖本模块与 [`super::shutdown`]。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config;
use crate::text;

use super::bot_dirs::BotDirEntry;
use super::bot_log_files::{bot_logs_dir, create_bot_log_file, target_logs_dir};
use super::bot_status::BotPauseStatusMemory;
use super::fs::remove_dir_all_if_exists;
use super::reply::Report;
use super::screen::{active_screen_sessions, session_name};

/// 一项提交给全局启动队列的请求。
struct StartRequest {
    bot_dir_entries: Vec<BotDirEntry>,
    reply: SyncSender<LaunchOutcome>,
}

/// 批次准入状态：两次登录之间的间隔由 `[startup_queue] login_interval_ms` 决定，这里只记"上一次登录是
/// 什么时候"与"当前批次已经拉起了几台"。状态跨请求保留，所以连续命令也绕不过限流。
struct StartBatchGate {
    batch_size: usize,
    /// 两次登录之间的最小间隔。
    interval: Duration,
    /// 每台机器人的复核等待（`timing.settle_per_bot_ms`）。批次完成 = 最后一次登录 + 两遍复核等待，
    /// 所以从完成时刻反推最后一次登录时刻要用到它。
    settle_per_bot: Duration,
    launched_in_batch: usize,
    last_login_at: Option<Instant>,
}

impl StartBatchGate {
    fn new(batch_size: usize, interval: Duration, settle_per_bot: Duration) -> Self {
        Self {
            batch_size: batch_size.max(1),
            interval,
            settle_per_bot,
            launched_in_batch: 0,
            last_login_at: None,
        }
    }

    fn remaining_capacity(&self) -> usize {
        self.batch_size - self.launched_in_batch
    }

    /// 下一次登录前还要等多久：上一次登录还没满一个间隔就返回剩余时间，否则不用等。
    fn wait_before_next_login(&self, now: Instant) -> Duration {
        self.last_login_at
            .map(|last_login_at| {
                self.interval
                    .saturating_sub(now.saturating_duration_since(last_login_at))
            })
            .unwrap_or_default()
    }

    /// 记账一次批次拉起：`completed_at` 是复核结束的时刻，`attempt_count` 是本批实际拉起台数。
    ///
    /// 最后一次登录时刻 = 完成时刻 − 两遍复核等待。这样"批间"要等的时间就只是间隔减去这段等待
    ///（不反推的话每一批都会白等一个复核窗口，实际间隔会大于配置值）。
    fn record_attempts(&mut self, attempt_count: usize, completed_at: Instant) {
        if attempt_count > 0 {
            let recheck_wait = self
                .settle_per_bot
                .saturating_mul(2u32.saturating_mul(attempt_count as u32));
            // `Instant` 只能往回退到进程开始计时的那一刻，退不动就按完成时刻算（只会更保守）。
            self.last_login_at = Some(
                completed_at
                    .checked_sub(recheck_wait)
                    .unwrap_or(completed_at),
            );
        }

        self.launched_in_batch = (self.launched_in_batch + attempt_count).min(self.batch_size);
    }

    /// 队列在一个未满批次后变为空时，把这个部分批次按已结束处理：下一条请求从新的一批开始
    ///（间隔由 `last_login_at` 保证，与批次满没满无关）。
    fn finish_partial_batch(&mut self) {
        self.launched_in_batch = self.batch_size;
    }

    fn begin_next_batch(&mut self) {
        self.launched_in_batch = 0;
    }
}

/// 启动调度器的唯一入口；工作线程与批次冷却状态在进程生命周期内保留。
static START_QUEUE: OnceLock<Result<SyncSender<StartRequest>, String>> = OnceLock::new();

/// 返回全局启动队列的发送端
fn start_queue_sender() -> Result<SyncSender<StartRequest>, String> {
    START_QUEUE
        .get_or_init(|| {
            let (sender, receiver) =
                mpsc::sync_channel(crate::config::get().startup_queue.max_pending_requests);
            thread::Builder::new()
                .name("bot-start-queue".to_string())
                .spawn(move || process_start_queue(receiver))
                .map(|_worker: JoinHandle<()>| sender)
                .map_err(|error| error.to_string())
        })
        .clone()
}

/// 准备 FishingBot.jar 的日志目录（启动流程的第二步）
/// 整个目录删掉重建 `<机器人目录>/<runtime.logs_dir>/raw`，并把它的原始日志指向 `discard_path`
/// 清之前先把"意料之外的文件"报出来，如果静默删除的话，会忽略 FishingBot.jar 的日志改了命名规则。
/// 意外只警告、不打断启动，最坏情况是 FishingBot.jar 因建不出日志文件而拒绝启动。
fn reset_fishingbot_logs_dir(bot_dir: &Path) -> PathBuf {
    let raw_game_logs = &config::get().raw_game_logs;

    reset_raw_logs_dir(
        &target_logs_dir(&bot_logs_dir(bot_dir), &raw_game_logs.dir_name),
        &raw_game_logs.file_name,
        &raw_game_logs.discard_path,
    )
}

/// [`reset_fishingbot_logs_dir`] 的实现：`raw_logs_dir` 是 raw 目录本身，
/// 名字与丢弃目标由调用方给（拆出来是为了能不吃配置地单测，读配置那一步在它上面）。
fn reset_raw_logs_dir(
    raw_logs_dir: &Path,
    game_log_file_name: &str,
    discard_path: &str,
) -> PathBuf {
    // 第一步：先报出上一轮留下的"意料之外的文件"，随后整个目录会被清掉。
    for file_name in unexpected_game_log_file_names(raw_logs_dir, discard_path) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::UnexpectedGameLogFile {
                dir: &raw_logs_dir.display().to_string(),
                file_name: &file_name,
            })
        );
    }

    // 第二步：整个目录删掉重建（"目录本来就不在"由 `remove_dir_all_if_exists` 当成成功，不必再判断存在性）。
    if let Err(e) = remove_dir_all_if_exists(raw_logs_dir) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogsDirRemoveFailed {
                dir: &raw_logs_dir.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }
    if let Err(e) = fs::create_dir_all(raw_logs_dir) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogsDirCreateFailed {
                dir: &raw_logs_dir.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }

    // 第三步：把预期的原始日志指向丢弃路径（FishingBot.jar 建不出日志文件会拒绝启动，给它一个"能写、但什么都不存"的目标）。
    let game_log_file = raw_logs_dir.join(game_log_file_name);
    if let Err(e) = std::os::unix::fs::symlink(discard_path, &game_log_file) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::GameLogDiscardFailed {
                file: &game_log_file.display().to_string(),
                device: discard_path,
                reason: &e.to_string(),
            })
        );
    }

    raw_logs_dir.to_path_buf()
}

/// 列出 `raw/` 里不应该存在的文件名（已排序）；目录读取错误时返回空数组。
/// 预期只有一个文件：`game_log_file_name` 指向 `discard_path` 的符号链接；
/// 其他文件都算意外。[`reset_raw_logs_dir`] 会在清掉整个目录前先把它报出来。
fn unexpected_game_log_file_names(raw_logs_dir: &Path, discard_path: &str) -> Vec<String> {
    let Ok(dir_reader) = fs::read_dir(raw_logs_dir) else {
        return Vec::new();
    };

    let mut unexpected_names: Vec<String> = dir_reader
        .flatten()
        .filter_map(|entry| {
            // 只有符号链接 + 指向 `discard_path` 才算预期的文件。
            let is_discard_link =
                fs::read_link(entry.path()).is_ok_and(|target| target == Path::new(discard_path));
            if is_discard_link {
                return None;
            }

            Some(entry.file_name().to_str()?.to_string())
        })
        .collect();
    unexpected_names.sort();

    unexpected_names
}

/// 启动过程的结果明细，只返回原始结果
/// 实现了方便转换原始结果为回复的方法，由调用方显式调用（见 [`LaunchOutcome::into_report`]）。
#[derive(Debug, Default)]
pub(crate) struct LaunchOutcome {
    /// 已发送启动命令、且会话已出现的机器人名。
    pub(crate) started_bot_names: Vec<String>,
    /// 任务执行时已由其他入口启动的机器人名。
    pub(crate) skipped_running_bot_names: Vec<String>,
    /// 已实际尝试拉起 JVM 的数量；准备阶段失败不计入全局批次限额。
    attempted_launch_count: usize,
    /// 失败明细（缺 Token / 日志建不出来 / 拉起失败 / 复核时没出现）。
    pub(crate) failure_lines: Vec<String>,
}

impl LaunchOutcome {
    fn append(&mut self, mut other: Self) {
        self.started_bot_names.append(&mut other.started_bot_names);
        self.skipped_running_bot_names
            .append(&mut other.skipped_running_bot_names);
        self.failure_lines.append(&mut other.failure_lines);
        self.attempted_launch_count += other.attempted_launch_count;
    }

    /// 有没有失败（与 [`StopOutcome::has_failed_lines`] 对称）。
    pub(crate) fn has_failed_lines(&self) -> bool {
        !self.failure_lines.is_empty()
    }

    /// 是否有机器人在排队期间已由其他入口启动。
    pub(crate) fn has_skipped_running_bots(&self) -> bool {
        !self.skipped_running_bot_names.is_empty()
    }

    /// 失败明细拼成单行错误提示（单个机器人指令直接拿它当 `Err`）。
    pub(crate) fn to_failure_text(&self) -> String {
        self.failure_lines.join(" / ")
    }

    /// 报告：成功的与失败的都进报告；计数由调用方按指令口径 `set_counts`。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new();

        report.succeeded = self
            .started_bot_names
            .iter()
            .map(|bot_name| text::get().start_command_sent(bot_name))
            .collect();
        report.skipped = self
            .skipped_running_bot_names
            .iter()
            .map(|bot_name| text::get().startall_skipped_running(bot_name))
            .collect();
        report.failed = self.failure_lines;

        report
    }
}

/// 将启动请求加入全局 FIFO 队列并等待本请求完成。
/// 队列按 `[startup_queue] max_batch_size` 分批；两次登录之间至少隔 `login_interval_ms`。
///
/// 空闲时工作线程阻塞等待新请求，并保留"上一次登录时刻"，防止连续命令绕过全局限流。
pub fn start_bot_sessions(bots_to_launch: &[BotDirEntry]) -> LaunchOutcome {
    if bots_to_launch.is_empty() {
        return LaunchOutcome::default();
    }

    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let request = StartRequest {
        bot_dir_entries: bots_to_launch.to_vec(),
        reply: reply_sender,
    };

    let queue_sender = match start_queue_sender() {
        Ok(sender) => sender,
        Err(error) => return queue_unavailable_outcome(bots_to_launch, &error),
    };
    if queue_sender.send(request).is_err() {
        return queue_unavailable_outcome(bots_to_launch, "start queue worker has stopped");
    }
    reply_receiver.recv().unwrap_or_else(|_| {
        queue_unavailable_outcome(bots_to_launch, "start queue worker has stopped")
    })
}

/// 创建队列不可用时，为本次请求的每台机器人生成失败明细。
fn queue_unavailable_outcome(bots_to_launch: &[BotDirEntry], reason: &str) -> LaunchOutcome {
    LaunchOutcome {
        started_bot_names: Vec::new(),
        skipped_running_bot_names: Vec::new(),
        attempted_launch_count: 0,
        failure_lines: bots_to_launch
            .iter()
            .map(|bot| text::get().start_launch_failed(&bot.bot_name, reason))
            .collect(),
    }
}

/// 全局启动工作线程：保持批次计数与冷却时刻跨请求存活。
fn process_start_queue(receiver: Receiver<StartRequest>) {
    let startup_queue = &config::get().startup_queue;
    let mut batch_gate = StartBatchGate::new(
        startup_queue.max_batch_size,
        Duration::from_millis(startup_queue.login_interval_ms),
        Duration::from_millis(config::get().timing.settle_per_bot_ms),
    );
    let mut pending_request = None;

    loop {
        let request = match pending_request.take() {
            Some(request) => request,
            None => match receiver.recv() {
                Ok(request) => request,
                Err(_) => break,
            },
        };
        let mut outcome = LaunchOutcome::default();
        let mut start_index = 0usize;

        while start_index < request.bot_dir_entries.len() {
            // 上一批已经结束（满了，或队列空闲时关掉了未满批次）：开新的一批。
            if batch_gate.remaining_capacity() == 0 {
                batch_gate.begin_next_batch();
            }

            // 两次登录之间至少隔 `login_interval_ms`：从"上一次登录时刻"算起，等够再拉下一批。
            thread::sleep(batch_gate.wait_before_next_login(Instant::now()));

            let remaining_batch_capacity = batch_gate.remaining_capacity();
            let end_index =
                (start_index + remaining_batch_capacity).min(request.bot_dir_entries.len());
            let batch_outcome =
                start_bot_sessions_now(&request.bot_dir_entries[start_index..end_index]);
            let attempted_launch_count = batch_outcome.attempted_launch_count;
            outcome.append(batch_outcome);
            batch_gate.record_attempts(attempted_launch_count, Instant::now());
            start_index = end_index;
        }

        // Keep a partial batch open if another request is already waiting, so FIFO requests can
        // share one batch. Close it only when the queue is empty; the login interval is kept
        // across idle either way, so consecutive commands cannot bypass the rate limit.
        let has_waiting_request = match receiver.try_recv() {
            Ok(next_request) => {
                pending_request = Some(next_request);
                true
            }
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => false,
        };
        if !has_waiting_request {
            batch_gate.finish_partial_batch();
        }

        // If the caller has already gone away, discard only its response; continue servicing the queue.
        let _ = request.reply.send(outcome);
    }
}

/// 在已由队列分批的单个批次内拉起机器人，并等待 `timing.settle_per_bot_ms` 后**复核两遍**会话是否还在。
/// 会话存在不等于已上线（登录、进服由 JVM 自己完成）。
///
/// 为什么要两遍：`screen -dmS` 建会话是瞬时的，而 JVM 可能在启动后一两秒就自己退出
/// （token 失效、jar 参数不对、被 OOM 杀）。只查一遍会把这种"起来就没了"报成启动成功。
fn start_bot_sessions_now(bots_to_launch: &[BotDirEntry]) -> LaunchOutcome {
    let mut outcome = LaunchOutcome::default();

    if bots_to_launch.is_empty() {
        return outcome;
    }

    // 批内相邻两台的间隔与批间间隔是同一个值（`[startup_queue] login_interval_ms`）：微软限流看的是
    // 登录频率，分成两个旋钮只会配出"批内几台连发"这种形状（见 troubleshooting 第 12 节）。
    let login_interval_ms = config::get().startup_queue.login_interval_ms;

    // 没被拉起过的机器人（前置检查或第一步就失败）：复核阶段要跳开，否则会多出一条误导信息。
    let mut not_launched_bot_names: Vec<String> = Vec::new();
    for (index, bot_dir_entry) in bots_to_launch.iter().enumerate() {
        let bot_name = &bot_dir_entry.bot_name;
        let bot_dir_path = &bot_dir_entry.bot_dir_path;

        // 队列等待期间，Token 状态和会话状态都可能改变；按执行时的磁盘与 screen 状态复核。
        let mut bot_dir_entry = bot_dir_entry.clone();
        bot_dir_entry.bot_token_status = super::bot_dirs::BotTokenStatus::from_dir(bot_dir_path);

        // 第零步：没有 Token 拒绝拉起。
        if !bot_dir_entry.bot_token_status.is_ready() {
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::StartSkippedMissingToken { bot_name })
            );
            not_launched_bot_names.push(bot_name.clone());
            outcome
                .failure_lines
                .push(text::get().start_skipped_missing_token(bot_name));
            continue;
        }

        if super::screen::is_bot_screen_running(bot_name) {
            not_launched_bot_names.push(bot_name.clone());
            outcome.skipped_running_bot_names.push(bot_name.clone());
            continue;
        }

        // 第一步：建这次运行的日志文件；建不出来就不拉会话。
        // 失败原因（带路径）写进程序日志，回复里只给结论。
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
                not_launched_bot_names.push(bot_name.clone());
                outcome
                    .failure_lines
                    .push(text::get().start_log_file_failed(bot_name));
                continue;
            }
        };

        // 第二步：让 FishingBot 自己的原始日志写向 /dev/null（磁盘上只留过滤后的日志）。
        let fishingbot_logs_dir = reset_fishingbot_logs_dir(bot_dir_path);

        // 第三步：拉起会话，JVM 的输出过滤后追加进第一步建好的日志文件。
        // 失败进"未拉起"名单，复核阶段跳过它。
        if let Err(message) =
            launch_bot_session(bot_name, bot_dir_path, &log_path, &fishingbot_logs_dir)
        {
            not_launched_bot_names.push(bot_name.clone());
            outcome.failure_lines.push(message);
        }
        outcome.attempted_launch_count += 1;

        // 新会话的状态要重置记忆的暂停状态。
        bot_name.forget_bot_pause_status();

        // 第四步：等够登录间隔再拉下一台（同时也是 JVM 启动错峰，避免多台同时启动把内存占满）。
        let is_last_bot = index + 1 == bots_to_launch.len();
        if !is_last_bot {
            thread::sleep(Duration::from_millis(login_interval_ms));
        }
    }

    // 没有实际启动的会话时无需等待与复核。
    if outcome.attempted_launch_count == 0 {
        return outcome;
    }

    // 等 JVM 初始化，让内存不足一类的问题先暴露出来再复核。
    let settle_ms = config::get()
        .timing
        .settle_per_bot_ms
        .saturating_mul(outcome.attempted_launch_count as u64);
    thread::sleep(Duration::from_millis(settle_ms));

    // 复核**两遍**，中间隔同样的时间：
    // 第一遍看会话有没有起来；第二遍还在的才算真起来了。只查一遍的话，"起来一两秒就自己退了"
    // （认证失败、jar 参数不对、被 OOM 杀）会报成"已启动"，随后 /show status 却什么都没有 ——
    // 用户完全看不出发生了什么（见 CONTEXT.md 的"回复口径不许夸大"）。
    let first_check_sessions = active_screen_sessions();
    thread::sleep(Duration::from_millis(settle_ms));
    let second_check_sessions = active_screen_sessions();

    for bot_dir_entry in bots_to_launch {
        let bot_name = &bot_dir_entry.bot_name;
        if not_launched_bot_names.contains(bot_name) {
            continue;
        }

        let Ok(target_session_name) = session_name(bot_name) else {
            // 名字非法时 `launch_bot_session` 已经失败并记过一条，这里不再补第二条。
            continue;
        };

        // 第二遍还在 = 启动成功（`-dmS` 偶尔慢一点也没关系：只有第一遍抓到也算成功）。
        if second_check_sessions.contains_key(&target_session_name) {
            outcome.started_bot_names.push(bot_name.clone());
        } else if first_check_sessions.contains_key(&target_session_name) {
            outcome
                .failure_lines
                .push(text::get().start_exited_soon(bot_name));
        } else {
            outcome
                .failure_lines
                .push(text::get().start_no_process(bot_name));
        }
    }

    outcome
}

#[cfg(test)]
mod start_queue_tests {
    use super::*;

    /// 间隔从"上一次登录"算起，而不是从批次完成算起：批次完成前的两遍复核等待要扣掉，
    /// 否则每一批都会白等一个复核窗口，实际间隔会大于配置值。
    #[test]
    fn gate_waits_the_interval_from_the_last_login_not_from_the_batch_end() {
        let interval = Duration::from_secs(20);
        let mut gate = StartBatchGate::new(3, interval, Duration::from_millis(300));
        let completed_at = Instant::now();

        assert_eq!(gate.remaining_capacity(), 3);
        gate.record_attempts(3, completed_at);

        // 三台一批：最后一次登录在完成时刻前 2 × 300ms × 3 = 1.8s，所以只需再等 18.2s。
        assert_eq!(gate.remaining_capacity(), 0);
        assert_eq!(
            gate.wait_before_next_login(completed_at),
            interval - Duration::from_millis(1_800)
        );
        assert_eq!(
            gate.wait_before_next_login(completed_at + interval),
            Duration::ZERO
        );

        gate.begin_next_batch();
        assert_eq!(gate.remaining_capacity(), 3);
    }

    /// 队列空闲时关掉未满批次：只影响"下一批从哪开始"，间隔仍然按上一次登录算，
    /// 所以连续命令绕不过限流。
    #[test]
    fn partial_batch_closed_on_idle_keeps_the_interval() {
        let interval = Duration::from_secs(20);
        let mut gate = StartBatchGate::new(3, interval, Duration::from_millis(300));
        let completed_at = Instant::now();

        gate.record_attempts(2, completed_at);
        assert_eq!(gate.remaining_capacity(), 1);

        gate.finish_partial_batch();
        assert_eq!(gate.remaining_capacity(), 0);

        // 两台：最后一次登录在完成时刻前 2 × 300ms × 2 = 1.2s，之后 4 秒里一直在倒计时。
        assert_eq!(
            gate.wait_before_next_login(completed_at + Duration::from_secs(4)),
            interval - Duration::from_millis(5_200)
        );
    }

    /// 排队中的下一条请求可以接着用这个未满批次（调度器不关它，也就不必多等一个间隔）。
    #[test]
    fn partial_batch_can_be_filled_by_the_next_queued_request() {
        let completed_at = Instant::now();
        let mut gate = StartBatchGate::new(3, Duration::from_secs(20), Duration::from_millis(300));

        gate.record_attempts(2, completed_at);
        assert_eq!(gate.remaining_capacity(), 1);

        // The dispatcher does not close this batch while a queued request is available.
        gate.record_attempts(1, completed_at);

        assert_eq!(gate.remaining_capacity(), 0);
    }

    #[test]
    fn zero_batch_size_is_safely_clamped_for_internal_callers() {
        let gate = StartBatchGate::new(0, Duration::from_secs(1), Duration::from_millis(300));
        assert_eq!(gate.remaining_capacity(), 1);
    }

    #[test]
    fn launch_outcomes_merge_without_losing_skips_or_batch_attempts() {
        let first = LaunchOutcome {
            started_bot_names: vec!["alpha".to_string()],
            skipped_running_bot_names: vec!["beta".to_string()],
            attempted_launch_count: 1,
            failure_lines: vec!["gamma failed".to_string()],
        };
        let second = LaunchOutcome {
            started_bot_names: vec!["delta".to_string()],
            skipped_running_bot_names: Vec::new(),
            attempted_launch_count: 1,
            failure_lines: Vec::new(),
        };

        let mut merged = LaunchOutcome::default();
        merged.append(first);
        merged.append(second);

        assert_eq!(merged.started_bot_names, ["alpha", "delta"]);
        assert_eq!(merged.skipped_running_bot_names, ["beta"]);
        assert_eq!(merged.failure_lines, ["gamma failed"]);
        assert_eq!(merged.attempted_launch_count, 2);
    }

    #[test]
    fn launch_outcomes_report_skipped_running_sessions_as_non_successes() {
        let outcome = LaunchOutcome {
            started_bot_names: Vec::new(),
            skipped_running_bot_names: vec!["duck".to_string()],
            attempted_launch_count: 0,
            failure_lines: Vec::new(),
        };

        assert!(!outcome.has_failed_lines());
        assert!(outcome.has_skipped_running_bots());
        assert!(outcome.into_report().succeeded.is_empty());
    }
}

/// 拉起一次"不由 `/start` 触发"的运行：建日志、准备原始日志目录、拉起会话，返回本次日志路径。
///
/// 给 `/bot login` 用（见 [`crate::bot_manage`]）：它要在一个**还没有 Token** 的机器人上拉起会话，
/// 而 [`start_bot_sessions`] 的第零步正是按 Token 状态拒掉这种机器人。其余三步与它完全一样，
/// 所以这里直接复用同一批私有实现（同一份命令拼接、同一份复核口径），不另写一份。
pub fn prepare_and_launch_bot(bot_name: &str, bot_dir_path: &Path) -> Result<PathBuf, String> {
    // 与 `start_bot_sessions` 一样：日志建不出来就不拉会话，而 `create_bot_log_file` 的 `Err`
    // 是给日志看的原文，回复用那句"建日志失败"。
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

    let fishingbot_logs_dir = reset_fishingbot_logs_dir(bot_dir_path);
    launch_bot_session(bot_name, bot_dir_path, &log_path, &fishingbot_logs_dir)?;

    Ok(log_path)
}

/// 拉起单个机器人的会话：只做一件事，校验与准备都由 [`start_bot_sessions`] 的前两步完成。
fn launch_bot_session(
    bot_name: &str,
    bot_dir_path: &Path,
    log_path: &Path,
    fishingbot_logs_dir: &Path,
) -> Result<(), String> {
    let target_session_name = session_name(bot_name)?;
    let launch_command = build_launch_command(log_path, fishingbot_logs_dir);

    let mut cmd = Command::new("screen");
    // 会话的工作目录就是机器人目录（FishingBot 以它为基准找相对路径）。
    cmd.current_dir(bot_dir_path);
    // -dmS：后台分离模式创建会话并执行命令。
    cmd.args(["-dmS", &target_session_name, "bash", "-c", &launch_command]);
    cmd.status()
        .map_err(|e| text::get().start_launch_failed(bot_name, &e.to_string()))?;

    log::info!(
        "{}",
        text::get().log_text(text::LogText::StartSessionLaunched {
            bot_name,
            session_name: &target_session_name,
            log_path: &log_path.display().to_string(),
        })
    );
    Ok(())
}

/// 拉起会话时要拼进命令的配置取值：一次给全，字段都有名字，不会传错位置。
///
/// 这几个字段散在配置的三个表里（`runtime` / `paths` / `keywords`），
/// 收成一个参数结构体是为了让 [`launch_command_line`] 只依赖"值"、不依赖"配置放在哪"。
#[derive(Debug, Clone, Copy)]
pub struct LaunchCommandConfig<'a> {
    /// `runtime.java_opts`：JVM 参数，未加引号内联进命令。
    pub java_opts: &'a str,
    /// `runtime.token_file`：Token 文件名（相对机器人目录）。
    pub token_file_name: &'a str,
    /// `keywords.filter_regex`：丢掉刷屏行的正则（传给 `grep -v -E`）。
    pub filter_regex: &'a str,
    /// `paths.jar`：FishingBot 的 jar。
    pub jar: &'a Path,
    /// `paths.fishingbot_config_file_name`：FishingBot 的共享配置（展开后的完整路径）。
    pub fishingbot_config: &'a Path,
}

/// 拼出会话里执行的启动脚本：跑 JVM → 过滤 → 追加进日志文件。
///
/// 纯函数（取值全部由调用方给），所以这条**最危险**的命令能对着固定取值逐字断言 ——
/// 见本文件测试；真正读配置的是 [`build_launch_command`]。
///
/// * `-logsdir` 指向 FishingBot 的日志目录（里面是指向 `/dev/null` 的符号链接，内容丢弃）；
/// * `grep --line-buffered -v -E` 丢掉刷屏行（**命中即丢**），`--line-buffered` 让日志实时落盘，
///   而不是等管道缓冲满。
pub fn launch_command_line(
    log_path: &Path,
    fishingbot_logs_dir: &Path,
    config: LaunchCommandConfig<'_>,
) -> String {
    // java_opts 为空时不补空格，避免参数与 -jar 粘连。
    let jvm_opts = if config.java_opts.trim().is_empty() {
        String::new()
    } else {
        format!("{} ", config.java_opts.trim())
    };

    format!(
        concat!(
            "java {jvm_opts}-jar '{jar}' -nogui -config '{fishingbot_config}' ",
            "-refreshToken '{token_file}' -logsdir '{logsdir}' 2>&1 ",
            "| grep --line-buffered -v -E '{filter_regex}' | tee -a '{log_path}'"
        ),
        jvm_opts = jvm_opts,
        jar = config.jar.display(),
        fishingbot_config = config.fishingbot_config.display(),
        token_file = config.token_file_name,
        logsdir = fishingbot_logs_dir.display(),
        filter_regex = config.filter_regex,
        log_path = log_path.display()
    )
}

/// 启动流程的取值层：只读拼命令要用的那五个字段，交给 [`launch_command_line`]。
fn build_launch_command(log_path: &Path, fishingbot_logs_dir: &Path) -> String {
    let settings = config::get();

    launch_command_line(
        log_path,
        fishingbot_logs_dir,
        LaunchCommandConfig {
            java_opts: &settings.runtime.java_opts,
            token_file_name: &settings.runtime.token_file,
            filter_regex: &settings.keywords.filter_regex,
            jar: &settings.paths.jar,
            fishingbot_config: &settings.paths.fishingbot_config,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 启动命令的拼接：JVM 输出过滤后追加进日志（`-logsdir` 指向丢弃目标）。
    ///
    /// 这条命令错了就拉不起机器人，所以按"逐字对照"钉住；期望串里的路径也用 `display()` 拼，
    /// 免得在 Windows 上跑测试时因为分隔符不同而误报。
    #[test]
    fn launch_command_is_built_as_expected() {
        let log_path = Path::new("/srv/bots/1/duck/logs/2026-09-28-01-47-01.log");
        let fishingbot_logs_dir = Path::new("/srv/bots/1/duck/logs/raw");
        let jar = Path::new("/fishingbots/FishingBot.jar");
        let fishingbot_config = Path::new("/fishingbots/config.json");

        let command = launch_command_line(
            log_path,
            fishingbot_logs_dir,
            LaunchCommandConfig {
                java_opts: "-Xms32m -Xmx128m",
                token_file_name: "refreshToken",
                filter_regex: "XP|游戏币",
                jar,
                fishingbot_config,
            },
        );

        assert_eq!(
            command,
            format!(
                "java -Xms32m -Xmx128m -jar '{}' -nogui -config '{}' -refreshToken 'refreshToken' \
                 -logsdir '{}' 2>&1 | grep --line-buffered -v -E 'XP|游戏币' | tee -a '{}'",
                jar.display(),
                fishingbot_config.display(),
                fishingbot_logs_dir.display(),
                log_path.display()
            )
        );
    }

    /// `java_opts` 为空（或只有空白）时不能在 `java` 与 `-jar` 之间留出多余空格。
    #[test]
    fn empty_java_opts_leaves_no_gap() {
        let command = launch_command_line(
            Path::new("/tmp/1.log"),
            Path::new("/tmp/raw"),
            LaunchCommandConfig {
                java_opts: "   ",
                token_file_name: "refreshToken",
                filter_regex: "XP",
                jar: Path::new("/tmp/FishingBot.jar"),
                fishingbot_config: Path::new("/tmp/config.json"),
            },
        );

        assert!(command.starts_with("java -jar "), "{command}");
    }

    /// FishingBot 的 raw 日志目录与文件都要准备好：它建不出文件会直接拒绝启动。
    #[test]
    fn game_logs_dir_is_prepared() {
        let bot_logs_dir = temp_test_dir("prepare-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");

        assert_eq!(game_logs_dir, raw_logs_dir);
        assert!(game_logs_dir.is_dir());
        assert!(game_logs_dir.join("log0.log").exists(), "日志文件要能写");

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// "启动时清空 `raw/`"这条规矩要能发现 FishingBot 换了日志文件名：
    /// 上一轮留下的意料之外文件会被**报出来**（随后整目录被清掉）；正常情况不该有任何告警。
    #[test]
    fn unexpected_game_logs_are_reported_before_cleanup() {
        let bot_logs_dir = temp_test_dir("unexpected-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null").is_empty(),
            "正常情况下 raw/ 里只有指向 /dev/null 的 log0.log"
        );

        // 模拟 FishingBot 换了文件名：多出一个**有内容的普通文件**。
        fs::write(game_logs_dir.join("log1.log"), "未过滤的刷屏行\n")
            .expect("造一个意料之外的文件");

        assert_eq!(
            unexpected_game_log_file_names(&game_logs_dir, "/dev/null"),
            vec!["log1.log".to_string()]
        );

        // 再走一次"启动清理"：它必须被清掉（磁盘上只留我们过滤后的那一份）。
        let game_logs_dir_again = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        assert!(
            !game_logs_dir_again.join("log1.log").exists(),
            "启动清理应当把意料之外的文件清掉"
        );

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 写进 FishingBot 日志文件的内容必须被丢掉（它是 `/dev/null` 的符号链接）：
    /// 磁盘上只能有一份日志，也就是我们的那份。
    #[test]
    fn game_log_file_discards_writes() {
        let bot_logs_dir = temp_test_dir("discard-game-log");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_log_file =
            reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null").join("log0.log");
        assert_eq!(
            fs::read_link(&game_log_file).expect("应当是符号链接"),
            Path::new("/dev/null")
        );

        fs::write(&game_log_file, "未过滤的刷屏行\n").expect("写 FishingBot 的日志");
        assert_eq!(fs::metadata(&game_log_file).expect("看文件大小").len(), 0);

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 上一次运行留下的东西不保留：`raw/` 每次启动前被清空重建。
    #[test]
    fn resetting_game_logs_dir_clears_previous_run() {
        let bot_logs_dir = temp_test_dir("reset-game-logs");
        let raw_logs_dir = target_logs_dir(&bot_logs_dir, "raw");

        let game_logs_dir = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");
        fs::write(game_logs_dir.join("log1.log"), "上一轮留下的\n").expect("造一个上一轮的文件");

        let game_logs_dir_again = reset_raw_logs_dir(&raw_logs_dir, "log0.log", "/dev/null");

        assert_eq!(game_logs_dir_again, game_logs_dir);
        assert!(
            !game_logs_dir.join("log1.log").exists(),
            "上一轮的文件不该被保留"
        );
        assert!(game_logs_dir.join("log0.log").exists());

        fs::remove_dir_all(&bot_logs_dir).expect("清理临时目录");
    }

    /// 每个测试用一个独立的临时目录（带进程号，避免并发跑测试时互相踩）。
    use crate::test_support::temp_test_dir;
}
