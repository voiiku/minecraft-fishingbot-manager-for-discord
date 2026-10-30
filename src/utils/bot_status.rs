//! 从日志实时判断连接状态；暂停状态根据指令回显保存在内存中。

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::thread;
use std::time::Duration;

use crate::config;
use crate::text;

use super::bot_dirs::BotDirEntry;
use super::bot_logs::latest_log_path;
use super::fs::{read_last_lines, read_lines_backwards_until};

/// 机器人到游戏服务器的连接状态：正常运行时只可能是这四种。
/// 其他情况（日志读取错误等）在正常运行里不会出现（日志由启动流程建好、建不出来就不拉会话）。
/// 所以 [`BotDirEntry::connection_status`] 把其他情况当错误报给调用者。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// 日志里还没有出现任何能判断状态的关键词（刚启动）。
    Initializing,
    /// 已经进入游戏服务器。
    Online,
    /// 正在重连：进程还在，但此刻机器人不在服务器（可能是玩家本人在线或是网络波动掉线），正在尝试重连。
    Reconnecting,
    /// 进程已结束：会话只剩 `(Dead ...)` 残壳（JVM 已被杀，screen 的空壳还挂着）。
    /// 由 `utils::running_bots` 按会话状态给出，不来自日志（因为日志最后一条可能还停在"在线"）。
    Stopped,
}

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
    /// 检查一行文本中是否包含了状态关键词。只可能得出"在线"与"重连"两种结论：
    /// "进程已结束"看会话状态（见 [`Self::Stopped`]），与日志无关。
    /// * 在线关键词来自配置（各部署不同：那是游戏服自己的进服提示）；
    /// * 重连提示要用前缀 + 后缀共同确认，避免把日志里凑巧出现的单个词当成状态，
    ///   这两句来自 JAR 自己（按当前语言取），所以不经过配置。
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
        // 一行都没扫到代表日志是空文件（正常运行时不会出现：建日志文件时就写了首行）。
        Ok(scanned_lines) if scanned_lines.is_empty() => {
            Err(text::get().bot_status_empty_log(bot_name))
        }
        // 没有能判断状态的关键词代表机器人刚启动。
        Ok(_) => Ok(connection.unwrap_or(ConnectionStatus::Initializing)),
        Err(_) => Err(text::get().bot_status_unreadable_log(bot_name)),
    }
}

/// 机器人的暂停状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseStatus {
    /// 正在钓鱼。
    Fishing,
    /// 收到 `/pause` 后暂停钓鱼（但没下线）。
    Paused,
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

/// 机器人的暂停状态， 由 [`BotPauseStatus`] 记忆，其他信息由上层调用方保管。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotPauseStatus {
    /// 暂停状态：只由 [`BotPauseStatus::set_paused`] / [`BotPauseStatus::set_fishing`] 改，由 [`BotPauseStatus::pause_status`] 读。
    pause: PauseStatus,
}

impl Default for BotPauseStatus {
    /// 初始状态：钓鱼中（新拉起的 JVM 一定没被暂停）。
    fn default() -> Self {
        Self {
            pause: PauseStatus::Fishing,
        }
    }
}

impl BotPauseStatus {
    /// 读当前的暂停状态的方法
    pub fn pause_status(&self) -> PauseStatus {
        self.pause
    }

    /// 上层调用者确认到 `/pause` 成功执行，将当前的暂停状态改为暂停。
    pub fn set_paused(&mut self) {
        self.pause = PauseStatus::Paused;
    }

    /// 上层调用者确认到 `/unpause` 成功执行，将当前的暂停状态改为钓鱼中。
    pub fn set_fishing(&mut self) {
        self.pause = PauseStatus::Fishing;
    }
}

/// 进程内保存的"所有在跑的机器人的状态"，按机器人名存，惰性存储，只在调用时更新。
/// 约定：只有能确认到命令成功执行的函数会改它（`/pause`、`/unpause` 里的 [`BotPauseStatusMemory::update_bot_pause_status`]），别处只读。
/// 手动操作（如/command bot /pause）不会更新它，因为自由指令是不可预测的。
/// 同一台机器人的 `/pause` 与 `/unpause` 同时到达时，状态按最新回显记录。
static ALL_RUNNING_BOTS_PAUSE_STATUSES: LazyLock<Mutex<HashMap<String, BotPauseStatus>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 按机器人名存储的暂停状态的入口：操作都实现在机器人名上
/// （`bot_dir_entry.bot_name.to_bot_pause_status()`）。
pub trait BotPauseStatusMemory {
    /// 取某个机器人的状态；若没有条目就是初始值，否则复制已经存在的状态。
    fn to_bot_pause_status(&self) -> BotPauseStatus;

    /// 改某个机器人的状态：`bot_name.update_bot_pause_status(BotPauseStatus::set_paused)`。
    fn update_bot_pause_status(&self, change: impl FnOnce(&mut BotPauseStatus));

    /// 忘掉某个机器人的状态：会话开始时与会话结束时都要调用。
    fn forget_bot_pause_status(&self);
}

impl BotPauseStatusMemory for str {
    fn to_bot_pause_status(&self) -> BotPauseStatus {
        ALL_RUNNING_BOTS_PAUSE_STATUSES
            .lock()
            .expect("尝试读取某个机器人的暂停状态时，暂停状态记忆的互斥锁被占用")
            .get(self)
            .copied()
            .unwrap_or_default()
    }

