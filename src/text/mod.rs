//! 文案按 `[i18n] language` 选择，每个进程使用一种语言。
//! 回复、日志与状态文案由 [`Text`] 提供；首次初始化完成前的错误使用英文。
//! 看门狗锁中毒的诊断文案随启动阶段和 `[i18n] language` 切换。
//! JAR 日志中的固定关键词也随语言选择，各部署自定义的关键词仍放在配置中。

pub mod bootstrap;
mod languages;
pub mod report;

use languages::{en_us, zh_cn};

use std::time::Duration;

/// 看门狗互斥锁中毒时的诊断上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogText {
    /// 重置连接尝试起始时刻。
    StartedLockReset,
    /// 清除上次握手状态。
    LastHandshakeLockClear,
    /// 记录握手时刻。
    LastHandshakeLockRecord,
    /// 读取握手状态。
    LastHandshakeLockRead,
    /// 清除探活失败起始时刻。
    UnhealthySinceLockClear,
    /// 记录探活失败起始时刻。
    UnhealthySinceLockRecord,
    /// 读取探活失败起始时刻。
    UnhealthySinceLockRead,
    /// 读取连接尝试起始时刻。
    StartedLockRead,
    /// 记录掉线原因与时刻。
    OfflineSinceLockRecord,
    /// 读取掉线记录。
    OfflineSinceLockRead,
    /// 记录待处理掉线原因。
    PendingDisconnectReasonLockRecord,
    /// 读取待处理掉线原因。
    PendingDisconnectReasonLockRead,
    /// 读取通知上下文以发送收尾报告。
    NoticeCtxLockSendReport,
    /// 记录 Discord 通知上下文。
    NoticeCtxLockRecord,
}

#[cfg(test)]
impl WatchdogText {
    const ALL: [Self; 14] = [
        Self::StartedLockReset,
        Self::LastHandshakeLockClear,
        Self::LastHandshakeLockRecord,
        Self::LastHandshakeLockRead,
        Self::UnhealthySinceLockClear,
        Self::UnhealthySinceLockRecord,
        Self::UnhealthySinceLockRead,
        Self::StartedLockRead,
        Self::OfflineSinceLockRecord,
        Self::OfflineSinceLockRead,
        Self::PendingDisconnectReasonLockRecord,
        Self::PendingDisconnectReasonLockRead,
        Self::NoticeCtxLockSendReport,
        Self::NoticeCtxLockRecord,
    ];
}

fn format_seconds(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let nanos = duration.subsec_nanos();
    if nanos == 0 {
        seconds.to_string()
    } else {
        format!("{seconds}.{:09}", nanos)
            .trim_end_matches('0')
            .to_string()
    }
}

/// 支持的语言；名称与 FishingBot 的语言常量一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 简体中文，对应 JAR 的 `CHINESE_SIMPLIFIED`。
    ChineseSimplified,
    /// 英语。
    English,
}

impl Language {
    /// 将配置中的语言名解析为语言实现。
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name.trim() {
            "CHINESE_SIMPLIFIED" => Ok(Self::ChineseSimplified),
            "ENGLISH" => Ok(Self::English),
            unknown => Err(bootstrap::language_unknown(unknown)),
        }
    }

    /// 返回支持的语言名。
    pub fn supported_names() -> &'static [&'static str] {
        &["ENGLISH", "CHINESE_SIMPLIFIED"]
    }

    /// 返回 FishingBot `config.json` 中使用的语言名。
    pub fn name(self) -> &'static str {
        match self {
            Self::ChineseSimplified => "CHINESE_SIMPLIFIED",
            Self::English => "ENGLISH",
        }
    }

    /// 返回语言实现。
    pub fn implementation(self) -> &'static dyn Text {
        match self {
            Self::ChineseSimplified => &zh_cn::ZH_CN,
            Self::English => &en_us::EN_US,
        }
    }
}

