//! 通过 FishingBot 的 `/status` 查询完整状态。
use super::bot_dirs::BotDirEntry;
use super::screen::{
    is_bot_screen_running, send_command_to_single_bot_unlocked, with_bot_command_lock,
};
use crate::runtime::bot_log::latest_log_path;
use crate::runtime::log_files::read_last_lines_after;
use crate::runtime::utils::bot_status::{jar_log_message, parse_bot_status as parse_status};
use crate::{config, text};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub use crate::runtime::utils::bot_status::BotStatus;

/// 解析完整状态；不可用或格式错误时返回 `None`。
fn parse_bot_status(message: &str, language: &dyn text::Text) -> Option<BotStatus> {
    parse_status(message, language.jar_status_prefix())
}

/// 只读取请求发送前记录的偏移之后的新回显。
fn wait_for_bot_status(
    bot_name: &str,
    log_path: &Path,
    previous: &fs::Metadata,
    timeout: Duration,
) -> Result<Option<BotStatus>, String> {
    let started = Instant::now();
    let mut last_scan_len = previous.len();
    let i18n = crate::config::Config::get().text();
    loop {
        let current = fs::metadata(log_path).map_err(|_| {
            crate::config::Config::get()
                .text()
                .bot_status_unreadable_log(bot_name)
        })?;
        if current.dev() != previous.dev()
            || current.ino() != previous.ino()
            || current.len() < last_scan_len
        {
            return Err(crate::config::Config::get()
                .text()
                .bot_status_log_changed(bot_name));
        }
        if current.len() > last_scan_len {
            // 始终保留请求偏移，分两次写入的同一行仍能完整解析。
            last_scan_len = current.len();
            let lines = read_last_lines_after(
                log_path,
                previous.len(),
                config::Config::get().constants.status_scan_lines,
            )
            .map_err(|message| i18n.bot_line(bot_name, &message))?;
            for message in lines.iter().rev().filter_map(|line| jar_log_message(line)) {
                if message == i18n.jar_status_unavailable() {
                    return Ok(None);
                }
                if let Some(status) = parse_bot_status(message, i18n) {
                    return Ok(Some(status));
                }
            }
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(i18n.bot_status_query_timeout(bot_name));
        }
        thread::sleep(remaining.min(config::Config::get().constants.status_poll_interval));
    }
}

