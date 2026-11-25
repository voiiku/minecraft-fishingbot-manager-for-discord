//! `/bot login`：等待 JAR 输出设备码并写入登录凭据。
//! 第一段返回设备码，第二段等待授权结果。

use crate::operations::permitted_bot_dir_entry;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::config;
use crate::runtime::bot_instances::bot_dirs::{BotDirEntry, token_file_path};
use crate::runtime::bot_instances::screen::is_bot_screen_running;
use crate::runtime::bot_instances::stop::stop_single_bot_session;
use crate::runtime::bot_startup::{StartupQueue, create_logs_and_launch_bot_session};
use crate::runtime::log_files::read_last_lines;

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

/// 检查权限，启动登录会话并等待设备码。
pub fn begin_bot_login(name: &str, caller_id: u64) -> Result<BotLoginPrompt, String> {
    let bot_dir_entry = permitted_bot_dir_entry(name, caller_id)?;
    let bot_name = &bot_dir_entry.bot_name;

    if is_bot_screen_running(bot_name)? {
        let refusal = if bot_dir_entry.bot_token_status.is_ready() {
            crate::config::Config::get()
                .text()
                .refusal_bot_running(bot_name)
        } else {
            crate::config::Config::get()
                .text()
                .bot_login_already_running(bot_name)
        };

        return Err(refusal);
    }

    // 排队中的启动请求可能创建同名会话。
    if StartupQueue::get_or_init().bot_in_queue(&bot_dir_entry) {
        return Err(crate::config::Config::get()
            .text()
            .bot_start_already_queued(bot_name));
    }

    let replaced_token_path =
        move_existing_token_aside(bot_name, &token_file_path(&bot_dir_entry.bot_dir_path))?;

    // 与 `/start` 共用会话启动逻辑，但不要求已有凭据。
    let log_path = match create_logs_and_launch_bot_session(bot_name, &bot_dir_entry.bot_dir_path) {
        Ok(log_path) => log_path,
        Err(message) => {
            // 会话启动失败时恢复旧凭据。
            restore_replaced_token(replaced_token_path);
            return Err(message);
        }
    };

    let session = LoginSession {
        bot_dir_entry,
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
        // 未收到设备码时，区分会话启动失败和设备码缺失。
        let message = match is_bot_screen_running(session.bot_name()) {
            Ok(true) => crate::config::Config::get()
                .text()
                .bot_login_prompt_missing(session.bot_name()),
            Ok(false) => crate::config::Config::get()
                .text()
                .start_failed(session.bot_name()),
            Err(error) => error,
        };
        if stop_login_session(&session) {
            restore_replaced_token(session.replaced_token_path);
        }
        return Err(message);
    };

    Ok(BotLoginPrompt {
        message: crate::config::Config::get().text().bot_login_prompt(
            session.bot_name(),
            &verification_url,
            &user_code,
            config::Config::get()
                .constants
                .login_authorization_timeout_minutes,
        ),
        session,
    })
}

/// 等待凭据落盘或登录失败；成功时保留会话。
pub fn finish_bot_login(prompt: BotLoginPrompt) -> String {
    let session = prompt.session;
    let bot_name = session.bot_name().to_string();
    let token_path = session.token_path();

    let constants = &config::Config::get().constants;
    let deadline = Instant::now()
        + Duration::from_secs(
            constants
                .login_authorization_timeout_minutes
                .saturating_mul(60),
        );

    while Instant::now() < deadline {
        if token_path.exists() {
            return finish_login(&session);
        }

        let log_tail = read_log_tail(&session.log_path);

        if log_tail
            .iter()
            .any(|line| line.contains(constants.login_auth_failure_marker))
        {
            // 停止会话并恢复旧凭据。
            log::warn!(
                "{}",
                crate::config::Config::get()
                    .text()
                    .bot_login_rejected_log_message(&bot_name)
            );
            if stop_login_session(&session) {
                restore_replaced_token(session.replaced_token_path);
            }
            return crate::config::Config::get()
                .text()
                .bot_login_rejected(&bot_name);
        }

        match is_bot_screen_running(&bot_name) {
            Ok(true) => {}
            Ok(false) => {
                restore_replaced_token(session.replaced_token_path);
                return crate::config::Config::get().text().start_failed(&bot_name);
            }
            Err(error) => {
                if stop_login_session(&session) {
                    restore_replaced_token(session.replaced_token_path);
                }
                return error;
            }
        }

        thread::sleep(constants.login_poll_interval);
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
    crate::config::Config::get()
        .text()
        .bot_login_timed_out(&bot_name, constants.login_authorization_timeout_minutes)
}

/// 登录成功后删除旧备份并收紧新凭据权限。
fn finish_login(session: &LoginSession) -> String {
    let bot_name = session.bot_name();

    // 删除旧凭据备份。
    if let Some(replaced_token_path) = &session.replaced_token_path {
        remove_replaced_token(replaced_token_path);
    }

    tighten_token_permissions(&session.token_path());

    crate::config::Config::get()
        .text()
        .bot_login_succeeded(bot_name)
}

/// 将凭据权限设为 `0600`；失败时只记警告。
fn tighten_token_permissions(token_path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Err(e) = fs::set_permissions(
        token_path,
        fs::Permissions::from_mode(config::Config::get().constants.private_file_mode),
    ) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .bot_login_chmod_failed(&token_path.display().to_string(), &e.to_string())
        );
    }
}

