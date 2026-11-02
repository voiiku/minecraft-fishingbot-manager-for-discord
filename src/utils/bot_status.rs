//! 从日志实时判断连接状态；暂停状态根据指令回显保存在内存中。

use std::fmt::{self, Display, Formatter};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config;
use crate::runtime::bot_state::PAUSE_STATUSES;
use crate::text;

use super::bot_dirs::BotDirEntry;
use super::bot_logs::latest_log_path;
use super::fs::{read_last_lines_after, read_lines_backwards_until};
pub use crate::models::{BotPauseStatus, ConnectionStatus, PauseCommand, PauseStatus};

impl Display for ConnectionStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let i18n_text = text::get();
        let label = match self {
            Self::Initializing => i18n_text.bot_status_initializing(),
            Self::Online => i18n_text.bot_status_online(),
            Self::Reconnecting => i18n_text.bot_status_reconnecting(),
            Self::Stopped => i18n_text.bot_status_stopped(),
        };

        write!(f, "{label}")
    }
}

impl ConnectionStatus {
    /// 从日志行判断在线或重连状态；重连提示须同时匹配 JAR 文案的前缀和后缀。
    fn from_line(line: &str, online_keyword: &str) -> Option<Self> {
        let i18n_text = text::get();

        if line.contains(online_keyword) {
            return Some(Self::Online);
        }
        if line.contains(i18n_text.jar_reconnect_prefix())
            && line.contains(i18n_text.jar_reconnect_suffix())
        {
            return Some(Self::Reconnecting);
        }

        None
    }
}

impl BotDirEntry {
    /// 根据最新日志判断连接状态；进程结束由 screen 会话状态判断。
    pub fn connection_status(&self) -> Result<ConnectionStatus, String> {
        let log_path = latest_log_path(&self.bot_dir_path);

        connection_status_of_log(&self.bot_name, log_path.as_deref())
    }
}

/// 从日志末尾读取最新的状态关键词。
fn connection_status_of_log(
    bot_name: &str,
    log_path: Option<&Path>,
) -> Result<ConnectionStatus, String> {
    let Some(log_path) = log_path else {
        return Err(text::get().bot_status_log_not_found(bot_name));
    };

    let online_keyword = &config::Config::get().keywords.online;
    let mut connection: Option<ConnectionStatus> = None;

    match read_lines_backwards_until(log_path, |line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        connection = ConnectionStatus::from_line(trimmed, online_keyword);

        connection.is_some()
    }) {
        Ok(scanned_lines) if scanned_lines.is_empty() => {
            Err(text::get().bot_status_empty_log(bot_name))
        }
        Ok(_) => Ok(connection.unwrap_or(ConnectionStatus::Initializing)),
        Err(_) => Err(text::get().bot_status_unreadable_log(bot_name)),
    }
}

impl Display for PauseStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let i18n_text = text::get();
        let label = match self {
            Self::Fishing => i18n_text.bot_pause_fishing(),
            Self::Paused => i18n_text.bot_pause_paused(),
        };

        write!(f, "{label}")
    }
}

/// 按机器人名查询和更新暂停状态。
pub trait BotPauseStatusMemory {
    /// 返回机器人暂停状态；无记录时使用初始状态。
    fn to_bot_pause_status(&self) -> BotPauseStatus;

    /// 更新机器人暂停状态。
    fn update_bot_pause_status(&self, change: impl FnOnce(&mut BotPauseStatus));

    /// 清除机器人暂停状态；会话开始和结束时调用。
    fn forget_bot_pause_status(&self);
}

impl BotPauseStatusMemory for str {
    fn to_bot_pause_status(&self) -> BotPauseStatus {
        PAUSE_STATUSES
            .lock()
            .expect("机器人暂停状态锁中毒：读取失败")
            .get(self)
            .copied()
            .unwrap_or_default()
    }