/// 程序日志与控制台信息的本地化文案。
pub trait LogMessages {
    /// 新日志文件的首行。
    fn process_log_first_line(&self) -> String;
    /// 程序日志文件的位置。
    fn process_log_path(&self, path: &str) -> String;
    /// 日志记录器安装失败，仅输出到终端。
    fn process_log_install_failed(&self, reason: &str) -> String;
    /// 日志记录器已安装，保留原实例。
    fn process_log_already_installed(&self) -> String;
    /// 日志输出或文件锁失败；由日志记录器直接输出，避免递归记录。
    fn process_log_output_failed(&self, destination: &str, operation: &str, reason: &str)
    -> String;
    /// 配置加载成功。
    fn config_loaded(&self, path: &str) -> String;
    /// 进程 PID。
    fn process_id(&self, pid: u32) -> String;
    /// 已连接 Discord。
    fn discord_connected(&self, application_id: u64) -> String;
    /// 邀请链接。
    fn invite_url(&self, url: &str) -> String;
    /// 收到退出信号。
    fn shutdown_signal(&self) -> String;
    /// REST 探活失败，等待重试。
    fn probe_failed(&self, reason: &str) -> String;
    /// 网关看门狗就绪及其实际时序参数。
    fn watchdog_ready(
        &self,
        window: Duration,
        probe_interval: Duration,
        first_handshake_timeout: Duration,
    ) -> String;
    /// 指令回调返回了 `Err`。
    fn command_run_failed(&self, error: &str) -> String;
    /// 发送指令失败提示时出错。
    fn command_fallback_reply_failed(
        &self,
        caller_id: u64,
        command_path: &str,
        reason: &str,
    ) -> String;
    /// 自动候选查询失败。
    fn autocomplete_lookup_failed(&self, reason: &str) -> String;
    /// 自动候选查询任务失败。
    fn autocomplete_task_failed(&self, reason: &str) -> String;
    /// 指令回复日志；`command_path` 是不带斜杠的 poise `qualified_name`。
    fn command_replied(&self, caller_id: u64, command_path: &str, reply: &str) -> String;
    /// 交互回复失败，最终报告改发频道消息；交互令牌的有效期为 15 分钟。
    fn report_posted_to_channel(&self, caller_id: u64, command_path: &str, reason: &str) -> String;
    /// 框架级错误（非指令级）。
    fn framework_error(&self, detail: &str) -> String;
    /// 注册 SIGTERM 处理失败。
    fn sigterm_register_failed(&self, reason: &str) -> String;
    /// 看门狗判定首次握手超时，停止机器人后重连。
    fn gateway_lost_never_ready(&self) -> String;
    /// 看门狗判定探活持续失败，停止机器人后重连。
    fn gateway_lost_unreachable(&self) -> String;
    /// 计划在指定延迟后重连。
    fn reconnect_scheduled(&self, attempt: u32, delay: Duration) -> String;
    /// 重连失败，将在指定延迟后重试。
    fn reconnect_failed(&self, attempt: u32, delay: Duration, reason: &str) -> String;
    /// 掉线后重新连接，报告通知发送结果。
    fn reconnected_after_offline(
        &self,
        offline_secs: u64,
        notified_guilds: usize,
        failed_guilds: usize,
    ) -> String;
    /// 发送恢复通知失败。
    fn reconnect_notice_failed(&self, guild_name: &str, reason: &str) -> String;
    /// 服务器没有可发通知的频道。
    fn reconnect_notice_no_channel(&self) -> String;
    /// Discord 重连通知正文。
    fn reconnect_notice_body(&self, offline_since: &str, offline_secs: u64) -> String;
    /// Discord 客户端初始化失败。
    fn discord_init_failed(&self, error: &str) -> String;
    /// 初始化失败：机器人未被邀请，或缺少 `applications.commands` 权限。
    fn discord_init_failed_not_invited(&self, error: &str) -> String;
    /// 初始化失败：Discord Token 无效或已重置。
    fn discord_init_failed_bad_token(&self, error: &str) -> String;
    /// 已清理服务器遗留的 guild 指令。
    fn guild_commands_cleared(&self, guild_name: &str, count: usize) -> String;
    /// 清理某个服务器上遗留的 guild 指令失败；需手动再次执行同步。
    fn guild_commands_clear_failed(&self, guild_name: &str, reason: &str) -> String;
    /// 与 Discord 的连接正常结束（客户端返回 `Ok`）。
    fn discord_client_ended(&self) -> String;
    /// 与 Discord 的连接出错。
    fn discord_client_error(&self, error: &str) -> String;
    /// 掉线或永久错误导致进程退出时的收尾报告标题。
    fn shutdown_report_title(&self) -> String;
    /// 收到退出信号后收尾报告的标题（正常退出，与 `/stopall` 同一份明细）。
    fn shutdown_signal_report_title(&self) -> String;
    /// 发送退出信号收尾报告失败。
    fn shutdown_report_notice_failed(&self, guild_name: &str, reason: &str) -> String;
    /// 收到退出信号时的收尾报告已发往各服务器。
    fn shutdown_report_notified(&self, notified_guilds: usize, failed_guilds: usize) -> String;
    /// 无可用 Discord 上下文，退出收尾报告仅写入日志。
    fn shutdown_report_no_context(&self) -> String;
    /// 退出收尾报告发送超时，跳过剩余消息。
    fn shutdown_report_timeout(&self, seconds: u64) -> String;
    /// 收到单机器人重启指令。
    fn bot_restart_requested(&self, bot_name: &str) -> String;
    /// 机器人已停止。
    fn bot_stopped(&self, bot_name: &str) -> String;
    /// 机器人目录创建成功。
    fn bot_added(&self, bot_name: &str) -> String;
    /// 机器人添加失败。
    fn bot_add_failed_log_message(&self, raw_bot_name: &str, reason: &str) -> String;
    /// 机器人删除失败。
    fn bot_delete_failed_log_message(&self, bot_name: &str, reason: &str) -> String;
    /// 机器人已删除。
    fn bot_deleted(&self, bot_name: &str) -> String;
    /// 机器人日志文件的第一行。
    fn bot_log_first_line(&self) -> String;
    /// 创建机器人日志文件失败。
    fn start_log_file_failed_log_message(&self, bot_name: &str, reason: &str) -> String;
    /// 正在启动会话。
    fn start_session_launched(&self, bot_name: &str, session_name: &str, log_path: &str) -> String;
    /// 启动结果复核失败。
    fn start_verification_failed(&self, bot_name: &str, reason: &str) -> String;
    /// 单机器人启动发生 panic，失败明细已写入账本。
    fn start_launch_panicked(&self, bot_name: &str, reason: &str) -> String;
    /// 等待启动结果超时：该机器人的"未确认"明细已写回账本。
    fn start_result_timeout_log_message(&self, bot_name: &str, timeout: Duration) -> String;
    /// 启动线程发生 panic，准备重启。
    fn starter_restarting(&self, reason: &str) -> String;
    /// FishingBot 日志目录包含意外文件。
    fn unexpected_game_log_file(&self, dir: &str, file_name: &str) -> String;
    /// 清空 FishingBot 日志目录失败。
    fn game_logs_dir_remove_failed(&self, dir: &str, reason: &str) -> String;
    /// 创建 FishingBot 日志目录失败。
    fn game_logs_dir_create_failed(&self, dir: &str, reason: &str) -> String;
    /// 将 FishingBot 日志文件指向丢弃设备失败。
    fn game_log_discard_failed(&self, file: &str, device: &str, reason: &str) -> String;
    /// 登录被微软拒绝，会话已停止。
    fn bot_login_rejected_log_message(&self, bot_name: &str) -> String;
    /// 登录结束后停止会话失败。
    fn bot_login_stop_failed(&self, bot_name: &str, reason: &str) -> String;
    /// 删除旧凭据失败。
    fn bot_login_old_token_remove_failed(&self, path: &str, reason: &str) -> String;
    /// 恢复旧凭据失败。
    fn bot_login_old_token_restore_failed(&self, path: &str, reason: &str) -> String;
    /// 将凭据文件权限设为 0600 失败。
    fn bot_login_chmod_failed(&self, path: &str, reason: &str) -> String;
    /// 已删除旧游戏侧配置文件。
    fn fishingbot_config_removed(&self, path: &str) -> String;
    /// 已生成游戏侧配置文件。
    fn fishingbot_config_written(&self, path: &str) -> String;
    /// 创建日志目录失败。
    fn create_log_dir_failed(&self, dir: &str, reason: &str) -> String;
    /// 创建日志文件失败。
    fn create_log_file_failed(&self, path: &str, reason: &str) -> String;
    /// 写入日志首行失败。
    fn write_log_first_line_failed(&self, path: &str, reason: &str) -> String;
    /// 打开日志文件失败。
    fn open_log_file_failed(&self, reason: &str) -> String;
    /// 读取日志文件元数据失败。
    fn read_log_metadata_failed(&self, reason: &str) -> String;
    /// 定位日志文件失败。
    fn seek_log_file_failed(&self, reason: &str) -> String;
    /// 读取日志内容失败。
    fn read_log_content_failed(&self, reason: &str) -> String;
}

