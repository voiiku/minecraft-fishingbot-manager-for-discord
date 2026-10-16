//! screen 会话的检测与指令注入，是本项目与机器人 JVM 之间的唯一通道。
//! 会话存在只代表机器人进程在运行，不代表机器人一定在线（见 [`crate::utils::running_bots`]）。

use std::collections::HashMap;
use std::process::Command;

use crate::config;
use crate::text;

use super::bot_name::ToCleanBotName;

/// 清洗原始机器人名，并只读取 `runtime.screen_prefix` 这一个字段，拼出 screen 会话名（形如 `fb_机器人名`）。
/// 名字非法（含特殊字符）时返回 `Err`：此类名字不可能有按同一口径拉起的会话。
pub fn session_name(raw_bot_name: &str) -> Result<String, String> {
    let clean_bot_name = raw_bot_name.to_clean_bot_name()?;

    Ok(format!(
        "{}{clean_bot_name}",
        config::Config::get().runtime.screen_prefix
    ))
}

/// 检测某个机器人对应的会话是否在运行。名字非法时按"没有会话"处理（返回 `false`）。
pub fn is_bot_screen_running(raw_bot_name: &str) -> bool {
    session_name(raw_bot_name).is_ok_and(|target_session_name| {
        active_screen_sessions().contains_key(&target_session_name)
    })
}

/// 把一条游戏内指令注入机器人的会话（`/command` 与 `/commandall` 共用）。
/// screen 原语：校验由调用者负责，错误文案可直接作为回复。
/// 末尾必须带换行符，screen 的伪终端才会把内容作为一次提交。
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

/// 一条 `screen -list` 里的会话。
///
/// 死残壳（`(Dead ...)`）指 screen 进程仍在、其内的 JVM 已退出后留下的会话。
/// 它按运行中处理：[`is_bot_screen_running`] 与停止流程都视其为运行中，`/stop` 才会走到
/// `screen -X quit` 把残壳清掉。
/// 但状态判定必须认出它：JVM 已结束，不应读取日志，而是直接认定该机器人已停止，见 [`crate::utils::running_bots`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSession {
    /// screen 进程的 PID（`screen -list` 里会话名前的数字）。
    pub pid: u32,
    /// `screen -list` 把它标成 `(Dead ...)`。
    pub is_dead: bool,
}

/// 获取当前所有存活的 screen 会话，返回会话名到 [`ScreenSession`] 的映射。
/// `screen -list` 的每条会话行形如 `12345.fb_gamename (Detached)`：第一个点之前的数字是 PID，
/// 之后到空白为止是会话名（完整匹配，调用方按名字查即可），再后面括号里是状态。
/// 解析细节（认哪些行、怎么认死残壳）在 [`parse_screen_list`]。
pub fn active_screen_sessions() -> HashMap<String, ScreenSession> {
    let Ok(output) = Command::new("screen").arg("-list").output() else {
        return HashMap::new();
    };

    parse_screen_list(&String::from_utf8_lossy(&output.stdout))
}

/// [`active_screen_sessions`] 的解析部分。
/// 只认「PID.会话名」后跟状态的会话行；头部说明行、尾部 `N Sockets in ...` 统计行与空行都跳过，
/// 因此 `-list` 输出被截断时留下的半行也被跳过。状态段取括号内那段：
/// 以 `Dead` 开头者视为死残壳，其余（`(Attached)`、`(Detached)`）视为存活。
fn parse_screen_list(output: &str) -> HashMap<String, ScreenSession> {
    let mut sessions = HashMap::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 不是「PID.会话名 ...」形状的（说明行、统计行）跳过。
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

        // 状态段内可能含空格（`(Dead ???)`），因此取"第一个空白分隔的片段"即可：
        // 它要么是 `(Detached)`，要么是 `(Dead`。
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