/// 将已有凭据移至备份路径，并清理上次中断留下的备份。
fn move_existing_token_aside(bot_name: &str, token_path: &Path) -> Result<Option<PathBuf>, String> {
    let replaced_token_path = replaced_token_path(token_path);

    if replaced_token_path.exists() {
        if token_path.exists() {
            remove_replaced_token(&replaced_token_path);
        } else {
            restore_token_in_place(&replaced_token_path, token_path);
        }
    }

    if !token_path.exists() {
        return Ok(None);
    }

    fs::rename(token_path, &replaced_token_path).map_err(|e| {
        crate::config::Config::get()
            .text()
            .bot_token_write_failed(bot_name, &e.to_string())
    })?;

    Ok(Some(replaced_token_path))
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
            crate::config::Config::get()
                .text()
                .bot_login_old_token_remove_failed(
                    &replaced_token_path.display().to_string(),
                    &e.to_string()
                )
        );
    }
}

/// 恢复旧凭据；失败时只记日志。
fn restore_token_in_place(replaced_token_path: &Path, token_path: &Path) {
    if let Err(e) = fs::rename(replaced_token_path, token_path) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .bot_login_old_token_restore_failed(
                    &replaced_token_path.display().to_string(),
                    &e.to_string()
                )
        );
    }
}

/// 生成旧凭据备份路径。
fn replaced_token_path(token_path: &Path) -> PathBuf {
    let mut file_name = token_path.file_name().unwrap_or_default().to_os_string();
    file_name.push(format!(
        ".{}",
        config::Config::get().constants.token_backup_suffix
    ));

    token_path.with_file_name(file_name)
}

/// 从备份路径还原凭据路径。
fn restored_token_path(replaced_token_path: &Path) -> Option<PathBuf> {
    let file_name = replaced_token_path.file_name()?.to_str()?;
    let original_file_name = file_name.strip_suffix(&format!(
        ".{}",
        config::Config::get().constants.token_backup_suffix
    ))?;

    Some(replaced_token_path.with_file_name(original_file_name))
}

/// 停止登录会话，给 JAR 留出写入凭据的机会。
fn stop_login_session(session: &LoginSession) -> bool {
    if let Err(reason) = stop_single_bot_session(&session.bot_dir_entry) {
        log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .bot_login_stop_failed(session.bot_name(), &reason)
        );
        return false;
    }
    true
}

/// 等待 JAR 输出设备码，返回验证网址与用户代码。
fn wait_for_device_code(session: &LoginSession) -> Result<Option<(String, String)>, String> {
    let constants = &config::Config::get().constants;
    let deadline = Instant::now() + constants.login_code_timeout;

    while Instant::now() < deadline {
        if let Some(prompt) = parse_device_code_prompt(&read_log_tail(&session.log_path)) {
            return Ok(Some(prompt));
        }

        if !is_bot_screen_running(session.bot_name())? {
            return Ok(parse_device_code_prompt(&read_log_tail(&session.log_path)));
        }

        thread::sleep(constants.login_poll_interval);
    }

    Ok(parse_device_code_prompt(&read_log_tail(&session.log_path)))
}

/// 解析验证网址与用户代码；移除网址末尾句点，不限制代码格式。
fn parse_device_code_prompt(lines: &[String]) -> Option<(String, String)> {
    for line in lines {
        let Some(after_marker) = line
            .split(config::Config::get().constants.login_device_code_marker)
            .nth(1)
        else {
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
    read_last_lines(
        log_path,
        config::Config::get().constants.login_log_tail_lines,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析设备码、网址并去除网址末尾的句点。
    #[test]
    fn device_code_prompt_is_parsed() {
        config::init_for_test();
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
        config::init_for_test();
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
        config::init_for_test();
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
        config::init_for_test();
        let documented = include_str!("../../../docs/refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md");

        let constants = &config::Config::get().constants;
        for marker in [
            constants.login_device_code_marker,
            constants.login_auth_failure_marker,
        ] {
            assert!(documented.contains(marker), "清单里找不到: {marker}");
        }
    }

    /// 根据新旧凭据是否存在决定删除或恢复备份。
    #[test]
    fn login_preserves_the_newest_available_credential() {
        config::init_for_test();
        let dir = crate::test_support::temp_test_dir("login-backup");
        fs::create_dir_all(&dir).unwrap();
        let token = dir.join("refreshToken");
        let backup = replaced_token_path(&token);
        assert_eq!(move_existing_token_aside("duck", &token).unwrap(), None);
        fs::write(&token, b"first").unwrap();
        for expected in [b"first".as_slice(), b"first", b"newer"] {
            if expected == b"newer" {
                fs::write(&token, expected).unwrap();
            }
            assert_eq!(
                move_existing_token_aside("duck", &token).unwrap().as_ref(),
                Some(&backup)
            );
            assert!(!token.exists());
            assert_eq!(fs::read(&backup).unwrap(), expected);
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
