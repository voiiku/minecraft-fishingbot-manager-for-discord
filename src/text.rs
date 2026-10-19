//! 文案按 `[i18n] language` 选择，每个进程使用一种语言。
//! 回复、日志与状态文案由 [`Text`] 提供；引导期提示固定使用英文。
//! JAR 日志中的固定关键词也随语言选择，各部署自定义的关键词仍放在配置中。

pub mod bootstrap;
mod en_us;
mod log_text;
mod requirement;
mod setup_failure;
mod zh_cn;

// 统一从入口导出文案数据类型。
pub use log_text::LogText;
pub use requirement::ConfigRequirement;
// 导出 Discord 装配失败分类函数。
pub use setup_failure::{failure_is_permanent, setup_failure_log_text};

use std::sync::OnceLock;

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

    /// 返回 FishingBot `conf.json` 中使用的语言名。
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
    /// "成功明细"小节标题。
    fn report_succeeded_label(&self) -> &'static str;
    /// "跳过明细"小节标题。
    fn report_skipped_label(&self) -> &'static str;
    /// "排队中"小节标题（`/show status` 报告下方那一栏：还在启动队列里排着的机器人）。
    fn report_queued_label(&self) -> &'static str;
    /// "失败明细"小节标题。
    fn report_failed_label(&self) -> &'static str;
    /// 整块内容的标签（原文里是 `【内容】:`）。
    fn report_content_label(&self) -> &'static str;
    /// 按行截断时的说明，`dropped_line_count` 是被省略的行数。
    fn report_omitted_lines_note(&self, dropped_line_count: usize) -> String;

    // ===================== 机器人状态（`utils/bot_status.rs`） =====================
    /// 报错：挑不到这个机器人的日志文件（正常运行时不会出现：日志由启动流程建好）。
    fn bot_status_log_not_found(&self, bot_name: &str) -> String;
    /// 报错：这个机器人的日志文件读不出来。
    fn bot_status_unreadable_log(&self, bot_name: &str) -> String;
    /// 报错：这个机器人的日志文件是空的。
    fn bot_status_empty_log(&self, bot_name: &str) -> String;
    /// 日志里还没有决定性关键词（刚启动）。
    fn bot_status_initializing(&self) -> &'static str;
    /// 已经进入游戏服务器。
    fn bot_status_online(&self) -> &'static str;
    /// 正在重连：进程还在，但此刻本人不在线。
    fn bot_status_reconnecting(&self) -> &'static str;
    /// 进程已结束或被手动中止。
    fn bot_status_stopped(&self) -> &'static str;
    /// 正在钓鱼。
    fn bot_pause_fishing(&self) -> &'static str;
    /// 已暂停钓鱼（但没下线）。
    fn bot_pause_paused(&self) -> &'static str;

    // ========== FishingBot.jar 自己打印的固定文案（`utils/bot_status.rs`）==========
    // 用于在机器人日志中识别"重连了"，不是本项目发往 Discord 的文案。
    // 这两句由 JAR 自己的语言资源决定（各部署都一样），所以不放进配置、不放 `[keywords]`：
    // `[i18n] language` 的取值会原样写进生成出来的 conf.json（`misc.language`），JAR 就按它说话，
    // 两边是同一个值，不会对不上。
    // 与游戏服务器 / 本机环境有关的关键词（进服提示、结束标记、刷屏噪声）各部署不同，
    // 仍留在配置的 `[keywords]` 里。
    // `/pause` 与 `/unpause` 的回显同属 JAR 打印，但放在配置的 `[command_echo]`：不同 JAR 构建可能打印
    // 不同回显，就地改比重新编一份语言表更实际。
    /// JAR 断线自动重连提示的前缀（`bot-automatic-reconnect` 那句的开头）。
    fn jar_reconnect_prefix(&self) -> &'static str;
    /// 同一句的后缀：须与前缀同时命中，避免日志中偶然出现的半句话被判为状态。
    fn jar_reconnect_suffix(&self) -> &'static str;

    // ===================== 机器人名（`utils/bot_name.rs`） =====================
    /// 名字为空（或只有空白）。
    fn bot_name_empty(&self) -> &'static str;
    /// 名字含非法字符。
    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String;

    // =================== 单机器人指令（`bot_control/*`） ===================
    /// `/start`：没有 Token 文件，无法启动。
    fn bot_start_missing_token(&self, bot_name: &str) -> String;
    /// `/start`：已经在跑。
    fn bot_start_already_running(&self, bot_name: &str) -> String;
    /// 机器人已经在启动队列里排队（`/start`、`/bot login` 用它提前拒绝）。
    fn bot_start_already_queued(&self, bot_name: &str) -> String;
    /// `/stop`：本来就没在跑。
    fn bot_stop_not_running(&self, bot_name: &str) -> String;
    /// `/stop`：停止流程当场复核过，可以说"已成功停止"。
    fn bot_stop_succeeded(&self, bot_name: &str) -> String;
    /// 机器人还没被拉起、只取消了它的排队启动（`/stop`、`/stopall` 用）。
    fn bot_stop_cancelled_queue(&self, bot_name: &str) -> String;
    /// `/restart`：命令已发送（停止那半已确认）。
    fn bot_restart_command_sent(&self, bot_name: &str) -> String;
    /// `/command`：要发的指令是空的。
    fn bot_command_empty(&self) -> &'static str;
    /// `/command`：机器人没在跑，没东西可注入。
    fn bot_command_not_running(&self, bot_name: &str) -> String;
    /// `/command`：指令已注入。
    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String;
    /// `/pause`：注入之后在日志末尾确认到了 JAR 的回显，才敢说"已暂停"。
    fn bot_pause_confirmed(&self, bot_name: &str) -> String;
    /// `/unpause`：同上（确认到回显才说"已恢复"）。
    fn bot_unpause_confirmed(&self, bot_name: &str) -> String;
    /// `/pause`、`/unpause`：发出的那条不是最新的（同一台机器人被同时下了相反指令），只说命令与当前状态。
    fn bot_pause_superseded(&self, my_command: &str, current_pause_status: &str) -> String;
    /// `/log`：行数必须大于 0。
    fn bot_log_zero_lines(&self) -> &'static str;
    /// `/log`：这个机器人还没有日志文件（或取不到日志文件名）。
    fn bot_log_missing(&self, bot_name: &str) -> String;

    // =================== 账号管理指令（`bot_manage/*`） ===================
    /// `/bot add`：目录建出来了。
    fn bot_add_succeeded(&self, bot_name: &str) -> String;
    /// `/bot add`：建目录失败（`reason` 是下层给的原文）。
    fn bot_add_failed(&self, raw_bot_name: &str, reason: &str) -> String;
    /// `/bot del`：删目录失败。
    fn bot_delete_failed(&self, bot_name: &str, reason: &str) -> String;
    /// `/bot del`：删掉了。
    fn bot_delete_succeeded(&self, bot_name: &str) -> String;
    /// `/bot rename`：新旧名字一样。
    fn bot_rename_same_name(&self) -> &'static str;
    /// `/bot rename`：新名字在磁盘上已经被占了。
    fn bot_rename_name_taken(&self, clean_new_bot_name: &str) -> String;
    /// `/bot rename`：取不到父目录（理论上不该发生）。
    fn bot_rename_no_parent_dir(&self) -> &'static str;
    /// `/bot rename`：改目录名失败。
    fn bot_rename_failed(&self, reason: &str) -> String;
    /// `/bot rename`：改好了。
    fn bot_rename_succeeded(&self, old_bot_name: &str, new_bot_name: &str) -> String;
    /// `/token`：写 Token 文件失败。
    fn bot_token_write_failed(&self, raw_bot_name: &str, reason: &str) -> String;
    /// `/token`：写好了。
    fn bot_token_updated(&self, raw_bot_name: &str) -> String;
    /// `/bot login`：第一条回复——把 JAR 给的登录网址与设备码告诉调用者。
    fn bot_login_prompt(&self, bot_name: &str, verification_url: &str, user_code: &str) -> String;
    /// `/bot login`：第二条回复——凭据已经落盘。
    fn bot_login_succeeded(&self, bot_name: &str) -> String;
    /// `/bot login`：会话已经在跑，而它还没有凭据（多半就是上一次登录还没结束）。
    fn bot_login_already_running(&self, bot_name: &str) -> String;
    /// `/bot login`：微软拒绝了这次授权（凭据没建出来）。
    fn bot_login_rejected(&self, bot_name: &str) -> String;
    /// `/bot login`：会话起来了，但没等到设备码提示。
    fn bot_login_prompt_missing(&self, bot_name: &str) -> String;
    /// `/bot login`：等凭据超时（已等 `waited_minutes` 分钟，会话已停）。
    fn bot_login_timed_out(&self, bot_name: &str, waited_minutes: u64) -> String;

    // =================== 批量指令（`all_bots_control/*`） ===================
    /// 计数行标签：有效账号总数。
    fn count_valid_accounts(&self) -> &'static str;
    /// 计数行标签：已发送启动命令。
    fn count_start_command_sent(&self) -> &'static str;
    /// 计数行标签：跳过（已在运行）。
    fn count_skipped_running(&self) -> &'static str;
    /// 计数行标签：跳过（缺少 Token）。
    fn count_skipped_missing_token(&self) -> &'static str;
    /// "已取消"计数行标签（`/startall`：尚未拉起即被"停止全部机器人"取消的启动）。
    fn count_start_cancelled(&self) -> &'static str;
    /// 计数行标签：启动失败。
    fn count_start_failed(&self) -> &'static str;
    /// 计数行标签：运行中账号。
    fn count_running_accounts(&self) -> &'static str;
    /// 计数行标签：已发送重启命令。
    fn count_restart_command_sent(&self) -> &'static str;
    /// 计数行标签：重启失败。
    fn count_restart_failed(&self) -> &'static str;
    /// 计数行标签：成功发送。
    fn count_sent_ok(&self) -> &'static str;
    /// 计数行标签：发送失败。
    fn count_send_failed(&self) -> &'static str;
    /// `/startall`：一个可启动的账号都没有。
    fn startall_no_valid_accounts(&self) -> &'static str;
    /// `/startall`：一个可启动的都没有，但另有若干缺 Token 的账号。
    fn startall_no_valid_accounts_with_missing(&self, missing_token_count: usize) -> String;
    /// `/startall`：有效账号全都已经在跑。
    fn startall_all_running(&self) -> &'static str;
    /// `/startall` 的报告标题。
    fn startall_title(&self) -> &'static str;
    /// `/startall` 的跳过明细：已在运行。
    fn startall_skipped_running(&self, bot_name: &str) -> String;
    /// `/stopall` 的报告标题。
    fn stopall_title(&self) -> &'static str;
    /// `/restartall`：没有在跑的实例。
    fn restartall_nothing_running(&self) -> &'static str;
    /// `/restartall` 的报告标题。
    fn restartall_title(&self) -> &'static str;
    /// `/commandall` 的报告标题。
    fn commandall_title(&self) -> &'static str;
    /// `/commandall`：把"发的是什么"插进计数行里。
    fn commandall_sending(&self, bot_command: &str) -> String;

    // ==================== 只读查询（`show/*`） ====================
    /// `/show status` 的报告标题。
    fn status_title(&self) -> &'static str;
    /// `/show status`：一个都没在跑。
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
    /// `/show mybots`：一个都没有。
    fn mybots_empty(&self) -> &'static str;
    /// `/show mybots` 的一行。
    fn mybots_row(&self, bot_name: &str, token_status: &str) -> String;
    /// `/show allbots` 的报告标题。
    fn allbots_title(&self) -> &'static str;
    /// `/show allbots`：系统里一个机器人都没有。
    fn allbots_empty(&self) -> &'static str;
    /// `/show allbots`：已配 Token 的数量。
    fn allbots_ready_token_count(&self, ready_token_count: usize) -> String;
    /// `/show allbots`：缺 Token 的数量。
    fn allbots_missing_token_count(&self, missing_token_count: usize) -> String;
    /// `/show allbots` 的一行。
    fn allbots_row(&self, bot_name: &str, owner_id: u64, token_status: &str) -> String;
    /// `/show admins` 的报告标题。
    fn admins_title(&self) -> &'static str;
    /// `/show admins`：还没配管理员。
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
    /// `/ping`：连接超时。
    fn ping_timeout(&self, host: &str, port: u16, timeout_ms: u64) -> String;
    /// `/ping`：协议解析失败。
    fn ping_protocol_failed(&self, host: &str, port: u16, reason: &str) -> String;
    /// `/checkserver` 的报告标题。
    fn checkserver_title(&self) -> &'static str;
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

    // ================= 命令层的固定话术（`commands/*`） =================
    /// 非启动长耗时指令的第一条进度消息；`command_tag` 形如 `/stopall`。
    fn command_started_notice(&self, command_tag: &str) -> String;
    /// 命令内部发生意外错误时返回给用户的通用提示，不包含内部错误细节。
    fn command_unexpected_failure(&self) -> &'static str;
    /// 启动请求的第一条消息：明确说明请求已进入全局启动队列。
    fn command_start_queued_notice(&self, command_tag: &str) -> String;
    /// `/startall` 的第一条消息：要启动几台、队列里已有几台，以及耗时预估与等结果上限（分钟）。
    fn command_startall_queued_notice(
        &self,
        bot_count: usize,
        queued_bot_count: usize,
        estimated_minutes: u64,
        timeout_minutes: u64,
    ) -> String;
    /// 拉起单台时发生内部错误（panic），作为失败原因传给 [`Text::start_launch_failed`]。
    fn start_internal_error_reason(&self) -> &'static str;
    /// `/startall` 报告中的启动队列设置：每批最多几台、批内相邻两台的间隔、批与批之间的间隔。
    fn command_start_queue_summary(
        &self,
        batch_size: usize,
        bot_login_interval_ms: u64,
        batch_login_interval_ms: u64,
    ) -> String;
    /// `/log`：附件那一行的正文。
    fn command_log_attachment_body(
        &self,
        bot_name: &str,
        file_name: &str,
        line_count: usize,
    ) -> String;
    /// `/log`：附件被大小上限裁过时的补充说明。
    fn command_log_dropped_note(&self, dropped_line_count: usize) -> String;
    /// `/show botconfig`：附件那一行的正文。
    fn command_botconfig_attachment_body(&self, file_name: &str) -> String;
    /// `/log`：行数校验失败时的用法提示。
    fn command_log_lines_usage(&self) -> &'static str;
    /// `/ping`：端口校验失败时的用法提示。
    fn command_ping_port_usage(&self) -> &'static str;
    /// `/whoami` 的回复：自己的用户 ID + 可直接抄进 `[admins]` 的那一行。
    fn command_whoami(&self, caller_id: u64) -> String;

    // ============== 权限与共享的拼接片段（`utils/*`） ==============
    /// 计数行的统一格式（`Report::set_counts` 用）：`标签: N 个`。
    fn count_line(&self, label: &str, count: usize) -> String;
    /// 非管理员执行批量指令时的拒绝文案。
    fn admin_required(&self, caller_id: u64) -> String;
    /// Token 文件状态的展示：已就绪。
    fn token_status_ready(&self) -> &'static str;
    /// Token 文件状态的展示：还没有。
    fn token_status_missing(&self) -> &'static str;
    /// 找不到机器人目录（或无权访问），刻意不区分这两种情况。
    fn bot_dir_not_found(&self, clean_bot_name: &str) -> String;
    /// `/bot add`：名字在磁盘上已经被占了。
    fn bot_add_name_taken(&self, bot_name: &str) -> String;
    /// `/bot add`：建目录失败。
    fn create_bot_dir_failed(&self, reason: &str) -> String;
    /// 读某个用户的机器人列表失败。
    fn read_bot_list_failed(&self, reason: &str) -> String;
    /// 读机器人根目录失败。
    fn read_bots_root_failed(&self, reason: &str) -> String;
    /// "运行中不许动"的拒绝文案。
    fn refusal_bot_running(&self, bot_name: &str) -> String;
    /// 拒绝：机器人正在启动队列中排队（"运行中拒绝操作"的第二种情形）。
    fn refusal_bot_queued(&self, bot_name: &str) -> String;
    /// 往会话注入指令失败；`reason` 为 `None` 表示 `screen` 返回了非 0 但没写原因。
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
    /// 停止成功明细；`forced` 表示是强杀才停下的。
    fn stop_succeeded_line(&self, bot_name: &str) -> String;
    /// 停止失败明细：收尾后会话还在。
    fn stop_still_running(&self, bot_name: &str) -> String;
    /// 启动失败明细：日志文件建不出来。
    fn start_log_file_failed(&self, bot_name: &str) -> String;
    /// 启动成功明细：会话存活满宽限期，且日志已确认进服。
    fn start_command_sent_confirmed(&self, bot_name: &str) -> String;
    /// 启动成功明细：会话存活满宽限期，但日志尚未确认进服状态。
    fn start_command_sent_unconfirmed(&self, bot_name: &str) -> String;
    /// 启动失败明细：拉起之后会话就没了（复核时 screen 里没有它）；
    /// 常见原因是微软拒绝频繁登录（`login_with_xbox` 限流）或内存不足。
    fn start_failed(&self, bot_name: &str) -> String;
    /// 启动被取消：尚未拉起即被"停止全部机器人"取消（`/start`、`/startall`、`/restartall` 用）。
    fn start_cancelled(&self, bot_name: &str) -> String;
    /// 等待启动结果超时：该机器人未回结果，已按"未确认"收尾（`/start`、`/startall`、`/restartall` 用）。
    fn start_result_timeout(&self, bot_name: &str, seconds: u64) -> String;
    /// 重启失败明细：缺 Token，已保持原状。
    fn restart_skipped_missing_token(&self, bot_name: &str) -> String;
    /// `/restart` 被拒：目标当前不在运行（要启动请用 `/start`）。
    fn restart_not_running(&self, bot_name: &str) -> String;
    /// autocomplete 候选的显示后缀：运行中（只给"运行与未运行都列"的指令用，如 `/log`）。
    fn bot_candidate_running_note(&self) -> &'static str;
    /// autocomplete 候选的显示后缀：未运行。
    fn bot_candidate_stopped_note(&self) -> &'static str;
    /// 候选列表里正在排队启动的机器人。
    fn bot_candidate_queued_note(&self) -> &'static str;
    /// 重启失败明细：停止阶段会话没注销干净，跳过重启。
    fn restart_skipped_session_alive(&self, bot_name: &str) -> String;
    /// 拉起会话失败（`screen` 起不来）。
    fn start_launch_failed(&self, bot_name: &str, reason: &str) -> String;
    /// 通用的一行明细：`机器人 X: 说明`（说明由下层给，通常是某个错误文案）。
    fn bot_line(&self, bot_name: &str, message: &str) -> String;

    // ======== 指令元数据（注册到 Discord 的说明文字，跟随 `[i18n] language`） ========
    /// 全部指令与子指令的说明（表结构见 [`CommandText`]）。
    ///
    /// 注册指令时用它覆盖 poise 宏里写死的说明，所以它跟的是配置里的语言，与 Discord 客户端语言无关。
    /// 齐备性由覆盖测试保证：漏一条指令或选项，测试就失败。
    fn command_texts(&self) -> &'static [CommandText];

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
    /// 渲染一条日志/控制台文案（见 [`LogText`]）。
    ///
    /// 调用点统一长这样（级别仍由调用点决定，因为那是语义而不是文案）：
    /// `log::warn!("{}", text::get().log_text(LogText::BotAddFailed { .. }))`。
    fn log_text(&self, message: LogText<'_>) -> String;
}

