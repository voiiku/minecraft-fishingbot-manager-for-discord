//! 线程与启动的具体逻辑
//!
//! * 命令侧（[`StartupQueue::start_bots`]）：登记 → 入队 → 等自己的条目都被拉起过一遍 → 交给账本复核。
//! * 线程侧（[`StartupQueue::run_starter`]）：等队列 → 等批间间隔 → 取一批 → 逐台拉起 → 交给账本。
//! * `/bot login` 走 [`StartupQueue::add_bot_for_login`]（不进队列，与逐台拉起共用同一份实现）。

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::text;
use crate::utils::bot_dirs::BotDirEntry;
use crate::utils::bot_status::BotPauseStatusMemory;
use crate::utils::screen::{is_bot_screen_running, session_name};

use crate::utils::reply::BotStartOutcome;

use super::ledger::StartLedger;
use super::{QueuedBot, StartupQueue};

/// 队列放不下时，记进账本的失败明细里的原因。
pub(super) const QUEUE_FULL_REASON: &str = "启动队列已满（max_pending_requests）";

/// 启动线程 panic 后、重启它之前的等待（避免连续 panic 时刷屏）。
const STARTER_RESTART_BACKOFF: Duration = Duration::from_secs(1);

/// 启动线程的入口：跑 [`StartupQueue::run_starter`]；它一旦挂掉（panic）就记日志、等一会儿再重启。
///
/// 这个函数不返回：启动线程是常驻的，只有重启，没有退出。
pub(super) fn run_starter_forever(queue: &'static StartupQueue) -> ! {
    loop {
        if let Err(panic) = panic::catch_unwind(AssertUnwindSafe(|| queue.run_starter())) {
            let reason = panic_reason(&panic);
            log::error!(
                "{}",
                text::get().log_text(text::LogText::StarterRestarting { reason: &reason })
            );
            thread::sleep(STARTER_RESTART_BACKOFF);
        }
    }
}

/// 从 `catch_unwind` 的载荷里取出可读原因。
fn panic_reason(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = panic.downcast_ref::<String>() {
        return message.clone();
    }

    "unknown panic payload".to_string()
}

impl StartupQueue {
    /// 启动线程起没起来（没起来时每次调用直接给失败报告）。
    pub(super) fn starter_result(&self) -> Result<(), String> {
        self.starter.get().cloned().unwrap_or(Ok(()))
    }

    /// 启动线程主体：等队列 → 等够批间间隔 → 取一批 → 逐台拉起（复核由提交的命令做）。
    pub(super) fn run_starter(&self) {
        loop {
            // 等待队列非空或收到停止拉起请求（写入队列或请求停止时都会被唤醒）。
            self.wait_until_work();

            // 收到停止拉起请求：立即取走（取到即清除，避免影响后续批次），本轮不取批。
            if self.take_stop_request() {
                continue;
            }

            // 批与批之间固定等一个间隔：从"上一次取出一批"算起，扣掉已经过去的部分。
            thread::sleep(remaining_batch_wait(
                Duration::from_millis(self.batch_login_interval_ms),
                self.latest_poped_timestamp(),
                Instant::now(),
            ));

            // 等待批间间隔期间也可能收到停止请求：同样不取批（此时等待列表已被清空）。
            if self.take_stop_request() {
                continue;
            }

            let bots = self.pop_bots_from_head(self.max_batch_size());
            if bots.is_empty() {
                continue;
            }

            self.start_single_batch(bots);
        }
    }

    /// 复核的宽限期：每台拉起后观察其会话存活多久（见 `timing.check_grace_ms`）。
    pub(super) fn check_grace(&self) -> Duration {
        Duration::from_millis(self.check_grace_ms)
    }

    /// 拉起一批：逐台拉起（批内错峰），结果写回各自的账本。
    fn start_single_batch(&self, mut entries: Vec<QueuedBot>) {
        for index in 0..entries.len() {
            // 拉起这一台之前检查停止拉起请求：命中则直接返回，
            // 这一台与其余条目随 `entries` 一起释放，由各自的 `Drop` 按"取消"写回账本。
            if self.take_stop_request() {
                return;
            }

            // 单台拉起 panic 时同样要把结果写回账本，否则等待结果的调用方会永久阻塞。
            let outcome = {
                let bot_dir_entry = entries[index].bot();
                panic::catch_unwind(AssertUnwindSafe(|| Self::launch_one_bot(bot_dir_entry)))
                    .unwrap_or_else(|panic| {
                        let reason = panic_reason(&panic);
                        log::error!(
                            "{}",
                            text::get().log_text(text::LogText::StartLaunchPanicked {
                                bot_name: &bot_dir_entry.bot_name,
                                reason: &reason,
                            })
                        );
                        BotStartOutcome::Failure(text::get().start_launch_failed(
                            &bot_dir_entry.bot_name,
                            text::get().start_internal_error_reason(),
                        ))
                    })
            };
            entries[index].record(outcome);

            // 批内相邻两台也隔一个间隔（JVM 启动错峰，别把内存占满）。
            let is_last_bot = index + 1 == entries.len();
            if !is_last_bot {
                thread::sleep(Duration::from_millis(self.bot_login_interval_ms));
            }
        }
    }