/// 面向 Discord 的文案接口。
pub trait Text: LogMessages + Send + Sync {
    // ========================== 报告（`text/report.rs`） ==========================
    /// 已完成处理的明细标题；结果仍可能未确认。
    fn report_succeeded_label(&self) -> &'static str;
    /// "跳过明细"小节标题。
    fn report_skipped_label(&self) -> &'static str;
    /// `/show status` 的排队机器人小节标题。
    fn report_queued_label(&self) -> &'static str;
    /// 失败或未完成处理的明细标题。
    fn report_failed_label(&self) -> &'static str;
    /// 整块内容的标签（原文里是 `【内容】:`）。
    fn report_content_label(&self) -> &'static str;
    /// 按行截断时的说明，`dropped_line_count` 是被省略的行数。
    fn report_omitted_lines_note(&self, dropped_line_count: usize) -> String;

    // ===================== 看门狗诊断文案 =====================
    /// 看门狗互斥锁中毒时的诊断信息；随 `[i18n] language` 切换。
    fn watchdog_text(&self, message: WatchdogText) -> &'static str;

    // ===================== 机器人状态（`runtime/bot_instances/bot_status.rs`） =====================
    /// 机器人日志文件不存在。
    fn bot_status_log_not_found(&self, bot_name: &str) -> String;
    /// 机器人日志文件无法读取。
    fn bot_status_unreadable_log(&self, bot_name: &str) -> String;
    /// 已经进入游戏服务器。
    fn bot_status_joined(&self) -> &'static str;
    /// FishingBot 未进入游戏服务器。
    fn bot_status_not_joined(&self) -> &'static str;
    /// FishingBot 无法提供状态。
    fn bot_status_unavailable(&self) -> &'static str;
    /// 进程已结束或被手动中止。
    fn bot_status_stopped(&self) -> &'static str;
    /// 钓鱼未暂停，不代表正在钓鱼。
    fn bot_pause_unpaused(&self) -> &'static str;
    /// 钓鱼已暂停，机器人仍在线。
    fn bot_pause_paused(&self) -> &'static str;
    /// FishingBot 报告状态不可用。
    fn bot_status_field_unavailable(&self) -> &'static str;
    /// 未收到状态回显。
    fn bot_status_field_unknown(&self) -> &'static str;
    /// 查询期间日志发生变化。
    fn bot_status_log_changed(&self, bot_name: &str) -> String;
    /// 未收到有效状态回显。
    fn bot_status_query_timeout(&self, bot_name: &str) -> String;
    /// 暂停指令已发送，但后续查询失败。
    fn bot_status_query_after_command_failed(&self, command: &str, error: &str) -> String;

