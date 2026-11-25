//! 程序与框架日志同时写入终端和 `<部署目录>/logs/时间戳.log`。
//! 日志文件不可用时将只写终端；级别由配置里的 `[log] debug` 决定。
//! 级别判定和记录渲染分别位于 `runtime/process_log/levels.rs` 与 `runtime/process_log/format.rs`。

mod format;
mod levels;

use std::fs::File;
use std::io::{self, Write};
use std::sync::Mutex;

use log::{Log, Metadata, Record};

use crate::runtime::log_files::create_timestamped_log_file;
use crate::{PROCESS_LOGGER, config};

use format::{format_record, timestamp};
use levels::LogLevels;

/// 程序日志：终端（stderr）与文件各一份，`impl log::Log`。
///
/// 进程内唯一的那一份由 [`ProcessLog::get_or_init`] 取（首次调用建日志文件并把它装上）。
pub struct ProcessLog {
    /// 日志级别上限。
    levels: LogLevels,
    /// 日志文件句柄（建不出来时为 `None`，那时只写终端）。
    file: Option<Mutex<File>>,
}

impl ProcessLog {
    /// 在配置加载后安装进程 logger；日志文件不可用时继续使用终端。
    pub fn get_or_init() -> &'static Self {
        if let Some(logger) = PROCESS_LOGGER.get() {
            println!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .process_log_already_installed()
            );
            return logger;
        }
        // 文件创建失败不影响运行，待 logger 安装后报告。
        let first_line = crate::config::Config::get().text().process_log_first_line();
        let mut file_warning: Option<String> = None;
        let (log_path, log_file) = match config::Config::install_dir().and_then(|dir| {
            create_timestamped_log_file(
                &dir.join(config::Config::get().constants.process_log_dir_name),
                &first_line,
            )
        }) {
            Ok((log_path, log_file)) => (Some(log_path), Some(log_file)),
            Err(message) => {
                file_warning = Some(message);
                (None, None)
            }
        };

        let levels = LogLevels::from_config();
        let logger = PROCESS_LOGGER.get_or_init(|| Self {
            levels,
            file: log_file.map(Mutex::new),
        });

        match log::set_logger(logger) {
            Ok(()) => log::set_max_level(levels.own.max(levels.dependencies)),
            Err(e) => println!(
                "{}",
                crate::bootstrap_error_text().process_log_install_failed(&e.to_string())
            ),
        }

        if let Some(warning) = file_warning {
            log::warn!("{warning}");
        }
        if let Some(log_path) = log_path {
            log::info!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .process_log_path(&log_path.display().to_string())
            );
        }
        log::info!(
            "{}",
            crate::config::Config::get()
                .text()
                .config_loaded(&config::Config::get().config_file_path.display().to_string())
        );
        log::info!(
            "{}",
            crate::config::Config::get()
                .text()
                .process_id(std::process::id())
        );

        logger
    }

    fn with_file(&self, action: impl FnOnce(&mut File)) {
        if let Some(file) = &self.file {
            match file.lock() {
                Ok(mut file) => action(&mut file),
                Err(error) => warn_output_failure("file", "lock", &error),
            }
        }
    }
}

fn warn_output_failure(destination: &str, operation: &str, error: &dyn std::fmt::Display) {
    let warning = config::Config::get().text().process_log_output_failed(
        destination,
        operation,
        &error.to_string(),
    );
    if let Err(stderr_error) = writeln!(io::stderr(), "{warning}")
        && let Err(stdout_error) = writeln!(io::stdout(), "{warning} (stderr: {stderr_error})")
    {
        panic!("cannot report process log failure: stderr: {stderr_error}; stdout: {stdout_error}");
    }
}

fn flush_output(output: &mut impl Write, destination: &str) {
    if let Err(error) = output.flush() {
        warn_output_failure(destination, "flush", &error);
    }
}

fn write_output(output: &mut impl Write, destination: &str, line: &[u8]) {
    if let Err(error) = output.write_all(line) {
        warn_output_failure(destination, "write", &error);
    }
    flush_output(output, destination);
}

impl Log for ProcessLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.levels.ceiling_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // 格式化单行日志；空消息被丢弃（`format_record` 返回 `None`），不写终端也不落文件。
        let Some(line) = format_record(
            &timestamp(),
            record.level(),
            &record.args().to_string(),
            config::Config::get()
                .constants
                .process_log_continuation_indent,
        ) else {
            return;
        };

        // 终端与文件各一份，失败不影响其他。
        write_output(&mut io::stderr(), "stderr", line.as_bytes());
        self.with_file(|file| write_output(file, "file", line.as_bytes()));
    }

    fn flush(&self) {
        self.with_file(|file| flush_output(file, "file"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingOutput;

    impl Write for FailingOutput {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("simulated write error"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("simulated flush error"))
        }
    }

    #[test]
    fn write_flush_and_poisoned_lock_failures_are_reported() {
        const CHILD: &str = "FISHBOT_PROCESS_LOG_FAILURE_CHILD";
        if std::env::var_os(CHILD).is_some() {
            config::init_for_test();
            write_output(&mut FailingOutput, "stderr", b"line");
            write_output(&mut FailingOutput, "file", b"line");

            let dir = crate::test_support::temp_test_dir("poisoned-process-log");
            std::fs::create_dir_all(&dir).unwrap();
            let file = Mutex::new(File::create(dir.join("log")).unwrap());
            std::thread::scope(|scope| {
                let thread = scope.spawn(|| {
                    let _guard = file.lock().unwrap();
                    panic!("poison test lock");
                });
                assert!(thread.join().is_err());
            });
            let logger = ProcessLog {
                levels: LogLevels::from_config(),
                file: Some(file),
            };
            logger.log(
                &Record::builder()
                    .level(log::Level::Info)
                    .target(env!("CARGO_CRATE_NAME"))
                    .args(format_args!("terminal remains available"))
                    .build(),
            );
            logger.flush();
            std::fs::remove_dir_all(dir).unwrap();
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime::process_log::tests::write_flush_and_poisoned_lock_failures_are_reported",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("run isolated logger test");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        for expected in [
            "process log stderr write failed: simulated write error",
            "process log stderr flush failed: simulated flush error",
            "process log file write failed: simulated write error",
            "process log file flush failed: simulated flush error",
            "process log file lock failed:",
            "terminal remains available",
        ] {
            assert!(
                stderr.contains(expected),
                "missing {expected:?} in {stderr}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn warning_falls_back_to_stdout_when_stderr_fails() {
        const CHILD: &str = "FISHBOT_PROCESS_LOG_STDERR_FAILURE_CHILD";
        if std::env::var_os(CHILD).is_some() {
            config::init_for_test();
            write_output(&mut io::stderr(), "stderr", b"line");
            return;
        }

        let (stderr, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(peer);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime::process_log::tests::warning_falls_back_to_stdout_when_stderr_fails",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .stderr(std::process::Stdio::from(std::os::fd::OwnedFd::from(
                stderr,
            )))
            .output()
            .expect("run isolated stderr failure test");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("process log stderr write failed"),
            "{stdout}"
        );
    }
}
