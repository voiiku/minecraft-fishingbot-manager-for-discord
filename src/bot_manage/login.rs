//! `/bot login`：等待 JAR 输出设备码并写入登录凭据。
//! 第一段返回设备码，第二段等待授权结果。

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::text;
use crate::utils::{
    BotDirEntry, StartupQueue, is_bot_screen_running, read_last_lines, stop_single_bot_session,
    token_file_path,
};

/// JAR 设备码提示关键字。
const DEVICE_CODE_MARKER: &str = "using the following code";

/// JAR 登录拒绝标记。
const AUTH_FAILED_MARKER: &str = "Error while creating refresh token";

/// 等待设备码的最长时间（秒）。
const CODE_WAIT_SECS: u64 = 60;

/// 等待授权的最长时间（秒），需与登录提示中的有效期一致。
const AUTHORIZE_WAIT_SECS: u64 = 600;

/// 轮询间隔（毫秒）。
const POLL_INTERVAL_MS: u64 = 500;

/// 每次轮询读取的日志行数。
const LOG_TAIL_LINES: usize = 200;

/// 旧凭据备份文件后缀。
const REPLACED_TOKEN_SUFFIX: &str = "renewing";

/// 登录第一段返回的提示与等待句柄。
#[derive(Debug)]
pub struct BotLoginPrompt {
    /// 登录链接与设备码。
    pub message: String,
    /// 第二段使用的会话信息。
    session: LoginSession,
}

/// 登录会话状态。
#[derive(Debug)]
struct LoginSession {
    /// 机器人目录条目。
    bot_dir_entry: BotDirEntry,
    /// 本次登录的日志文件。
    log_path: PathBuf,
    /// 旧凭据备份路径。
    replaced_token_path: Option<PathBuf>,
}

impl LoginSession {
    /// 机器人名。
    fn bot_name(&self) -> &str {
        &self.bot_dir_entry.bot_name
    }

    /// 本次运行的新凭据路径。
    fn token_path(&self) -> PathBuf {
        token_file_path(&self.bot_dir_entry.bot_dir_path)
    }
}

/// 启动登录会话并等待设备码；权限由路由层检查。
pub(crate) fn begin_login(bot_dir_entry: &BotDirEntry) -> Result<BotLoginPrompt, String> {
    let bot_name = &bot_dir_entry.bot_name;

    // 避免打断登录中的会话或运行中的机器人。
    if is_bot_screen_running(bot_name)? {
        let refusal = if bot_dir_entry.bot_token_status.is_ready() {
            text::get().refusal_bot_running(bot_name)
        } else {
            text::get().bot_login_already_running(bot_name)
        };

        return Err(refusal);
    }

    // 已在启动队列中排队的：等待其启动或先取消，避免与队列拉起的会话重名。
    if StartupQueue::get_or_init().bot_in_queue(bot_dir_entry) {
        return Err(text::get().bot_start_already_queued(bot_name));
    }

    let replaced_token_path = move_existing_token_aside(bot_dir_entry)?;

    // 拉起会话（与 `/start` 的逐台拉起同一份实现，只是不按凭据状态拒绝）。
    let log_path = match StartupQueue::add_bot_for_login(bot_name, &bot_dir_entry.bot_dir_path) {
        Ok(log_path) => log_path,
        Err(message) => {
            // 会话未起来：恢复旧凭据，避免一次失败的登录使可用凭据丢失。
            restore_replaced_token(replaced_token_path);
            return Err(message);
        }
    };

    let session = LoginSession {
        bot_dir_entry: bot_dir_entry.clone(),
        log_path,
        replaced_token_path,
    };

    let prompt = match wait_for_device_code(&session) {
        Ok(prompt) => prompt,
        Err(error) => {
            if stop_login_session(&session) {
                restore_replaced_token(session.replaced_token_path);
            }
            return Err(error);
        }
    };
    let Some((verification_url, user_code)) = prompt else {
        // 等不到提示：JAR 可能没起来、参数不对，或这份构建没有设备码登录。
        // 会话已经拉起来了，这里停掉它再把结论回给用户。
        let message = match is_bot_screen_running(session.bot_name()) {
            Ok(true) => text::get().bot_login_prompt_missing(session.bot_name()),
            Ok(false) => text::get().start_failed(session.bot_name()),
            Err(error) => error,
        };
        if stop_login_session(&session) {
            restore_replaced_token(session.replaced_token_path);
        }
        return Err(message);
    };

    Ok(BotLoginPrompt {
        message: text::get().bot_login_prompt(session.bot_name(), &verification_url, &user_code),
        session,
    })
}