    /// 拉起一台机器人：建日志 → 准备原始日志目录 → 拉起会话（Token 之类的前置校验由调用方负责）。
    fn launch_one_bot(bot_dir_entry: &BotDirEntry) -> BotStartOutcome {
        let bot_name = &bot_dir_entry.bot_name;
        let bot_dir_path = &bot_dir_entry.bot_dir_path;

        // 已经在跑的不再拉起（跳过，不算失败）。
        if is_bot_screen_running(bot_name) {
            return BotStartOutcome::Skipped;
        }

        // 与 `/bot login` 共用同一份实现（见 [`Self::add_bot_for_login`]）。
        let launch_result = Self::add_bot_for_login(bot_name, bot_dir_path);
        // 拉起命令发出去了，新会话的状态要重置记忆的暂停状态。
        bot_name.forget_bot_pause_status();

        launch_result.map_or_else(BotStartOutcome::Failure, |_log_path| {
            BotStartOutcome::Succeeded
        })
    }
}

/// 这一批还要等多久：`interval` 减去"距离上一次取出一批"已经过去的部分。
fn remaining_batch_wait(
    interval: Duration,
    latest_poped_timestamp: Instant,
    now: Instant,
) -> Duration {
    interval.saturating_sub(now.saturating_duration_since(latest_poped_timestamp))
}

/// 这几台各记一条失败明细（原因拼成能直接进回复的一行）。
pub(super) fn record_launch_failures(ledger: &StartLedger, bots: &[BotDirEntry], reason: &str) {
    let i18n_text = text::get();

    for bot_dir_entry in bots {
        let failure_line = i18n_text.start_launch_failed(&bot_dir_entry.bot_name, reason);
        ledger.record_failure(bot_dir_entry, failure_line);
    }
}

// 拉起会话：拼启动命令（纯函数，可逐字断言）与 `screen -dmS`。

use std::process::Command;

use crate::config;