    fn update_bot_pause_status(&self, change: impl FnOnce(&mut BotPauseStatus)) {
        let mut bot_pause_statuses = PAUSE_STATUSES
            .lock()
            .expect("机器人暂停状态锁中毒：更新失败");
        change(bot_pause_statuses.entry(self.to_string()).or_default());
    }

    fn forget_bot_pause_status(&self) {
        let mut bot_pause_statuses = PAUSE_STATUSES
            .lock()
            .expect("机器人暂停状态锁中毒：清除失败");
        bot_pause_statuses.remove(self);
    }
}

impl PauseCommand {
    /// 返回配置中的指令回显关键词。
    fn echo_keyword(self, command_echo: &config::CommandEchoConfig) -> &str {
        match self {
            Self::SetPause => &command_echo.pause_echo,
            Self::SetUnpause => &command_echo.unpause_echo,
        }
    }

    /// 返回日志窗口中最新的暂停或恢复指令回显；无回显时返回 `None`。
    fn last_confirmed_pause_command(
        recent_lines: &[String],
        pause_command: Self,
        command_echo: &config::CommandEchoConfig,
    ) -> Option<Self> {
        let my_echo_keyword = pause_command.echo_keyword(command_echo);
        let other_echo_keyword = pause_command.other().echo_keyword(command_echo);

        recent_lines
            .iter()
            .filter_map(|line| {
                if line.contains(my_echo_keyword) {
                    Some(pause_command)
                } else if line.contains(other_echo_keyword) {
                    Some(pause_command.other())
                } else {
                    None
                }
            })
            .next_back()
    }
}