    // ========== FishingBot.jar 固定文案（`runtime/bot_instances/bot_status.rs`）==========
    /// FishingBot 状态回显的前缀。
    fn jar_status_prefix(&self) -> &'static str;
    /// FishingBot 状态不可用回显。
    fn jar_status_unavailable(&self) -> &'static str;

    // ===================== 机器人名（`utils/bot_name.rs`） =====================
    /// 名字为空（或只有空白）。
    fn bot_name_empty(&self) -> &'static str;
    /// 名字含非法字符。
    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String;

    // =================== 单机器人指令（`operations/*`） ===================
    /// `/start`：没有 Token 文件，无法启动。
    fn bot_start_missing_token(&self, bot_name: &str) -> String;
    /// `/start`：screen 会话已存在。
    fn bot_start_already_running(&self, bot_name: &str) -> String;
    /// 机器人已在启动队列中。
    fn bot_start_already_queued(&self, bot_name: &str) -> String;
    /// `/stop`：screen 会话不存在。
    fn bot_stop_not_running(&self, bot_name: &str) -> String;
    /// `/stop`：已确认停止。
    fn bot_stop_succeeded(&self, bot_name: &str) -> String;
    /// 机器人尚未启动，排队请求已取消。
    fn bot_stop_cancelled_queue(&self, bot_name: &str) -> String;
    /// `/restart`：重启后已确认进服。
    fn bot_restart_online_confirmed(&self, bot_name: &str) -> String;
    /// `/restart`：新会话存在，但当前未进服。
    fn bot_restart_not_joined(&self, bot_name: &str) -> String;
    /// `/command`：指令为空。
    fn bot_command_empty(&self) -> &'static str;
    /// `/command`：screen 会话不存在。
    fn bot_command_not_running(&self, bot_name: &str) -> String;
    /// `/command`：指令已注入。
    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String;
    /// `/pause`：查询确认当前处于暂停状态。
    fn bot_pause_confirmed(&self, bot_name: &str) -> String;
    /// `/unpause`：查询确认当前未暂停。
    fn bot_unpause_confirmed(&self, bot_name: &str) -> String;
    /// 指令已发送，但 FishingBot 无法提供状态。
    fn bot_pause_query_unavailable(&self, bot_name: &str, bot_command: &str) -> String;
    /// 查询结果与指令的目标状态不同。
    fn bot_pause_status_mismatch(&self, my_command: &str, current_pause_status: &str) -> String;
    /// `/log`：行数必须大于 0。
    fn bot_log_zero_lines(&self) -> &'static str;
    /// `/log`：日志文件或文件名不可用。
    fn bot_log_missing(&self, bot_name: &str) -> String;

    // =================== 账号管理指令（`operations/accounts/*`） ===================
    /// `/bot add`：目录已创建。
    fn bot_add_succeeded(&self, bot_name: &str) -> String;
    /// `/bot add`：目录创建失败；`reason` 保留原始原因。
    fn bot_add_failed(&self, raw_bot_name: &str, reason: &str) -> String;
    /// `/bot del`：删除目录失败。
    fn bot_delete_failed(&self, bot_name: &str, reason: &str) -> String;
    /// `/bot del`：目录已删除。
    fn bot_delete_succeeded(&self, bot_name: &str) -> String;
    /// `/bot rename`：新旧名称相同。
    fn bot_rename_same_name(&self) -> &'static str;
    /// `/bot rename`：目标名称已存在。
    fn bot_rename_name_taken(&self, clean_new_bot_name: &str) -> String;
    /// `/bot rename`：无法获取父目录。
    fn bot_rename_no_parent_dir(&self) -> &'static str;
    /// `/bot rename`：目录重命名失败。
    fn bot_rename_failed(&self, reason: &str) -> String;
    /// `/bot rename`：目录已重命名。
    fn bot_rename_succeeded(&self, old_bot_name: &str, new_bot_name: &str) -> String;
    /// `/token`：写入 Token 文件失败。
    fn bot_token_write_failed(&self, raw_bot_name: &str, reason: &str) -> String;
    /// `/token`：Token 已更新。
    fn bot_token_updated(&self, raw_bot_name: &str) -> String;
    /// `/bot login`：包含验证网址与设备码的首次回复。
    fn bot_login_prompt(
        &self,
        bot_name: &str,
        verification_url: &str,
        user_code: &str,
        minutes: u64,
    ) -> String;
    /// `/bot login`：写入程序日志的安全摘要，不包含设备码或验证网址。
    fn bot_login_prompt_logged(&self, bot_name: &str) -> String;
    /// `/bot login`：凭据已写入。
    fn bot_login_succeeded(&self, bot_name: &str) -> String;
    /// `/bot login`：无凭据且登录会话已存在。
    fn bot_login_already_running(&self, bot_name: &str) -> String;
    /// `/bot login`：授权被微软拒绝。
    fn bot_login_rejected(&self, bot_name: &str) -> String;
    /// `/bot login`：会话已启动，但未收到设备码。
    fn bot_login_prompt_missing(&self, bot_name: &str) -> String;
    /// `/bot login`：等待凭据超时。
    fn bot_login_timed_out(&self, bot_name: &str, waited_minutes: u64) -> String;

