//! 按批次启动排队中的机器人并记录结果；`/bot login` 复用会话启动逻辑但不入队。

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::config;
use crate::runtime::bot_instances::bot_dirs::BotDirEntry;
use crate::runtime::bot_instances::screen::{is_bot_screen_running, session_name};
use crate::runtime::bot_log::{create_bot_log_file, reset_fishingbot_raw_logs_dir};

use super::ledger::StartLedger;
use super::{QueuedBot, StartIssue, StartupQueue};

/// 常驻启动线程入口；捕获 panic 并在退避后重新运行。
pub(super) fn run_starter_forever(queue: &'static StartupQueue) -> ! {
    loop {
        if let Err(panic) = panic::catch_unwind(AssertUnwindSafe(|| queue.run_starter())) {
            let reason = panic_reason(&panic);
            log::error!(
                "{}",
                crate::bootstrap_error_text().starter_restarting(&reason)
            );
            thread::sleep(config::Config::get().constants.starter_restart_backoff);
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
                            crate::config::Config::get()
                                .text()
                                .start_launch_panicked(&bot_dir_entry.bot_name, &reason)
                        );
                        Err(StartIssue::Failure(
                            crate::config::Config::get().text().start_launch_failed(
                                &bot_dir_entry.bot_name,
                                crate::config::Config::get()
                                    .text()
                                    .start_internal_error_reason(),
                            ),
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
    fn launch_one_bot(bot_dir_entry: &BotDirEntry) -> Result<(), StartIssue> {
        let bot_name = &bot_dir_entry.bot_name;
        let bot_dir_path = &bot_dir_entry.bot_dir_path;

        match is_bot_screen_running(bot_name) {
            Ok(true) => return Err(StartIssue::AlreadyRunning),
            Ok(false) => {}
            Err(error) => {
                return Err(StartIssue::Failure(
                    crate::config::Config::get()
                        .text()
                        .start_launch_failed(bot_name, &error),
                ));
            }
        }

        create_logs_and_launch_bot_session(bot_name, bot_dir_path)
            .map(|_| ())
            .map_err(StartIssue::Failure)
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
    let i18n_text = crate::config::Config::get().text();

    for bot_dir_entry in bots {
        let failure_line = i18n_text.start_launch_failed(&bot_dir_entry.bot_name, reason);
        ledger.record_failure(bot_dir_entry, failure_line);
    }
}

/// 创建日志并启动机器人会话；不入队或复核。
pub(crate) fn create_logs_and_launch_bot_session(
    bot_name: &str,
    bot_dir_path: &Path,
) -> Result<PathBuf, String> {
    let log_path = match create_bot_log_file(bot_dir_path) {
        Ok(log_path) => log_path,
        Err(message) => {
            log::error!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .start_log_file_failed_log_message(bot_name, &message)
            );
            return Err(crate::config::Config::get()
                .text()
                .start_log_file_failed(bot_name));
        }
    };

    let fishingbot_logs_dir = reset_fishingbot_raw_logs_dir(bot_dir_path);
    launch_bot_session(bot_name, bot_dir_path, &log_path, &fishingbot_logs_dir)?;
    Ok(log_path)
}

/// 通过 `screen -dmS` 在后台启动会话。
fn launch_bot_session(
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
    cmd.status().map_err(|e| {
        crate::config::Config::get()
            .text()
            .start_launch_failed(bot_name, &e.to_string())
    })?;

    log::info!(
        "{}",
        crate::config::Config::get().text().start_session_launched(
            bot_name,
            &target_session_name,
            &log_path.display().to_string()
        )
    );
    Ok(())
}

/// 生成启动 JVM、过滤输出并追加日志的脚本。
fn launch_command_line(
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
}