/// 注入暂停或恢复指令，并根据发送后的日志回显更新状态。
/// 本次回显已确认返回 `Ok(true)`；无回显返回 `Ok(false)`。
/// 注入或日志读取失败，以及指令被较新的相反指令回显取代时返回 `Err`。
pub fn confirm_pause_command(
    bot_dir_entry: &BotDirEntry,
    inject: impl FnOnce() -> Result<(), String>,
    pause_command: PauseCommand,
) -> Result<bool, String> {
    let previous_log = latest_log_path(&bot_dir_entry.bot_dir_path)
        .map(|path| fs::metadata(&path).map(|metadata| (path, metadata)))
        .transpose()
        .map_err(|_| text::get().bot_status_unreadable_log(&bot_dir_entry.bot_name))?;
    inject()?;

    thread::sleep(Duration::from_millis(
        config::Config::get().command_echo.wait_ms,
    ));

    let Some(log_path) = latest_log_path(&bot_dir_entry.bot_dir_path) else {
        return Err(text::get().bot_status_log_not_found(&bot_dir_entry.bot_name));
    };
    let metadata = fs::metadata(&log_path)
        .map_err(|_| text::get().bot_status_unreadable_log(&bot_dir_entry.bot_name))?;
    let start = previous_log
        .filter(|(path, previous)| {
            *path == log_path
                && previous.dev() == metadata.dev()
                && previous.ino() == metadata.ino()
                && previous.len() <= metadata.len()
        })
        .map_or(0, |(_, previous)| previous.len());
    let recent_lines = read_last_lines_after(
        &log_path,
        start,
        config::Config::get().command_echo.scan_lines,
    )
    .map_err(|message| text::get().bot_line(&bot_dir_entry.bot_name, &message))?;

    let command_echo = &config::Config::get().command_echo;
    match PauseCommand::last_confirmed_pause_command(&recent_lines, pause_command, command_echo) {
        Some(confirmed_pause_command) if confirmed_pause_command == pause_command => {
            bot_dir_entry
                .bot_name
                .update_bot_pause_status(|status| confirmed_pause_command.apply(status));
            Ok(true)
        }
        Some(latest_pause_command) => {
            bot_dir_entry
                .bot_name
                .update_bot_pause_status(|status| latest_pause_command.apply(status));
            let message = text::get().bot_pause_superseded(
                pause_command.command(),
                &latest_pause_command.target_pause_status().to_string(),
            );
            Err(text::get().bot_line(&bot_dir_entry.bot_name, &message))
        }
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用回显：与示例配置 `[command_echo]` 的默认值一致，直接构造，不读取真实配置。
    fn command_echo() -> config::CommandEchoConfig {
        config::CommandEchoConfig {
            pause_echo: "Fishing paused".to_string(),
            unpause_echo: "Fishing resumed".to_string(),
            wait_ms: 1,
            scan_lines: 3,
        }
    }

    /// 测试用关键词：仅填充部署相关的两个字段，单行判定直接使用参数。
    /// JAR 固定文案由 `text::get()` 提供。
    fn keywords() -> config::KeywordsConfig {
        config::KeywordsConfig {
            online: "回到测试服".to_string(),
            filter_regex: "XP".to_string(),
        }
    }

    /// 单行判定只识别在线关键词；重连判定由下一测试覆盖。
    /// 完整流程（读取日志、命中后停止、读不到日志时的错误）由
    /// [`connection_status_of_log`] 覆盖；该函数需要真实配置，因此此处仅测试按参数判定的单行逻辑。
    /// “向后读取、命中后停止”的行为由 `bot_logs` 的测试覆盖。
    #[test]
    fn connection_keywords_are_recognized() {
        let keywords = keywords();
        let of_line = |line: &str| ConnectionStatus::from_line(line, &keywords.online);

        assert_eq!(of_line(&keywords.online), Some(ConnectionStatus::Online));
        // 旧的结束标记不参与连接状态判定；进程结束状态由会话状态提供。
        assert_eq!(of_line(">>> 机器人进程已结束或被手动中止 <<<"), None);
        // 不包含关键词的行不产生连接状态。
        assert_eq!(of_line("正在加载配置"), None);
    }

    #[test]
    fn online_keyword_must_match_a_contiguous_log_substring() {
        let line = "[INFO] 欢迎 duck 回到示例服，很高兴认识你";
        assert_eq!(ConnectionStatus::from_line(line, "欢迎回到示例服"), None);
        assert_eq!(
            ConnectionStatus::from_line(line, "回到示例服"),
            Some(ConnectionStatus::Online)
        );
    }

    /// 重连提示必须同时包含前缀与后缀；只出现其中一个不足以确认重连。
    /// 提示文案来自当前语言的 JAR 实现，因此测试使用同一语言表拼接日志行。
    #[test]
    fn reconnect_needs_both_affixes() {
        let keywords = keywords();
        let i18n_text = text::get();
        let of_line = |line: &str| ConnectionStatus::from_line(line, &keywords.online);

        // 前后缀都在：判为重连。
        assert_eq!(
            of_line(&format!(
                "[INFO   ] {}{}{}",
                i18n_text.jar_reconnect_prefix(),
                5,
                i18n_text.jar_reconnect_suffix()
            )),
            Some(ConnectionStatus::Reconnecting)
        );
        // 仅出现前缀时不产生结论。
        assert_eq!(
            of_line(&format!("[INFO   ] {}5", i18n_text.jar_reconnect_prefix())),
            None
        );
        // 仅出现后缀时同样不产生结论。
        assert_eq!(
            of_line(&format!("[INFO   ] 5{}", i18n_text.jar_reconnect_suffix())),
            None
        );
    }

    /// 状态默认是"钓鱼中"；只有 [`BotPauseStatus::set_paused`] / [`BotPauseStatus::set_fishing`] 会改它。
    #[test]
    fn pause_state_defaults_to_fishing() {
        let mut status = BotPauseStatus::default();
        assert_eq!(status.pause_status(), PauseStatus::Fishing);

        status.set_paused();
        assert_eq!(status.pause_status(), PauseStatus::Paused);

        status.set_fishing();
        assert_eq!(status.pause_status(), PauseStatus::Fishing);
    }

    /// 找不到日志文件（从未启动或日志已移走）时返回错误，不将其视为机器人状态。
    /// 该测试覆盖 `None` 分支，不读取真实配置。
    #[test]
    fn missing_log_is_an_error() {
        assert!(connection_status_of_log("duck", None).is_err());
    }

    /// 持续状态按机器人名保存：无记录时为初始值，更新后可按名称读取，移除记录后恢复初始值。
    #[test]
    fn bot_status_is_remembered_by_name() {
        let bot_name = "minecraft-fishingbot-manager-for-discord-测试-状态记忆";

        bot_name.forget_bot_pause_status();
        assert_eq!(
            bot_name.to_bot_pause_status().pause_status(),
            PauseStatus::Fishing
        );

        bot_name.update_bot_pause_status(BotPauseStatus::set_paused);
        assert_eq!(
            bot_name.to_bot_pause_status().pause_status(),
            PauseStatus::Paused
        );

        // 其他机器人的状态不受影响。
        assert_eq!(
            "另一个机器人".to_bot_pause_status().pause_status(),
            PauseStatus::Fishing
        );

        // 会话结束或重新开始时移除记录，状态恢复为初始值。
        bot_name.forget_bot_pause_status();
        assert_eq!(
            bot_name.to_bot_pause_status().pause_status(),
            PauseStatus::Fishing
        );
    }

    /// 回显关键词来自配置：两条指令分别读取自身配置项。
    #[test]
    fn echo_keywords_come_from_the_config() {
        let command_echo = command_echo();

        assert_eq!(
            PauseCommand::SetPause.echo_keyword(&command_echo),
            command_echo.pause_echo
        );
        assert_eq!(
            PauseCommand::SetUnpause.echo_keyword(&command_echo),
            command_echo.unpause_echo
        );
    }

    /// 以窗口内最后一条 pause/unpause 回显判定实际生效的指令。
    /// 同一机器人的两条命令并发时，两条回显都会进入窗口，只有最后一条有效。
    #[test]
    fn the_last_pause_or_unpause_echo_wins() {
        let command_echo = command_echo();
        let pause_echo = command_echo.pause_echo.as_str();
        let unpause_echo = command_echo.unpause_echo.as_str();

        let as_lines = |lines: &[&str]| {
            lines
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
        };
        let last_of = |lines: &[&str], pause_command: PauseCommand| {
            PauseCommand::last_confirmed_pause_command(
                &as_lines(lines),
                pause_command,
                &command_echo,
            )
        };

        assert_eq!(
            last_of(&[pause_echo], PauseCommand::SetPause),
            Some(PauseCommand::SetPause)
        );
        // `/pause` 之后出现 `/unpause`：对 `/pause` 而言表示未生效。
        assert_eq!(
            last_of(&[pause_echo, unpause_echo], PauseCommand::SetPause),
            Some(PauseCommand::SetUnpause)
        );
        assert_eq!(
            last_of(&[unpause_echo, pause_echo], PauseCommand::SetPause),
            Some(PauseCommand::SetPause)
        );
        assert_eq!(
            last_of(&[pause_echo, unpause_echo], PauseCommand::SetUnpause),
            Some(PauseCommand::SetUnpause)
        );
        // 两条回显均不在窗口中时返回 `None`。
        assert_eq!(last_of(&["正在加载配置"], PauseCommand::SetPause), None);
    }

    /// 指令名与确认后应更新的状态必须一一对应。
    #[test]
    fn pause_command_names_and_targets() {
        assert_eq!(PauseCommand::SetPause.command(), "/pause");
        assert_eq!(PauseCommand::SetUnpause.command(), "/unpause");

        let mut status = BotPauseStatus::default();

        PauseCommand::SetPause.apply(&mut status);
        assert_eq!(status.pause_status(), PauseStatus::Paused);

        PauseCommand::SetUnpause.apply(&mut status);
        assert_eq!(status.pause_status(), PauseStatus::Fishing);
    }
}