    // =================== 批量指令（`operations/*`） ===================
    /// 计数行标签：已配置 Token 的账号数。
    fn count_valid_accounts(&self) -> &'static str;
    /// 计数行标签：启动后会话存在且状态查询成功。
    fn count_start_verified(&self) -> &'static str;
    /// 计数行标签：跳过（会话已存在）。
    fn count_skipped_running(&self) -> &'static str;
    /// 计数行标签：跳过（缺少 Token）。
    fn count_skipped_missing_token(&self) -> &'static str;
    /// `/startall` 中已取消的启动请求计数标签。
    fn count_start_cancelled(&self) -> &'static str;
    /// 计数行标签：启动失败或超时。
    fn count_start_failed(&self) -> &'static str;
    /// 计数行标签：目标机器人。
    fn count_running_accounts(&self) -> &'static str;
    /// 计数行标签：重启后会话存在且状态查询成功。
    fn count_restart_verified(&self) -> &'static str;
    /// 计数行标签：重启失败或超时。
    fn count_restart_failed(&self) -> &'static str;
    /// 计数行标签：成功发送。
    fn count_sent_ok(&self) -> &'static str;
    /// 计数行标签：发送失败。
    fn count_send_failed(&self) -> &'static str;
    /// 批量暂停或恢复的处理计数。
    fn count_processed_successfully(&self) -> &'static str;
    /// 批量暂停或恢复的发送或确认失败计数。
    fn count_processing_failed(&self) -> &'static str;
    /// `/startall`：无可启动账号。
    fn startall_no_valid_accounts(&self) -> &'static str;
    /// `/startall`：无可启动账号，且存在缺少 Token 的账号。
    fn startall_no_valid_accounts_with_missing(&self, missing_token_count: usize) -> String;
    /// `/startall`：有效账号均已运行。
    fn startall_all_running(&self) -> &'static str;
    /// `/startall` 的报告标题。
    fn startall_title(&self) -> &'static str;
    /// `/startall` 的跳过明细：已在运行。
    fn startall_skipped_running(&self, bot_name: &str) -> String;
    /// `/stopall` 的报告标题。
    fn stopall_title(&self) -> &'static str;
    /// `/restartall`：无运行中的实例。
    fn restartall_nothing_running(&self) -> &'static str;
    /// `/restartall` 的报告标题。
    fn restartall_title(&self) -> &'static str;
    /// 批量暂停和恢复的报告标题。
    fn pauseall_title(&self) -> &'static str;
    /// 批量恢复钓鱼的报告标题。
    fn unpauseall_title(&self) -> &'static str;
    /// `/commandall` 的报告标题。
    fn commandall_title(&self) -> &'static str;
    /// `/commandall`：显示待发送的指令。
    fn commandall_sending(&self, bot_command: &str) -> String;

    /// 反 AFK 已启动（不代表输入已送达）。
    fn anti_afk_started(&self) -> &'static str;
    /// 反 AFK 已在运行，未重复启动。
    fn anti_afk_already_running(&self) -> &'static str;
    /// 反 AFK 已停止。
    fn anti_afk_stopped(&self) -> &'static str;
    /// `/show status` 中的反 AFK 线程状态。
    fn anti_afk_status(&self, running: bool) -> &'static str;
    /// 反 AFK 未运行，无需停止。
    fn anti_afk_stop_not_running(&self) -> &'static str;
    /// 反 AFK 状态锁访问失败。
    fn anti_afk_lock_failed(&self, reason: &str) -> String;
    /// 反 AFK 配置构造失败。
    fn anti_afk_config_failed(&self, reason: &str) -> String;
    /// 反 AFK 线程运行失败。
    fn anti_afk_worker_failed(&self, reason: &str) -> String;
    /// 反 AFK 启动失败。
    fn anti_afk_start_failed(&self, reason: &str) -> String;
    /// 反 AFK 停止操作失败。
    fn anti_afk_stop_failed(&self, reason: &str) -> String;
    // ==================== 只读查询（`queries/show_*`） ====================
    /// `/show status` 的报告标题。
    fn status_title(&self) -> &'static str;
    /// `/show status`：无运行中的机器人。
    fn status_no_running_bots(&self) -> &'static str;
    /// `/show status` 表头第 1 列：机器人名（列序见 `docs/mods/commands/commands-zh_cn.md` 的 6.2 节）。
    fn status_column_bot_name(&self) -> &'static str;
    /// `/show status` 表头第 2 列：进服状态。
    fn status_column_joined(&self) -> &'static str;
    /// `/show status` 表头第 3 列：暂停状态。
    fn status_column_pause(&self) -> &'static str;
    /// `/show status` 表头：没有可用鱼竿。
    fn status_column_no_rod(&self) -> &'static str;
    /// `/show status` 表头：存在鱼漂。
    fn status_column_bobber(&self) -> &'static str;
    /// `/show status` 表头：上次钓鱼动作的 Unix 毫秒时间戳。
    fn status_column_last_fish(&self) -> &'static str;
    /// `/show status` 表头：运行时长。
    fn status_column_uptime(&self) -> &'static str;
    /// 通用计数行：`共 N 个`（`/show status`、`/show mybots`、`/show admins` 共用）。
    fn common_total_count(&self, count: usize) -> String;
    /// `/show mybots` 的报告标题。
    fn mybots_title(&self, caller_id: u64) -> String;
    /// `/show mybots`：无机器人。
    fn mybots_empty(&self) -> &'static str;
    /// `/show mybots` 的一行。
    fn mybots_row(&self, bot_name: &str, token_status: &str) -> String;
    /// `/show allbots` 的报告标题。
    fn allbots_title(&self) -> &'static str;
    /// `/show allbots`：无机器人。
    fn allbots_empty(&self) -> &'static str;
    /// `/show allbots`：已配 Token 的数量。
    fn allbots_ready_token_count(&self, ready_token_count: usize) -> String;
    /// `/show allbots`：缺 Token 的数量。
    fn allbots_missing_token_count(&self, missing_token_count: usize) -> String;
    /// `/show allbots` 的一行。
    fn allbots_row(&self, bot_name: &str, owner_id: u64, token_status: &str) -> String;
    /// `/show admins` 的报告标题。
    fn admins_title(&self) -> &'static str;
    /// `/show admins`：未配置管理员。
    fn admins_empty(&self) -> &'static str;
    /// `/show admins` 的一行。
    fn admins_row(&self, name: &str, user_id: &str) -> String;
    /// `/show botconfig`：读配置文件失败。
    fn botconfig_read_failed(&self, path: &str, reason: &str) -> String;
    /// `/show botconfig`：文件超过附件上限。
    fn botconfig_too_large(&self, bytes: usize, max_bytes: usize) -> String;

