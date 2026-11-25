//! GNU screen 会话快照、指令注入与停止信号。
use crate::runtime::utils::screen as screen_utils;
use crate::utils::bot_name::ToCleanBotName;
use crate::{SCREEN_COMMAND_LOCKS, config};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::process::Output;
use std::sync::Mutex;

pub(crate) use crate::runtime::utils::screen::ScreenSession;
use crate::runtime::utils::screen::parse_screen_list;

/// 校验机器人名并拼接前缀，生成 screen 会话名。
pub(crate) fn session_name(raw_bot_name: &str) -> Result<String, String> {
    let clean_bot_name = raw_bot_name
        .to_clean_bot_name()
        .map_err(crate::runtime::bot_instances::bot_dirs::bot_name_error)?;

    Ok(format!(
        "{}{clean_bot_name}",
        config::Config::get().runtime.screen_prefix
    ))
}

/// 检测 screen 会话是否存在（包括死残壳）；查询失败返回错误。
pub fn is_bot_screen_running(raw_bot_name: &str) -> Result<bool, String> {
    let target_session_name = session_name(raw_bot_name)?;
    Ok(screen_session_snapshot()?.contains_key(&target_session_name))
}

/// 将游戏内指令注入 screen 会话；指令校验由调用方负责，末尾换行用于提交。
pub fn send_command_to_single_bot(bot_name: &str, bot_command: &str) -> Result<(), String> {
    with_bot_command_lock(bot_name, || {
        send_command_to_single_bot_unlocked(bot_name, bot_command)
    })
}

/// 同一机器人的注入与状态查询不可交错。
pub(crate) fn with_bot_command_lock<T>(bot_name: &str, action: impl FnOnce() -> T) -> T {
    let count = config::Config::get()
        .constants
        .screen_command_lock_count
        .get();
    let locks = SCREEN_COMMAND_LOCKS.get_or_init(|| (0..count).map(|_| Mutex::new(())).collect());
    let mut hasher = DefaultHasher::new();
    bot_name.hash(&mut hasher);
    let _guard = locks[hasher.finish() as usize % locks.len()]
        .lock()
        .expect("机器人指令锁中毒");
    action()
}

pub(crate) fn send_command_to_single_bot_unlocked(
    bot_name: &str,
    bot_command: &str,
) -> Result<(), String> {
    let target_session_name = session_name(bot_name)?;
    let payload = format!("{bot_command}\n");
    let status = screen_utils::send_command(&target_session_name, &payload).map_err(|e| {
        crate::bootstrap_error_text().send_command_failed(bot_name, Some(&e.to_string()))
    })?;

    if !status.success() {
        return Err(crate::bootstrap_error_text().send_command_failed(bot_name, None));
    }

    Ok(())
}

/// 给一个会话注入 Ctrl+C（screen 原语：`^C` 由 screen 解释成控制字符，不需要结尾换行）。
pub(crate) fn send_stop_signal_to_session(session_name: &str) -> Result<(), String> {
    screen_utils::send_stop_signal(session_name)
        .map_err(|e| crate::bootstrap_error_text().stop_signal_failed(&e.to_string()))?;

    Ok(())
}

/// 强杀一个会话（screen 原语：`screen -X quit`；不等待、不复核）。
pub(crate) fn force_quit_session(session_name: &str) -> Result<(), String> {
    screen_utils::force_quit(session_name)
        .map_err(|e| crate::bootstrap_error_text().force_quit_failed(&e.to_string()))?;

    Ok(())
}

/// 查询会话快照（含死残壳）；无会话时返回空映射，查询失败时返回错误。
pub(crate) fn screen_session_snapshot() -> Result<HashMap<String, ScreenSession>, String> {
    screen_snapshot_from_output(screen_utils::list_sessions())
}