/// 等待凭据落盘或登录失败；成功时保留会话。
pub(crate) fn await_login(prompt: BotLoginPrompt) -> String {
    let session = prompt.session;
    let bot_name = session.bot_name().to_string();
    let token_path = session.token_path();

    let deadline = Instant::now() + Duration::from_secs(AUTHORIZE_WAIT_SECS);

    while Instant::now() < deadline {
        if token_path.exists() {
            return finish_login(&session);
        }

        let log_tail = read_log_tail(&session.log_path);

        if log_tail
            .iter()
            .any(|line| line.contains(AUTH_FAILED_MARKER))
        {
            // 停止会话并恢复旧凭据。
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::BotLoginRejected {
                    bot_name: &bot_name
                })
            );
            if stop_login_session(&session) {
                restore_replaced_token(session.replaced_token_path);
            }
            return text::get().bot_login_rejected(&bot_name);
        }

        match is_bot_screen_running(&bot_name) {
            Ok(true) => {}
            Ok(false) => {
                restore_replaced_token(session.replaced_token_path);
                return text::get().start_failed(&bot_name);
            }
            Err(error) => {
                if stop_login_session(&session) {
                    restore_replaced_token(session.replaced_token_path);
                }
                return error;
            }
        }

        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    // 超时后再次检查凭据，避免漏掉刚完成的授权。
    if token_path.exists() {
        return finish_login(&session);
    }

    let stopped = stop_login_session(&session);

    // 停止会话后再次检查凭据。
    if token_path.exists() {
        return finish_login(&session);
    }

    if stopped {
        restore_replaced_token(session.replaced_token_path);
    }
    text::get().bot_login_timed_out(&bot_name, AUTHORIZE_WAIT_SECS / 60)
}

/// 登录成功后删除旧备份并收紧新凭据权限。
fn finish_login(session: &LoginSession) -> String {
    let bot_name = session.bot_name();

    // 删除旧凭据备份。
    if let Some(replaced_token_path) = &session.replaced_token_path {
        remove_replaced_token(replaced_token_path);
    }

    tighten_token_permissions(&session.token_path());

    text::get().bot_login_succeeded(bot_name)
}

/// 将凭据权限设为 `0600`；失败时只记警告。
fn tighten_token_permissions(token_path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Err(e) = fs::set_permissions(token_path, fs::Permissions::from_mode(0o600)) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::BotLoginChmodFailed {
                path: &token_path.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }
}

/// 将已有凭据移至备份路径，并清理上次中断留下的备份。
fn move_existing_token_aside(bot_dir_entry: &BotDirEntry) -> Result<Option<PathBuf>, String> {
    let token_path = token_file_path(&bot_dir_entry.bot_dir_path);
    let replaced_token_path = replaced_token_path(&token_path);

    match leftover_action(replaced_token_path.exists(), token_path.exists()) {
        LeftoverAction::Nothing => {}
        LeftoverAction::RemoveLeftover => remove_replaced_token(&replaced_token_path),
        LeftoverAction::RestoreLeftover => {
            restore_token_in_place(&replaced_token_path, &token_path)
        }
    }

    if !token_path.exists() {
        return Ok(None);
    }

    fs::rename(&token_path, &replaced_token_path)
        .map_err(|e| text::get().bot_token_write_failed(&bot_dir_entry.bot_name, &e.to_string()))?;

    Ok(Some(replaced_token_path))
}