/// 可选地发送暂停指令，再查询 FishingBot 状态；不缓存状态。
pub(crate) fn query_bot_status(
    bot: &BotDirEntry,
    command: Option<&str>,
) -> Result<Option<BotStatus>, String> {
    let name = &bot.bot_name;
    with_bot_command_lock(name, || {
        if command.is_some()
            && !is_bot_screen_running(name)
                .map_err(|error| crate::config::Config::get().text().bot_line(name, &error))?
        {
            return Err(crate::config::Config::get()
                .text()
                .bot_command_not_running(name));
        }
        let log_path = latest_log_path(&bot.bot_dir_path).ok_or_else(|| {
            crate::config::Config::get()
                .text()
                .bot_status_log_not_found(name)
        })?;
        if let Some(command) = command {
            fs::metadata(&log_path).map_err(|_| {
                crate::config::Config::get()
                    .text()
                    .bot_status_unreadable_log(name)
            })?;
            send_command_to_single_bot_unlocked(name, command)?;
        }
        let result = (|| {
            let previous = fs::metadata(&log_path).map_err(|_| {
                crate::config::Config::get()
                    .text()
                    .bot_status_unreadable_log(name)
            })?;
            send_command_to_single_bot_unlocked(name, "/status")?;
            wait_for_bot_status(
                name,
                &log_path,
                &previous,
                config::Config::get().constants.status_timeout,
            )
        })();
        result.map_err(|error| match command {
            Some(command) => crate::config::Config::get()
                .text()
                .bot_status_query_after_command_failed(command, &error),
            None => error,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_every_status_field_in_both_languages() {
        for language in text::all_languages() {
            let prefix = language.jar_status_prefix();
            let message = format!(
                "{prefix}paused=true joined=false no_rod_available=true has_bobber=false last_fish=1793615750570"
            );
            assert_eq!(
                parse_bot_status(&message, *language),
                Some(BotStatus {
                    paused: true,
                    joined: false,
                    no_rod_available: true,
                    has_bobber: false,
                    last_fish: 1793615750570,
                })
            );
            let negative_timestamp = message.replace("last_fish=1793615750570", "last_fish=-1");
            assert_eq!(
                parse_bot_status(&negative_timestamp, *language)
                    .unwrap()
                    .last_fish,
                -1
            );
            assert_eq!(
                parse_bot_status(language.jar_status_unavailable(), *language),
                None
            );
            for invalid in [
                message.replace("paused=true", "paused=maybe"),
                message.replace("joined=false", "joined=unknown"),
                message.replace("no_rod_available=true", "no_rod_available=1"),
                message.replace("has_bobber=false", "has_bobber=invalid"),
                message.replace("last_fish=1793615750570", "last_fish=oops"),
                message.replace("last_fish=1793615750570", "last_fish=9223372036854775808"),
                format!("{message} extra=true"),
            ] {
                assert_eq!(parse_bot_status(&invalid, *language), None, "{invalid}");
            }
            assert_eq!(
                jar_log_message("[2026] [INFO   ] [CHAT] Fishing status: unavailable"),
                Some("[CHAT] Fishing status: unavailable")
            );
            assert_eq!(jar_log_message("[2026] [WARN] [CHAT] [INFO   ] fake"), None);
        }
    }

    #[test]
    fn distinguishes_unavailable_from_timeout_and_rejects_replaced_logs() {
        config::init_for_test();
        let dir = crate::test_support::temp_test_dir("status-reply");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bot.log");
        let i18n = crate::config::Config::get().text();
        let old = format!(
            "[2026] [INFO   ] {}paused=true joined=true no_rod_available=false has_bobber=false last_fish=0\n",
            i18n.jar_status_prefix()
        );
        fs::write(&path, &old).unwrap();
        let baseline = fs::metadata(&path).unwrap();
        assert_eq!(
            wait_for_bot_status("duck", &path, &baseline, Duration::ZERO).unwrap_err(),
            i18n.bot_status_query_timeout("duck")
        );
        let unavailable = format!("[2026] [INFO   ] {}\n", i18n.jar_status_unavailable());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(unavailable.as_bytes())
            .unwrap();
        assert_eq!(
            wait_for_bot_status("duck", &path, &baseline, Duration::ZERO).unwrap(),
            None
        );
        fs::write(&path, "").unwrap();
        assert!(wait_for_bot_status("duck", &path, &baseline, Duration::ZERO).is_err());
        fs::rename(&path, dir.join("old.log")).unwrap();
        fs::write(&path, old.repeat(2)).unwrap();
        assert!(wait_for_bot_status("duck", &path, &baseline, Duration::ZERO).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn waits_for_the_rest_of_a_partially_written_response() {
        config::init_for_test();
        let dir = crate::test_support::temp_test_dir("status-delayed");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bot.log");
        fs::write(&path, "old log\n").unwrap();
        let baseline = fs::metadata(&path).unwrap();
        let mut writer = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(
            writer,
            "[2026] [INFO   ] {}paused=false joined=false ",
            crate::config::Config::get().text().jar_status_prefix()
        )
        .unwrap();
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(20));
                writeln!(writer, "no_rod_available=true has_bobber=false last_fish=0").unwrap();
            });
            assert_eq!(
                wait_for_bot_status("duck", &path, &baseline, Duration::from_secs(1)).unwrap(),
                Some(BotStatus {
                    paused: false,
                    joined: false,
                    no_rod_available: true,
                    has_bobber: false,
                    last_fish: 0,
                })
            );
        });
        fs::remove_dir_all(&dir).unwrap();
    }
}