    // ==================== 通用指令（`queries/`） ====================
    /// `/help` 的报告标题（正文来自配置的 `text.help`）。
    fn help_title(&self) -> &'static str;
    /// `/ping` 的报告标题。
    fn ping_title(&self, host: &str, port: u16) -> String;
    /// `/ping`：在线。
    fn ping_status_online(&self) -> &'static str;
    /// `/ping`：人数。
    fn ping_players(&self, online_players: usize, max_players: usize) -> String;
    /// `/ping`：延迟（含"这一跳"的说明）。
    fn ping_latency(&self, latency_ms: u128) -> String;
    /// `/ping`：连接失败。
    fn ping_connect_failed(&self, host: &str, port: u16, reason: &str) -> String;
    /// 域名解析没有返回地址。
    fn ping_no_address_reason(&self) -> &'static str;
    /// 建连失败时没有具体错误的原因。
    fn ping_connection_failed_reason(&self) -> &'static str;
    /// `/ping`：连接超时。
    fn ping_timeout(&self, host: &str, port: u16, timeout: Duration) -> String;
    /// `/ping`：用户指定的目标是本地或私网地址。
    fn ping_local_target(&self, host: &str, port: u16) -> String;
    /// `/ping`：协议解析失败。
    fn ping_protocol_failed(&self, host: &str, port: u16, reason: &str) -> String;
    /// `/checkserver` 的报告标题。
    fn checkserver_title(&self) -> &'static str;
    /// `/checkserver`：系统信息初始化失败。
    fn checkserver_system_failed(&self, reason: &str) -> String;
    /// `/checkserver`：磁盘信息查询失败。
    fn checkserver_disks_failed(&self, reason: &str) -> String;
    /// `/checkserver`：系统运行时间查询失败。
    fn checkserver_uptime_failed(&self, reason: &str) -> String;
    /// `/checkserver`：CPU 占用率。
    fn checkserver_cpu_usage(&self, cpu_usage: f32) -> String;
    /// `/checkserver`：已用内存与总内存（MB；显示时按总量自动换成 GB）。
    fn checkserver_memory_used_total(&self, used_mem_mb: u64, total_mem_mb: u64) -> String;
    /// `/checkserver`：内存占用率。
    fn checkserver_memory_usage(&self, mem_usage_pct: f64) -> String;
    /// `/checkserver`：磁盘占用率。
    fn checkserver_disk_usage(&self, disk_usage_pct: f64) -> String;
    /// `/checkserver`：系统已运行时长（`uptime_text` 是已格式化好的运行时长文本）。
    fn checkserver_uptime(&self, uptime_text: &str) -> String;
    /// `/checkserver`：本次运行时长（`uptime_text` 是已格式化好的运行时长文本）。
    fn checkserver_process_uptime(&self, uptime_text: &str) -> String;
    /// `/checkserver`：当前系统时间与时区（`time_text` 是已格式化好的时间文本）。
    fn checkserver_system_time(&self, time_text: &str) -> String;

