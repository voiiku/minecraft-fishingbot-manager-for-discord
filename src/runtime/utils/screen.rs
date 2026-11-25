//! GNU screen 的进程调用与会话列表解析。

use std::collections::HashMap;
use std::io;
use std::process::{Command, ExitStatus, Output};

/// screen 会话；死残壳仍视为存在，以便停止流程清理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenSession {
    pub pid: u32,
    pub is_dead: bool,
}

pub(crate) fn list_sessions() -> io::Result<Output> {
    Command::new("screen")
        .arg("-list")
        .env("LC_ALL", "C")
        .output()
}

pub(crate) fn send_command(session_name: &str, payload: &str) -> io::Result<ExitStatus> {
    Command::new("screen")
        .args(["-S", session_name, "-X", "stuff", payload])
        .status()
}

pub(crate) fn send_stop_signal(session_name: &str) -> io::Result<ExitStatus> {
    Command::new("screen")
        .args(["-X", "-S", session_name, "stuff", "^C"])
        .status()
}

pub(crate) fn force_quit(session_name: &str) -> io::Result<ExitStatus> {
    Command::new("screen")
        .args(["-X", "-S", session_name, "quit"])
        .status()
}

pub(crate) fn parse_screen_list(output: &str) -> HashMap<String, ScreenSession> {
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