/// 上次登录留下的备份处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftoverAction {
    /// 无需处理。
    Nothing,
    /// 新旧凭据都在，删除备份。
    RemoveLeftover,
    /// 只有备份，将其恢复。
    RestoreLeftover,
}

/// 根据凭据文件是否存在决定备份处理方式。
fn leftover_action(replaced_token_exists: bool, token_exists: bool) -> LeftoverAction {
    match (replaced_token_exists, token_exists) {
        (false, _) => LeftoverAction::Nothing,
        (true, true) => LeftoverAction::RemoveLeftover,
        (true, false) => LeftoverAction::RestoreLeftover,
    }
}

/// 登录失败或超时时恢复旧凭据。
fn restore_replaced_token(replaced_token_path: Option<PathBuf>) {
    let Some(replaced_token_path) = replaced_token_path else {
        return;
    };

    let Some(token_path) = restored_token_path(&replaced_token_path) else {
        return;
    };

    // 新凭据已存在时删除旧备份。
    if token_path.exists() {
        remove_replaced_token(&replaced_token_path);
        return;
    }

    restore_token_in_place(&replaced_token_path, &token_path);
}

/// 删除旧凭据备份；失败时只记日志。
fn remove_replaced_token(replaced_token_path: &Path) {
    if let Err(e) = fs::remove_file(replaced_token_path) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::BotLoginOldTokenRemoveFailed {
                path: &replaced_token_path.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }
}

/// 恢复旧凭据；失败时只记日志。
fn restore_token_in_place(replaced_token_path: &Path, token_path: &Path) {
    if let Err(e) = fs::rename(replaced_token_path, token_path) {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::BotLoginOldTokenRestoreFailed {
                path: &replaced_token_path.display().to_string(),
                reason: &e.to_string(),
            })
        );
    }
}

/// 生成旧凭据备份路径。
fn replaced_token_path(token_path: &Path) -> PathBuf {
    let mut file_name = token_path.file_name().unwrap_or_default().to_os_string();
    file_name.push(format!(".{REPLACED_TOKEN_SUFFIX}"));

    token_path.with_file_name(file_name)
}

/// 从备份路径还原凭据路径。
fn restored_token_path(replaced_token_path: &Path) -> Option<PathBuf> {
    let file_name = replaced_token_path.file_name()?.to_str()?;
    let original_file_name = file_name.strip_suffix(&format!(".{REPLACED_TOKEN_SUFFIX}"))?;

    Some(replaced_token_path.with_file_name(original_file_name))
}

/// 停止登录会话，给 JAR 留出写入凭据的机会。
fn stop_login_session(session: &LoginSession) -> bool {
    let result = stop_single_bot_session(session.bot_dir_entry.clone()).and_then(|outcome| {
        if outcome.has_failed_lines() {
            Err(outcome.to_failure_text())
        } else {
            Ok(())
        }
    });
    if let Err(reason) = result {
        log::warn!(
            "{}",
            text::get().log_text(text::LogText::BotLoginStopFailed {
                bot_name: session.bot_name(),
                reason: &reason,
            })
        );
        return false;
    }
    true
}