fn screen_snapshot_from_output(
    output: io::Result<Output>,
) -> Result<HashMap<String, ScreenSession>, String> {
    let output = output
        .map_err(|error| crate::bootstrap_error_text().screen_list_failed(&error.to_string()))?;
    if output.status.code() == Some(1)
        && output.stderr.is_empty()
        && output.stdout.starts_with(b"No Sockets found in ")
    {
        return Ok(HashMap::new());
    }

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = format!(
            "{}; stdout: {}; stderr: {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
        return Err(crate::bootstrap_error_text().screen_list_failed(&reason));
    }

    let sessions = parse_screen_list(&String::from_utf8_lossy(&output.stdout));
    if sessions.is_empty() {
        let output = String::from_utf8_lossy(&output.stdout);
        let i18n_text = crate::bootstrap_error_text();
        return Err(
            i18n_text.screen_list_failed(&i18n_text.screen_unrecognized_output(output.trim()))
        );
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn command_locks_serialize_the_same_bot() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        config::init_for_test();
        let active = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..20 {
                        with_bot_command_lock("duck", || {
                            assert_eq!(active.fetch_add(1, Ordering::SeqCst), 0);
                            std::thread::yield_now();
                            assert_eq!(active.fetch_sub(1, Ordering::SeqCst), 1);
                        });
                    }
                });
            }
        });
    }

    fn output(code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        Ok(Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    #[test]
    fn snapshot_distinguishes_empty_sessions_from_failures() {
        crate::config::init_for_test();
        assert!(
            screen_snapshot_from_output(output(1, "No Sockets found in /tmp/screen.\n", ""))
                .expect("GNU screen reports no sessions")
                .is_empty()
        );
        let sessions = screen_snapshot_from_output(output(
            0,
            "There is a screen on:\n\t123.fb_duck\t(Detached)\n",
            "",
        ))
        .expect("valid screen output");
        assert!(sessions.contains_key("fb_duck"));

        for result in [
            Err(io::Error::new(io::ErrorKind::NotFound, "screen missing")),
            output(1, "", "permission denied"),
            output(1, "No Sockets found in /tmp/screen.\n", "failure"),
            output(2, "", "screen failed"),
            output(0, "", ""),
        ] {
            assert!(screen_snapshot_from_output(result).is_err());
        }
        let error = screen_snapshot_from_output(output(0, "unexpected text", "")).unwrap_err();
        let i18n_text = crate::config::Config::get().text();
        assert_eq!(
            error,
            i18n_text.screen_list_failed(&i18n_text.screen_unrecognized_output("unexpected text"))
        );
    }

    /// 真实的 `screen -list` 输出（头部说明行、三条会话、尾部统计行）：
    /// 会话名取第一个点之后的部分，PID 取点之前的部分，`(Dead ???)` 需识别为死残壳。
    #[test]
    fn screen_list_output_is_parsed() {
        let output = concat!(
            "There are screens on:\n",
            "\t12345.fb_duck\t(Detached)\n",
            "\t23456.fb_goose\t(Attached)\n",
            "\t34567.fb_crane\t(Dead ???)\n",
            "3 Sockets in /run/screen/S-fish.\n",
        );

        let sessions = parse_screen_list(output);

        assert_eq!(sessions.len(), 3);
        assert_eq!(
            sessions["fb_duck"],
            ScreenSession {
                pid: 12345,
                is_dead: false
            }
        );
        assert_eq!(
            sessions["fb_goose"],
            ScreenSession {
                pid: 23456,
                is_dead: false
            }
        );
        assert_eq!(
            sessions["fb_crane"],
            ScreenSession {
                pid: 34567,
                is_dead: true
            }
        );
    }

    /// 认不出的行一律跳过：说明行、统计行、空行、没有点的行，以及被截断、没有状态段的半行
    ///（后者按存活处理：老版本 screen 不打印状态段）。
    #[test]
    fn unrecognized_screen_list_lines_are_skipped() {
        let output = concat!(
            "There is a screen on:\n",
            "\n",
            "没有点的会话段\n",
            "\t67890.fb_duck\n",
            "1 Socket in /run/screen/S-fish.\n",
        );

        let sessions = parse_screen_list(output);

        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions["fb_duck"],
            ScreenSession {
                pid: 67890,
                is_dead: false
            }
        );
    }
}
