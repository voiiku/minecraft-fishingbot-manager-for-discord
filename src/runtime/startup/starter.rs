//! 按批次启动排队中的机器人并记录结果；`/bot login` 复用会话启动逻辑但不入队。

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::text;
use crate::utils::{BotDirEntry, BotPauseStatusMemory, is_bot_screen_running, session_name};

use super::ledger::StartLedger;
use super::{BotStartOutcome, QueuedBot, StartupQueue};

/// 启动线程 panic 后的重启退避时间。
const STARTER_RESTART_BACKOFF: Duration = Duration::from_secs(1);

/// 常驻启动线程入口；捕获 panic 并在退避后重新运行。
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

/// 从 panic 载荷提取原因。
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
    /// 返回启动线程的创建结果。
    pub(super) fn starter_result(&self) -> Result<(), String> {
        self.starter.get().cloned().unwrap_or(Ok(()))
    }

    /// 等待队列和批间间隔，逐台启动并记录结果；复核由调用方执行。
    pub(super) fn run_starter(&self) {
        loop {
            self.wait_until_work();

            // 消费停止请求，避免影响后续批次。
            if self.take_stop_request() {
                continue;
            }

            thread::sleep(remaining_batch_wait(
                Duration::from_millis(self.batch_login_interval_ms),
                self.last_batch_dequeued_at(),
                Instant::now(),
            ));

            // 批间等待期间可能收到停止请求。
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

    /// 启动请求的会话存活宽限期。
    pub(super) fn check_grace(&self) -> Duration {
        Duration::from_millis(self.check_grace_ms)
    }

    /// 按批内间隔逐台启动，并将结果写入对应账本。
    fn start_single_batch(&self, mut entries: Vec<QueuedBot>) {
        for index in 0..entries.len() {
            // 剩余条目释放时由 `Drop` 记录取消。
            if self.take_stop_request() {
                return;
            }

            // 单台启动发生 panic 时仍须记录结果，避免调用方持续等待。
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

            let is_last_bot = index + 1 == entries.len();
            if !is_last_bot {
                thread::sleep(Duration::from_millis(self.bot_login_interval_ms));
            }
        }
    }

    /// 检查会话并启动机器人；凭据等前置校验由调用方负责。
    fn launch_one_bot(bot_dir_entry: &BotDirEntry) -> BotStartOutcome {
        let bot_name = &bot_dir_entry.bot_name;
        let bot_dir_path = &bot_dir_entry.bot_dir_path;

        match is_bot_screen_running(bot_name) {
            Ok(true) => return BotStartOutcome::Skipped,
            Ok(false) => {}
            Err(error) => {
                return BotStartOutcome::Failure(text::get().start_launch_failed(bot_name, &error));
            }
        }

        let launch_result = Self::create_logs_and_launch_bot_session(bot_name, bot_dir_path);
        // 新会话需要重置暂停状态。
        bot_name.forget_bot_pause_status();

        launch_result.map_or_else(BotStartOutcome::Failure, |_log_path| {
            BotStartOutcome::Succeeded
        })
    }
}

/// 计算距上次取出批次后的剩余等待时间。
fn remaining_batch_wait(
    interval: Duration,
    last_batch_dequeued_at: Instant,
    now: Instant,
) -> Duration {
    interval.saturating_sub(now.saturating_duration_since(last_batch_dequeued_at))
}

/// 为每个机器人记录启动失败明细。
pub(super) fn record_launch_failures(ledger: &StartLedger, bots: &[BotDirEntry], reason: &str) {
    let i18n_text = text::get();

    for bot_dir_entry in bots {
        let failure_line = i18n_text.start_launch_failed(&bot_dir_entry.bot_name, reason);
        ledger.record_failure(bot_dir_entry, failure_line);
    }
}

use std::process::Command;

use crate::config;

/// 通过 `screen -dmS` 在后台启动会话。
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
    // FishingBot 相对路径以机器人目录为基准。
    cmd.current_dir(bot_dir_path);
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

/// 生成启动 JVM、过滤输出并追加日志的脚本。
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

    use crate::utils::BotStartResults;

    use super::*;

    /// 创建指定节奏参数（每批 3 台、批内 20s、批间 30s、复核等待 300ms）的测试队列。
    fn queue() -> StartupQueue {
        StartupQueue::new_for_test(3, 20_000, 30_000, 300, 32)
    }

    /// 复核等待时间按整批最坏情况计算，与本次调用数量无关。
    #[test]
    fn verify_wait_covers_a_full_batch() {
        assert_eq!(queue().check_grace(), Duration::from_millis(300));
    }

    /// 批间等待从上一次取出批次开始计算；间隔已结束时无需等待。
    #[test]
    fn the_wait_between_batches_counts_from_the_last_pop() {
        let interval = Duration::from_secs(20);
        let now = Instant::now();

        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(60), now),
            Duration::ZERO,
            "超过批间间隔后不应继续等待"
        );
        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(3), now),
            Duration::from_secs(17),
            "只等待剩余间隔"
        );
        assert_eq!(
            remaining_batch_wait(interval, now - Duration::from_secs(20), now),
            Duration::ZERO,
            "间隔恰好结束时等待时间为 0"
        );
    }

    #[cfg(test)]
    mod launch_command_tests {
        use super::*;

        /// 验证启动命令行：JVM 输出过滤后追加到日志，`-logsdir` 指向丢弃目标。
        /// 路径使用 `display()` 拼接，避免平台路径分隔符差异导致误报。
        #[test]
        fn launch_command_is_built_as_expected() {
            let log_path = Path::new("/srv/bots/1/duck/logs/2026-09-28-01-47-01.log");
            let fishingbot_logs_dir = Path::new("/srv/bots/1/duck/logs/raw");
            let jar = Path::new("/fishingbots/FishingBot.jar");
            let fishingbot_config = Path::new("/fishingbots/config.json");

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

        /// `java_opts` 为空或仅含空白时，不得在 `java` 与 `-jar` 之间产生多余空格。
        #[test]
        fn empty_java_opts_leaves_no_gap() {
            let command = launch_command_line(
                Path::new("/tmp/1.log"),
                Path::new("/tmp/raw"),
                "   ",
                "refreshToken",
                "XP",
                Path::new("/tmp/FishingBot.jar"),
                Path::new("/tmp/config.json"),
            );

            assert!(command.starts_with("java -jar "), "{command}");
        }
    }

    /// 一条启动结果：机器人条目 + 那台的结果（明细里只用到名字，条目本身是复核要用的）。
    fn launched_bot(
        bot_name: &str,
        outcome: BotStartOutcome,
    ) -> (crate::utils::BotDirEntry, BotStartOutcome) {
        let bot_dir_entry = crate::utils::BotDirEntry {
            owner_id: 1,
            bot_name: bot_name.to_string(),
            bot_dir_path: std::path::PathBuf::from("/nonexistent"),
            bot_token_status: crate::utils::BotTokenStatus::Missing,
        };

        (bot_dir_entry, outcome)
    }

    /// 一批启动结果（测试摆结果用）。
    fn results(entries: Vec<(crate::utils::BotDirEntry, BotStartOutcome)>) -> BotStartResults {
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