/// 等待 JAR 输出设备码，返回验证网址与用户代码。
fn wait_for_device_code(session: &LoginSession) -> Result<Option<(String, String)>, String> {
    let deadline = Instant::now() + Duration::from_secs(CODE_WAIT_SECS);

    while Instant::now() < deadline {
        if let Some(prompt) = parse_device_code_prompt(&read_log_tail(&session.log_path)) {
            return Ok(Some(prompt));
        }

        if !is_bot_screen_running(session.bot_name())? {
            return Ok(parse_device_code_prompt(&read_log_tail(&session.log_path)));
        }

        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    Ok(parse_device_code_prompt(&read_log_tail(&session.log_path)))
}

/// 从日志里解析出设备码提示：返回 `(验证网址, 用户代码)`。
///
/// 真机上的那一行长这样（前面 `[时间] [WARNING] ` 那段是 JAR 自己的日志格式）：
/// `Please log in with your Microsoft account using the following code AQHV33R4 at https://www.microsoft.com/link.`
/// 网址末尾带句点，需去掉；代码按原文取（不校验形状，避免上游改变格式后误判）。
fn parse_device_code_prompt(lines: &[String]) -> Option<(String, String)> {
    for line in lines {
        let Some(after_marker) = line.split(DEVICE_CODE_MARKER).nth(1) else {
            continue;
        };

        let mut parts = after_marker.split(" at ");
        let Some(user_code) = parts.next().map(str::trim) else {
            continue;
        };
        let Some(verification_url) = parts.next().map(str::trim) else {
            continue;
        };

        let verification_url = verification_url.trim_end_matches('.').trim();
        if user_code.is_empty() || verification_url.is_empty() {
            continue;
        }

        return Some((verification_url.to_string(), user_code.to_string()));
    }

    None
}

/// 读取日志尾部；失败时返回空列表并继续轮询。
fn read_log_tail(log_path: &Path) -> Vec<String> {
    read_last_lines(log_path, LOG_TAIL_LINES).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析设备码、网址并去除网址末尾的句点。
    #[test]
    fn device_code_prompt_is_parsed() {
        let lines = vec![
            "[2026-10-07 02:04:46] [INFO   ] Trying to authenticate at Microsoft...".to_string(),
            "[2026-10-07 02:04:48] [WARNING] Please log in with your Microsoft account using the \
             following code AQHV33R4 at https://www.microsoft.com/link."
                .to_string(),
        ];

        assert_eq!(
            parse_device_code_prompt(&lines),
            Some((
                "https://www.microsoft.com/link".to_string(),
                "AQHV33R4".to_string()
            ))
        );
    }

    /// 无关键字或缺少代码/网址时返回 `None`。
    #[test]
    fn unrecognized_prompt_lines_are_skipped() {
        let cases = vec![
            vec!["[INFO   ] Trying to authenticate at Microsoft...".to_string()],
            vec!["using the following code AQHV33R4".to_string()],
            vec!["using the following code  at https://www.microsoft.com/link.".to_string()],
            vec!["using the following code AQHV33R4 at ".to_string()],
            Vec::new(),
        ];

        for lines in cases {
            assert_eq!(parse_device_code_prompt(&lines), None, "{lines:?}");
        }
    }

    /// 备份路径可还原为原凭据路径。
    #[test]
    fn replaced_token_path_round_trips() {
        let token_path = Path::new("/srv/bots/42/duck/refreshToken");
        let replaced = replaced_token_path(token_path);

        assert_eq!(
            replaced,
            Path::new("/srv/bots/42/duck/refreshToken.renewing")
        );
        assert_eq!(
            restored_token_path(&replaced),
            Some(token_path.to_path_buf())
        );
    }

    /// 标记应与上游消息清单一致。
    #[test]
    fn markers_match_the_documented_upstream_messages() {
        let documented = include_str!("../../docs/refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md");

        for marker in [DEVICE_CODE_MARKER, AUTH_FAILED_MARKER] {
            assert!(documented.contains(marker), "清单里找不到: {marker}");
        }
    }

    /// 根据新旧凭据是否存在决定删除或恢复备份。
    #[test]
    fn leftover_decision_covers_every_case() {
        assert_eq!(leftover_action(false, false), LeftoverAction::Nothing);
        assert_eq!(leftover_action(false, true), LeftoverAction::Nothing);
        assert_eq!(leftover_action(true, true), LeftoverAction::RemoveLeftover);
        assert_eq!(
            leftover_action(true, false),
            LeftoverAction::RestoreLeftover
        );
    }
}
