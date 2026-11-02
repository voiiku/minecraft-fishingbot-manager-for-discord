//! 文案按 `[i18n] language` 选择，每个进程使用一种语言。
//! 回复、日志与状态文案由 [`Text`] 提供；引导期提示固定使用英文。
//! 看门狗锁中毒的诊断文案由 `watchdog` 子模块提供，随 `[i18n] language` 切换。
//! JAR 日志中的固定关键词也随语言选择，各部署自定义的关键词仍放在配置中。

pub mod bootstrap;
mod en_us;
mod log_text;
mod requirement;
mod setup_failure;
mod watchdog;
mod zh_cn;

pub use log_text::LogText;
pub use requirement::ConfigRequirement;
pub use setup_failure::{failure_is_permanent, setup_failure_log_text};
pub use watchdog::WatchdogText;

use std::sync::OnceLock;
use std::time::Duration;

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
    fn implementation(self) -> &'static dyn Text {
        match self {
            Self::ChineseSimplified => &zh_cn::ZH_CN,
            Self::English => &en_us::EN_US,
        }
    }
}

/// 面向 Discord 的文案接口。
pub trait Text: Send + Sync {
    // ========================== 报告（`utils/reply.rs`） ==========================
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

    // ===================== 看门狗诊断文案（`text/watchdog.rs`） =====================
    /// 看门狗互斥锁中毒时的诊断信息；随 `[i18n] language` 切换。
    fn watchdog_text(&self, message: WatchdogText) -> &'static str;

    // ===================== 机器人状态（`utils/bot_status.rs`） =====================
    /// 机器人日志文件不存在。
    fn bot_status_log_not_found(&self, bot_name: &str) -> String;
    /// 机器人日志文件无法读取。
    fn bot_status_unreadable_log(&self, bot_name: &str) -> String;
    /// 机器人日志文件为空。
    fn bot_status_empty_log(&self, bot_name: &str) -> String;
    /// 日志尚未出现状态关键词。
    fn bot_status_initializing(&self) -> &'static str;
    /// 已经进入游戏服务器。
    fn bot_status_online(&self) -> &'static str;
    /// 进程存活，但机器人正在重连。
    fn bot_status_reconnecting(&self) -> &'static str;
    /// 进程已结束或被手动中止。
    fn bot_status_stopped(&self) -> &'static str;
    /// 正在钓鱼。
    fn bot_pause_fishing(&self) -> &'static str;
    /// 钓鱼已暂停，机器人仍在线。
    fn bot_pause_paused(&self) -> &'static str;

    // ========== FishingBot.jar 固定文案（`utils/bot_status.rs`）==========
    // 重连提示由 JAR 的语言资源决定；服务器关键词和可能因构建而异的指令回显保留在配置中。
    /// JAR 断线自动重连提示的前缀。
    fn jar_reconnect_prefix(&self) -> &'static str;
    /// 重连提示的后缀；须与前缀同时匹配。
    fn jar_reconnect_suffix(&self) -> &'static str;

    // ===================== 机器人名（`utils/bot_name.rs`） =====================
    /// 名字为空（或只有空白）。
    fn bot_name_empty(&self) -> &'static str;
    /// 名字含非法字符。
    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String;

    // =================== 单机器人指令（`handlers/bot_control/*`） ===================
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
    /// `/restart`：旧版通用回复。
    fn bot_restart_command_sent(&self, bot_name: &str) -> String;
    /// `/restart`：重启后已确认进服。
    fn bot_restart_online_confirmed(&self, bot_name: &str) -> String {
        self.start_command_sent_confirmed(bot_name)
    }
    /// `/restart`：新会话存在，但进服状态未确认。
    fn bot_restart_session_unconfirmed(&self, bot_name: &str) -> String {
        self.start_command_sent_unconfirmed(bot_name)
    }
    /// `/command`：指令为空。
    fn bot_command_empty(&self) -> &'static str;
    /// `/command`：screen 会话不存在。
    fn bot_command_not_running(&self, bot_name: &str) -> String;
    /// `/command`：指令已注入。
    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String;
    /// `/pause`：已通过日志回显确认暂停。
    fn bot_pause_confirmed(&self, bot_name: &str) -> String;
    /// `/unpause`：已通过日志回显确认恢复。
    fn bot_unpause_confirmed(&self, bot_name: &str) -> String;
    /// 暂停或恢复指令已发送，但未确认状态。
    fn bot_pause_unconfirmed(&self, bot_name: &str, bot_command: &str) -> String;
    /// `/pause`、`/unpause`：指令已被并发的更新请求取代。
    fn bot_pause_superseded(&self, my_command: &str, current_pause_status: &str) -> String;
    /// `/log`：行数必须大于 0。
    fn bot_log_zero_lines(&self) -> &'static str;
    /// `/log`：日志文件或文件名不可用。
    fn bot_log_missing(&self, bot_name: &str) -> String;