/// 一条指令（含子指令）的说明文字：注册到 Discord 时用，跟随配置语言。
///
/// 为什么不给每条指令配一个 trait 方法：指令元数据与 `src/commands/*.rs` 里的名字一一对应，
/// 用表更好读、也更容易和 Discord 上的名字对照；齐备性由覆盖测试兜住（两种语言都要齐）。
#[derive(Debug, Clone, Copy)]
pub struct CommandText {
    /// 指令路径：顶层写 `"start"`，子指令写 `"bot add"`（与 Discord 上的名字逐字一致）。
    pub name_path: &'static str,
    /// 指令说明（Discord 指令列表里显示的那句话）。
    pub description: &'static str,
    /// 选项说明：`(选项名, 说明)`；没有选项就写空表。
    pub options: &'static [(&'static str, &'static str)],
}

/// 全部语言的实现。
///
/// 只给"每种语言都要齐备"这类测试与自查用（业务代码只该走 [`crate::text::get`]），
/// 例如覆盖测试用它逐语言检查指令元数据有没有漏。
pub fn all_languages() -> &'static [&'static dyn Text] {
    // 收成静态值：这样返回的引用是 `'static`，调用处不必关心生命周期。
    static ALL_LANGUAGES: [&dyn Text; 2] = [&zh_cn::ZH_CN, &en_us::EN_US];

    &ALL_LANGUAGES
}

/// 当前语言的文案（进程入口在 [`crate::config::Config::get_or_init`] 里用 [`install`] 选定一次）。
///
/// 语言没有安装时直接 panic：这是进程入口的调用顺序错误，不存在语言回落。
/// 单元测试进程通常不安装语言，测试构建因此单独返回英语实现（见 `uninstalled_language`）。
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
    // 测试进程通常不装语言；固定用英语，避免每个测试都调用 `install()`。
    &en_us::EN_US
}

/// 选定语言；进程内只生效第一次（与"配置只在启动时读一次"一致）。
pub fn install(language: Language) {
    // "哪种标签对应哪个实现"在 `Language::implementation` 里；`set` 失败说明已经装过
    //（重复调用不是错误），沿用第一次那个。
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
        assert_eq!(english.report_succeeded_label(), "[Success]");
        assert!(english.bot_stop_not_running("duck").contains("not running"));
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