    fn update_bot_pause_status(&self, change: impl FnOnce(&mut BotPauseStatus)) {
        let mut bot_pause_statuses = ALL_RUNNING_BOTS_PAUSE_STATUSES
            .lock()
            .expect("尝试更新某个机器人的暂停状态时，暂停状态记忆的互斥锁被占用");
        change(bot_pause_statuses.entry(self.to_string()).or_default());
    }

    fn forget_bot_pause_status(&self) {
        let mut bot_pause_statuses = ALL_RUNNING_BOTS_PAUSE_STATUSES
            .lock()
            .expect("尝试忘掉某个机器人的暂停状态时，暂停状态记忆的互斥锁被占用");
        bot_pause_statuses.remove(self);
    }
}

/// 需要确认回显的两条指令：`/pause` 与 `/unpause`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCommand {
    /// 暂停钓鱼但保持在线（`/pause`）。
    SetPause,
    /// 取消暂停、恢复钓鱼（`/unpause`）。
    SetUnpause,
}

impl PauseCommand {
    /// 要注入游戏里的指令。
    pub fn command(self) -> &'static str {
        match self {
            Self::SetPause => "/pause",
            Self::SetUnpause => "/unpause",
        }
    }

    /// 另一条命令：`/pause` ↔ `/unpause`（判断"最后那条回显是谁的"时要用它）。
    fn other(self) -> Self {
        match self {
            Self::SetPause => Self::SetUnpause,
            Self::SetUnpause => Self::SetPause,
        }
    }

    /// 这条命令生效之后应该是什么状态（把"当前状态"报给用户时要用）。
    fn target_pause_status(self) -> PauseStatus {
        match self {
            Self::SetPause => PauseStatus::Paused,
            Self::SetUnpause => PauseStatus::Fishing,
        }
    }

    /// 确认到回显之后，把进程内记忆的暂停状态改成对应值（调 [`BotPauseStatus`] 自己的方法）。
    fn apply(self, status: &mut BotPauseStatus) {
        match self {
            Self::SetPause => status.set_paused(),
            Self::SetUnpause => status.set_fishing(),
        }
    }

    /// 这条命令的回显关键词：来自配置 `[command_echo]`（所用 JAR 支持 `/pause`、`/unpause` 时才会打印这两句）。
    /// 吃 `&CommandEchoConfig` 而不是自己调 `config::Config::get()`，是为了让"最后一条回显是谁的"能单测。
    fn echo_keyword(self, command_echo: &config::CommandEchoConfig) -> &str {
        match self {
            Self::SetPause => &command_echo.pause_echo,
            Self::SetUnpause => &command_echo.unpause_echo,
        }
    }

    /// 判断日志窗口里最后出现的那条 pause/unpause 回显属于哪条指令（两条都不在窗口里就 `None`）。
    /// 当两条指令被同时执行时，只有最新的返回是现在的真正状态。
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

/// 发一条需要确认回显的指令，确认它是否生效，并记下这一次的暂停状态。
/// 只有 `/pause`、`/unpause` 用它：它们的成功回显写在配置 `[command_echo]` 里
/// （`PauseCommand::echo_keyword`）；其他指令无法预测回显。
/// 1. 先 `inject()`：失败直接返回；
/// 2. 等配置里 `command_echo.wait`（JAR 收到指令会同步打一行回显，等它写入日志）；
/// 3. 看最新日志最后 `command_echo.scan_lines` 行里最新的一条 pause/unpause 回显是谁的
///    （`PauseCommand::last_confirmed_pause_command`）；
/// 4. 是本次执行预期的回显 → 记下状态、返回 `Ok(true)`；是另一条指令的回显 → 状态按它记（它才是真的），
///    返回 `Err`（带上"现在的状态是哪个"，让调用方如实回一句）；两条都不在 → 不下结论、`Ok(false)`。
///
/// 返回true = 本条命令生效；返回false = 指令已发送但没回显（关键词不对 / 等待时间太短，
/// 来不及写入 / 日志名被更换等），此时调用方只能照实说"发了但没确认到"；`Err` = 注入失败、
/// 找不到合法日志、读日志失败。最新的回显是另一条（指令已发出，但结果被后到的指令覆盖）时
/// 也返回 `Err`，调用方照实回一句即可。
pub fn confirm_pause_command(
    bot_dir_entry: &BotDirEntry,
    inject: impl FnOnce() -> Result<(), String>,
    pause_command: PauseCommand,
) -> Result<bool, String> {
    inject()?;

    thread::sleep(Duration::from_millis(
        config::Config::get().command_echo.wait_ms,
    ));

    // 回显一定落在最新一份日志的末尾；没有合法日志就是错误
    //（这时指令已经发出去了，调用方只能说"发了但没确认到"）。
    let Some(log_path) = latest_log_path(&bot_dir_entry.bot_dir_path) else {
        return Err(text::get().bot_status_log_not_found(&bot_dir_entry.bot_name));
    };
    let recent_lines = read_last_lines(&log_path, config::Config::get().command_echo.scan_lines)?;

    // 最新的回显决定这次调用的结果
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
            Err(text::get().bot_pause_superseded(
                pause_command.command(),
                &latest_pause_command.target_pause_status().to_string(),
            ))
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