    // =================== 账号管理指令（`handlers/bot_manage/*`） ===================
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
    fn bot_login_prompt(&self, bot_name: &str, verification_url: &str, user_code: &str) -> String;
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

    // =================== 批量指令（`handlers/all_bots_control/*`） ===================
    /// 计数行标签：已配置 Token 的账号数。
    fn count_valid_accounts(&self) -> &'static str;
    /// 计数行标签：启动后会话存在。
    fn count_start_command_sent(&self) -> &'static str;
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
    /// 计数行标签：重启后会话存在。
    fn count_restart_command_sent(&self) -> &'static str;
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
    /// 反 AFK 状态查询失败。
    fn anti_afk_status_failed(&self, reason: &str) -> String;

    // ==================== 只读查询（`show/*`） ====================
    /// `/show status` 的报告标题。
    fn status_title(&self) -> &'static str;
    /// `/show status`：无运行中的机器人。
    fn status_no_running_bots(&self) -> &'static str;
    /// `/show status` 表头第 1 列：机器人名（列序见 `docs/mods/commands/commands-zh_cn.md` 的 6.2 节）。
    fn status_column_bot_name(&self) -> &'static str;
    /// `/show status` 表头第 2 列：服务器状态。
    fn status_column_connection(&self) -> &'static str;
    /// `/show status` 表头第 3 列：暂停状态。
    fn status_column_pause(&self) -> &'static str;
    /// `/show status` 表头第 4 列：运行时长。
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

    // ==================== 通用指令（`general/*`） ====================
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

    // ================= 命令层文案（`discord_commands/*`） =================
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

    // ============== 权限与共享的拼接片段（`utils/*`） ==============
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

    // ============ 启动 / 停止流程的明细行（`utils/startup|shutdown`） ============
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
    /// 启动成功明细：会话存活满宽限期，且日志已确认进服。
    fn start_command_sent_confirmed(&self, bot_name: &str) -> String;
    /// 启动成功明细：会话存活满宽限期，但日志尚未确认进服状态。
    fn start_command_sent_unconfirmed(&self, bot_name: &str) -> String;
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

    // ======== 日志、启动期控制台与配置报错（与 Discord 文案共用同一套语言） ========
    /// 渲染日志或控制台文案；日志级别由调用方决定。
    fn log_text(&self, message: LogText<'_>) -> String;
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
    InvalidLength { length: usize },
}

/// 返回全部语言实现，供覆盖测试使用；业务代码使用 [`crate::text::get`]。
pub fn all_languages() -> &'static [&'static dyn Text] {
    static ALL_LANGUAGES: [&dyn Text; 2] = [&zh_cn::ZH_CN, &en_us::EN_US];

    &ALL_LANGUAGES
}

/// 返回已安装语言的文案；生产构建未安装时 panic，单元测试默认使用英语。
pub fn get() -> &'static dyn Text {
    match INSTALLED.get() {
        Some(i18n_text) => *i18n_text,
        None => uninstalled_language(),
    }
}

/// 未安装语言时的实现：生产构建 panic，测试构建返回英语。
#[cfg(not(test))]
fn uninstalled_language() -> &'static dyn Text {
    panic!("{}", bootstrap::TEXT_NOT_INSTALLED)
}

/// 未安装语言时的实现：生产构建 panic，测试构建返回英语。
#[cfg(test)]
fn uninstalled_language() -> &'static dyn Text {
    &en_us::EN_US
}

/// 安装语言；进程内仅首次调用生效。
pub fn install(language: Language) {
    let _ = INSTALLED.set(language.implementation());
}

/// 已安装的语言实现。
static INSTALLED: OnceLock<&'static dyn Text> = OnceLock::new();

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

    /// JAR 重连提示在中英文实现中都存在，且两种语言的值不同。
    #[test]
    fn jar_reconnect_messages_are_defined_for_both_languages() {
        assert_eq!(zh_cn::ZH_CN.jar_reconnect_prefix(), "钓鱼机器人将在");
        assert_eq!(zh_cn::ZH_CN.jar_reconnect_suffix(), "秒内重新连接");
        assert_eq!(
            en_us::EN_US.jar_reconnect_prefix(),
            "FishingBot will reconnect in"
        );
        assert_eq!(en_us::EN_US.jar_reconnect_suffix(), " seconds");

        assert_ne!(
            zh_cn::ZH_CN.jar_reconnect_prefix(),
            en_us::EN_US.jar_reconnect_prefix()
        );
        assert_ne!(
            zh_cn::ZH_CN.jar_reconnect_suffix(),
            en_us::EN_US.jar_reconnect_suffix()
        );
    }
}