    // ================= 命令交互与回复文案 =================
    /// 手动同步指令的结果标题。
    fn command_sync_title(&self) -> &'static str;
    /// 全局指令已注册及传播风险。
    fn command_sync_registered(&self) -> &'static str;
    /// 已检查服务器数及清理结果。
    fn command_sync_summary(&self, guilds: usize, cleared: usize, failed: usize) -> String;
    /// 指定服务器已更新。
    fn command_sync_guild_updated(&self, guild_name: &str) -> String;
    /// 指定服务器注册失败。
    fn command_sync_guild_failed(&self, guild_name: &str, reason: &str) -> String;
    /// 指定服务器的同步入口创建失败。
    fn command_sync_bootstrap_failed(&self, guild_name: &str, reason: &str) -> String;
    /// 服务器模式下的结果计数。
    fn command_sync_guild_summary(&self, guilds: usize, updated: usize, failures: usize) -> String;
    /// 同步未完成时保留旧命令。
    fn command_sync_cleanup_skipped(&self) -> &'static str;
    /// 已清理旧全局指令。
    fn command_sync_global_cleared(&self, count: usize) -> String;
    /// 清理旧全局指令失败。
    fn command_sync_global_clear_failed(&self, reason: &str) -> String;
    /// 全局指令注册失败，不执行清理。
    fn command_sync_failed(&self, reason: &str) -> String;
    /// 已有同步操作正在执行。
    fn command_sync_in_progress(&self) -> &'static str;
    /// 非启动长耗时指令的第一条进度消息；`command_tag` 形如 `/stopall`。
    fn command_started_notice(&self, command_tag: &str) -> String;
    /// 命令内部发生意外错误时返回给用户的通用提示，不包含内部错误细节。
    fn command_unexpected_failure(&self) -> &'static str;
    /// 启动请求的第一条消息：明确说明请求已进入全局启动队列。
    fn command_start_queued_notice(&self, command_tag: &str) -> String;
    /// `/startall` 的进度回执：启动数量、排队数量、预计耗时与结果超时（分钟）。
    fn command_startall_queued_notice(
        &self,
        bot_count: usize,
        queued_bot_count: usize,
        estimated_minutes: u64,
        timeout_minutes: u64,
    ) -> String;
    /// 单机器人启动发生 panic 时的失败原因。
    fn start_internal_error_reason(&self) -> &'static str;
    /// 启动队列已满时的失败原因。
    fn start_queue_full_reason(&self) -> &'static str;
    /// `/startall` 报告中的批次大小、批内间隔与批间间隔。
    fn command_start_queue_summary(
        &self,
        batch_size: usize,
        bot_login_interval: Duration,
        batch_login_interval: Duration,
    ) -> String;
    /// `/log`：附件消息正文。
    fn command_log_attachment_body(
        &self,
        bot_name: &str,
        file_name: &str,
        line_count: usize,
    ) -> String;
    /// `/log`：附件因大小限制截断时的说明。
    fn command_log_dropped_note(&self, dropped_line_count: usize) -> String;
    /// `/show botconfig`：附件消息正文。
    fn command_botconfig_attachment_body(&self, file_name: &str) -> String;
    /// `/log`：行数校验失败时的用法提示。
    fn command_log_lines_usage(&self) -> &'static str;
    /// `/ping`：端口校验失败时的用法提示。
    fn command_ping_port_usage(&self) -> &'static str;
    /// `/whoami`：用户 ID 与 `[admins]` 配置示例。
    fn command_whoami(&self, caller_id: u64) -> String;

    // ============== 权限与共享的拼接片段 ==============
    /// 计数行的统一格式（`Report::set_counts` 用）：`标签: N 个`。
    fn count_line(&self, label: &str, count: usize) -> String;
    /// 非管理员执行批量指令时的拒绝文案。
    fn admin_required(&self, caller_id: u64) -> String;
    /// Token 文件状态的展示：已就绪。
    fn token_status_ready(&self) -> &'static str;
    /// Token 文件状态：缺失。
    fn token_status_missing(&self) -> &'static str;
    /// 找不到机器人目录（或无权访问），刻意不区分这两种情况。
    fn bot_dir_not_found(&self, clean_bot_name: &str) -> String;
    /// `/bot add`：目标名称已存在。
    fn bot_add_name_taken(&self, bot_name: &str) -> String;
    /// `/bot add`：建目录失败。
    fn create_bot_dir_failed(&self, reason: &str) -> String;
    /// 读某个用户的机器人列表失败。
    fn read_bot_list_failed(&self, reason: &str) -> String;
    /// 读机器人根目录失败。
    fn read_bots_root_failed(&self, reason: &str) -> String;
    /// 无法查询 screen 会话。
    fn screen_list_failed(&self, reason: &str) -> String;
    /// screen 输出无法识别。
    fn screen_unrecognized_output(&self, output: &str) -> String;
    /// 机器人运行时拒绝操作。
    fn refusal_bot_running(&self, bot_name: &str) -> String;
    /// 机器人排队启动时拒绝操作。
    fn refusal_bot_queued(&self, bot_name: &str) -> String;
    /// 会话指令注入失败；`reason` 为 `None` 表示 screen 未提供失败原因。
    fn send_command_failed(&self, bot_name: &str, reason: Option<&str>) -> String;
    /// 发送停止信号（Ctrl+C）失败。
    fn stop_signal_failed(&self, reason: &str) -> String;
    /// 强制终止会话失败。
    fn force_quit_failed(&self, reason: &str) -> String;