/// 把会话拉起来：`screen -dmS` 后台分离跑 `bash -c <启动命令>`。
pub(super) fn launch_bot_session(
    bot_name: &str,
    bot_dir_path: &Path,
    log_path: &Path,
    fishingbot_logs_dir: &Path,
) -> Result<(), String> {
    let settings = config::Config::get();
    let target_session_name = session_name(bot_name)?;
    let launch_command = launch_command_line(
        log_path,
        fishingbot_logs_dir,
        &settings.runtime.java_opts,
        &settings.runtime.token_file,
        &settings.keywords.filter_regex,
        &settings.paths.jar,
        &settings.paths.fishingbot_config,
    );

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

/// 拼出会话里执行的启动脚本：跑 JVM → 过滤 → 追加进日志文件。
///
/// 纯函数（取值全部由调用方给），所以这条最危险的命令能对着固定取值逐字断言（见下方测试）。
pub(super) fn launch_command_line(
    log_path: &Path,
    fishingbot_logs_dir: &Path,
    java_opts: &str,
    token_file_name: &str,
    filter_regex: &str,
    jar: &Path,
    fishingbot_config: &Path,
) -> String {
    // java_opts 为空时不补空格，避免参数与 -jar 粘连。
    let jvm_opts = if java_opts.trim().is_empty() {
        String::new()
    } else {
        format!("{} ", java_opts.trim())
    };

    format!(
        concat!(
            "java {jvm_opts}-jar '{jar}' -nogui -config '{fishingbot_config}' ",
            "-refreshToken '{token_file}' -logsdir '{logsdir}' 2>&1 ",
            "| grep --line-buffered -v -E '{filter_regex}' | tee -a '{log_path}'"
        ),
        jvm_opts = jvm_opts,
        jar = jar.display(),
        fishingbot_config = fishingbot_config.display(),
        token_file = token_file_name,
        logsdir = fishingbot_logs_dir.display(),
        filter_regex = filter_regex,
        log_path = log_path.display()
    )
}

#[cfg(test)]
mod tests {

    use crate::utils::reply::BotStartResults;

    use super::*;

    /// 每批最多 3 台、批内间隔 20s、批间间隔 30s、每台复核等待 300ms 的队列。
    fn queue() -> StartupQueue {
        StartupQueue::new_for_test(3, 20_000, 30_000, 300, 32)
    }

    /// 复核等待按整批最坏情况算，与"这次调用有几台"无关。
    #[test]
    fn verify_wait_covers_a_full_batch() {
        assert_eq!(queue().check_grace(), Duration::from_millis(300));
    }

    /// 批间等待从"上一次取出一批"算起；间隔已经过完（例如空闲很久后的第一条命令）就不用等。
    #[test]
    fn the_wait_between_batches_counts_from_the_last_pop() {
        let interval = Duration::from_secs(20);
        let now = Instant::now();

        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(60), now),
            Duration::ZERO,
            "空闲很久之后的第一条命令不该白等"
        );
        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(3), now),
            Duration::from_secs(17),
            "只等剩下的那部分"
        );
        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(20), now),
            Duration::ZERO,
            "间隔刚好过完就不再等"
        );
    }

    #[cfg(test)]
    mod launch_command_tests {
        use super::*;

        /// 启动命令的拼接：JVM 输出过滤后追加进日志（`-logsdir` 指向丢弃目标）。
        ///
        /// 这条命令错了就拉不起机器人，所以按逐字对照钉住；期望串里的路径也用 `display()` 拼，
        /// 免得 Windows 上因为分隔符不同而误报。
        #[test]
        fn launch_command_is_built_as_expected() {
            let log_path = Path::new("/srv/bots/1/duck/logs/2026-09-28-01-47-01.log");
            let fishingbot_logs_dir = Path::new("/srv/bots/1/duck/logs/raw");
            let jar = Path::new("/fishingbots/FishingBot.jar");
            let fishingbot_config = Path::new("/fishingbots/conf.json");

            let command = launch_command_line(
                log_path,
                fishingbot_logs_dir,
                "-Xms32m -Xmx128m",
                "refreshToken",
                "XP|游戏币",
                jar,
                fishingbot_config,
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
                "   ",
                "refreshToken",
                "XP",
                Path::new("/tmp/FishingBot.jar"),
                Path::new("/tmp/conf.json"),
            );

            assert!(command.starts_with("java -jar "), "{command}");
        }
    }

    /// 一条启动结果：机器人条目 + 那台的结果（明细里只用到名字，条目本身是复核要用的）。
    fn launched_bot(
        bot_name: &str,
        outcome: BotStartOutcome,
    ) -> (crate::utils::bot_dirs::BotDirEntry, BotStartOutcome) {
        let bot_dir_entry = crate::utils::bot_dirs::BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: std::path::PathBuf::from("/nonexistent"),
            bot_token_status: crate::utils::bot_dirs::BotTokenStatus::Missing,
        };

        (bot_dir_entry, outcome)
    }

    /// 一批启动结果（测试摆结果用）。
    fn results(
        entries: Vec<(crate::utils::bot_dirs::BotDirEntry, BotStartOutcome)>,
    ) -> BotStartResults {
        let mut results = BotStartResults::default();
        for (bot_dir_entry, outcome) in entries {
            results.push(bot_dir_entry, outcome);
        }

        results
    }

    /// 全跳过：没有失败，也不是成功。
    #[test]
    fn launch_results_report_skipped_running_sessions_as_non_successes() {
        let results = results(vec![launched_bot("duck", BotStartOutcome::Skipped)]);

        assert!(!results.has_failed_lines());
        assert!(results.has_skipped_running_bots());
        assert_eq!(results.skipped_running_bot_names(), ["duck"]);
        assert!(results.into_report().succeeded.is_empty());
    }

    /// 明细按逐台结果分三份：成功进成功、跳过进跳过、失败进失败。
    #[test]
    fn a_report_is_built_from_the_per_bot_results() {
        let results = results(vec![
            launched_bot("duck", BotStartOutcome::Succeeded),
            launched_bot("goose", BotStartOutcome::Skipped),
            launched_bot(
                "crane",
                BotStartOutcome::Failure("拉起命令失败".to_string()),
            ),
        ]);

        assert!(results.has_failed_lines());
        assert_eq!(results.failure_lines(), ["拉起命令失败"]);
        assert_eq!(results.to_failure_text(), "拉起命令失败");
        assert_eq!(results.skipped_running_bot_names(), ["goose"]);

        let report = results.into_report();
        assert_eq!(report.failed, ["拉起命令失败"]);
        assert_eq!(report.succeeded.len(), 1);
        assert_eq!(report.skipped.len(), 1);
    }
}
