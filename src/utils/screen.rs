//! screen 会话查询与指令注入；会话存在不保证机器人进程存活或在线。

use std::collections::HashMap;
use std::io;
use std::process::{Command, Output};

use crate::config;
use crate::text;

use super::bot_name::ToCleanBotName;

/// 校验机器人名并拼接前缀，生成 screen 会话名。
pub fn session_name(raw_bot_name: &str) -> Result<String, String> {
    let clean_bot_name = raw_bot_name.to_clean_bot_name()?;

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
    let target_session_name = session_name(bot_name)?;
    let payload = format!("{bot_command}\n");
    let status = Command::new("screen")
        .args(["-S", &target_session_name, "-X", "stuff", &payload])
        .status()
        .map_err(|e| text::get().send_command_failed(bot_name, Some(&e.to_string())))?;

    if !status.success() {
        return Err(text::get().send_command_failed(bot_name, None));
    }

    Ok(())
}

/// 给一个会话注入 Ctrl+C（screen 原语：`^C` 由 screen 解释成控制字符，不需要结尾换行）。
pub fn send_stop_signal_to_session(session_name: &str) -> Result<(), String> {
    Command::new("screen")
        .args(["-X", "-S", session_name, "stuff", "^C"])
        .status()
        .map_err(|e| text::get().stop_signal_failed(&e.to_string()))?;

    Ok(())
}

/// 强杀一个会话（screen 原语：`screen -X quit`；不等待、不复核）。
pub fn force_quit_session(session_name: &str) -> Result<(), String> {
    Command::new("screen")
        .args(["-X", "-S", session_name, "quit"])
        .status()
        .map_err(|e| text::get().force_quit_failed(&e.to_string()))?;

    Ok(())
}

/// screen 会话；死残壳仍视为存在，以便停止流程清理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSession {
    /// screen 进程的 PID（`screen -list` 里会话名前的数字）。
    pub pid: u32,
    /// `screen -list` 把它标成 `(Dead ...)`。
    pub is_dead: bool,
}

/// 查询会话快照（含死残壳）；无会话时返回空映射，查询失败时返回错误。
pub fn screen_session_snapshot() -> Result<HashMap<String, ScreenSession>, String> {
    let output = Command::new("screen")
        .arg("-list")
        .env("LC_ALL", "C")
        .output();
    screen_snapshot_from_output(output)
}

fn screen_snapshot_from_output(
    output: io::Result<Output>,
) -> Result<HashMap<String, ScreenSession>, String> {
    let output = output.map_err(|error| text::get().screen_list_failed(&error.to_string()))?;
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
        return Err(text::get().screen_list_failed(&reason));
    }

    let sessions = parse_screen_list(&String::from_utf8_lossy(&output.stdout));
    if sessions.is_empty() {
        let output = String::from_utf8_lossy(&output.stdout);
        let i18n_text = text::get();
        return Err(
            i18n_text.screen_list_failed(&i18n_text.screen_unrecognized_output(output.trim()))
        );
    }
    Ok(sessions)
}

/// 从 screen 输出提取 `PID.会话名`；允许状态段缺失。
fn parse_screen_list(output: &str) -> HashMap<String, ScreenSession> {
    let mut sessions = HashMap::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(session_part) = parts.next() else {
            continue;
        };
        let Some(dot_pos) = session_part.find('.') else {
            continue;
        };
        let Ok(pid) = session_part[..dot_pos].parse::<u32>() else {
            continue;
        };

        // 死残壳状态可能包含空格。
        let is_dead = parts
            .next()
            .is_some_and(|state| state.trim_start_matches('(').starts_with("Dead"));

        sessions.insert(
            session_part[dot_pos + 1..].to_string(),
            ScreenSession { pid, is_dead },
        );
    }

    sessions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn output(code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        Ok(Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    #[test]
    fn snapshot_distinguishes_empty_sessions_from_failures() {
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
        let i18n_text = text::get();
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