    // ============ 启动 / 停止流程的明细行（`runtime/bot_startup`、`runtime/bot_instances/stop.rs`） ============
    /// 计数行标签：扫描机器人目录。
    fn count_scanned_dirs(&self) -> &'static str;
    /// 计数行标签：成功停止。
    fn count_stopped(&self) -> &'static str;
    /// 计数行标签：停止失败。
    fn count_stop_failed(&self) -> &'static str;
    /// 停止成功明细。
    fn stop_succeeded_line(&self, bot_name: &str) -> String;
    /// 停止失败明细：收尾后会话仍存在。
    fn stop_still_running(&self, bot_name: &str) -> String;
    /// 启动失败明细：无法创建日志文件。
    fn start_log_file_failed(&self, bot_name: &str) -> String;
    /// 启动复核明细：状态查询确认已进服。
    fn start_join_confirmed(&self, bot_name: &str) -> String;
    /// 启动复核明细：状态查询确认当前未进服。
    fn start_not_joined(&self, bot_name: &str) -> String;
    /// 启动失败明细：复核时会话不存在。
    fn start_failed(&self, bot_name: &str) -> String;
    /// 启动请求在执行前被取消。
    fn start_cancelled(&self, bot_name: &str) -> String;
    /// 等待启动结果超时，状态标记为未确认。
    fn start_result_timeout(&self, bot_name: &str, timeout: Duration) -> String;
    /// 重启失败明细：缺 Token，已保持原状。
    fn restart_skipped_missing_token(&self, bot_name: &str) -> String;
    /// `/restart` 被拒：目标当前不在运行（要启动请用 `/start`）。
    fn restart_not_running(&self, bot_name: &str) -> String;
    /// autocomplete 候选的运行中状态后缀。
    fn bot_candidate_running_note(&self) -> &'static str;
    /// autocomplete 候选的显示后缀：未运行。
    fn bot_candidate_stopped_note(&self) -> &'static str;
    /// autocomplete 候选的排队状态后缀。
    fn bot_candidate_queued_note(&self) -> &'static str;
    /// 重启失败明细：会话未停止，跳过启动。
    fn restart_skipped_session_alive(&self, bot_name: &str) -> String;
    /// screen 会话启动失败。
    fn start_launch_failed(&self, bot_name: &str, reason: &str) -> String;
    /// 通用明细行：`机器人 X: 说明`。
    fn bot_line(&self, bot_name: &str, message: &str) -> String;

    // ======== 指令元数据（注册到 Discord 的说明文字，跟随 `[i18n] language`） ========
    /// 全部指令与子指令的说明（表结构见 [`CommandText`]）。
    ///
    /// 注册前按配置语言填充并校验。
    fn command_texts(&self) -> &'static [CommandText];
    /// `/log` 的行数选项说明，包含实际使用的默认值。
    fn command_log_lines_description(&self, default_lines: usize) -> String;
    /// 指令说明表不完整或无效时的诊断。
    fn command_text_invalid(&self, path: &str, problem: CommandTextProblem) -> String;

    /// `/token` 弹窗的标题。
    fn token_modal_title(&self) -> &'static str;
    /// `/token` 弹窗里"机器人名"字段的标签。
    fn token_modal_bot_name_label(&self) -> &'static str;
    /// `/token` 弹窗里"机器人名"字段的占位提示。
    fn token_modal_bot_name_placeholder(&self) -> &'static str;
    /// `/token` 弹窗里"refreshToken"字段的标签。
    fn token_modal_token_label(&self) -> &'static str;
    /// `/token` 弹窗里"refreshToken"字段的占位提示。
    fn token_modal_token_placeholder(&self) -> &'static str;
}

/// 指令及其选项的本地化说明，注册时与指令树逐项校验。
#[derive(Debug, Clone, Copy)]
pub struct CommandText {
    /// 指令路径，例如 `"start"` 或 `"bot add"`。
    pub name_path: &'static str,
    /// Discord 指令列表中的说明。
    pub description: &'static str,
    /// 选项名称及说明；无选项时为空。
    pub options: &'static [(&'static str, &'static str)],
}

/// 指令说明表与指令定义不一致，或说明长度不合法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTextProblem {
    Missing,
    Duplicate,
    Unknown,
    InvalidLength { length: usize, max: usize },
}

/// 返回全部语言实现，供覆盖测试使用。
pub fn all_languages() -> &'static [&'static dyn Text] {
    static ALL_LANGUAGES: [&dyn Text; 2] = [&zh_cn::ZH_CN, &en_us::EN_US];

    &ALL_LANGUAGES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 英文文案实现应返回英文，而不是另一种语言的字面量。
    #[test]
    fn english_implementation_uses_english_text() {
        let english = &en_us::EN_US;

        assert_eq!(english.count_line("stopped", 2), "stopped: 2");
        assert_eq!(english.report_succeeded_label(), "[Completed]");
        assert!(
            english
                .bot_stop_not_running("duck")
                .contains("no screen session")
        );
    }

    #[test]
    fn every_watchdog_text_is_present_and_localized() {
        let (zh_cn, en_us) = (all_languages()[0], all_languages()[1]);
        for message in WatchdogText::ALL {
            let chinese = zh_cn.watchdog_text(message);
            let english = en_us.watchdog_text(message);
            assert_ne!(
                chinese, english,
                "watchdog text was not localized: {message:?}"
            );
            assert!(chinese.contains("锁中毒"));
            assert!(english.contains("mutex poisoned"));
        }
    }
}
