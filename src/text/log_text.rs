//! 程序日志、启动提示与配置错误使用的文案类型。

use std::time::Duration;

use super::ConfigRequirement;

/// 一条日志或控制台文案及其参数。
#[derive(Debug, Clone, Copy)]
pub enum LogText<'a> {
    // ============ 程序日志（`process_log.rs`） ============
    /// 新日志文件的首行。
    ProcessLogFirstLine,
    /// 程序日志文件的位置。
    ProcessLogPath { path: &'a str },
    /// 日志记录器安装失败，仅输出到终端。
    ProcessLogInstallFailed { reason: &'a str },
    /// 日志记录器已安装，保留原实例。
    ProcessLogAlreadyInstalled,

    // ============ 进程入口与看门狗（`main.rs` / `watchdog.rs`） ============
    /// 配置加载成功。
    ConfigLoaded { path: &'a str },
    /// 进程 PID。
    ProcessId { pid: u32 },
    /// 已连接 Discord。
    DiscordConnected { application_id: u64 },
    /// 邀请链接。
    InviteUrl { url: &'a str },
    /// 收到退出信号。
    ShutdownSignal,
    /// REST 探活失败，等待重试。
    ProbeFailed { reason: &'a str },
    /// 网关看门狗就绪及其实际时序参数。
    WatchdogReady {
        window: Duration,
        probe_interval: Duration,
        first_handshake_timeout: Duration,
    },
    /// 指令回调返回了 `Err`。
    CommandRunFailed { error: &'a str },
    /// 发送指令失败提示时出错。
    CommandFallbackReplyFailed {
        caller_id: u64,
        command_path: &'a str,
        reason: &'a str,
    },
    /// 自动候选查询失败。
    AutocompleteLookupFailed { reason: &'a str },
    /// 自动候选查询任务失败。
    AutocompleteTaskFailed { reason: &'a str },
    /// 指令回复日志；`command_path` 是不带斜杠的 poise `qualified_name`。
    CommandReplied {
        caller_id: u64,
        command_path: &'a str,
        reply: &'a str,
    },
    /// 交互回复失败，最终报告改发频道消息；交互令牌的有效期为 15 分钟。
    ReportPostedToChannel {
        caller_id: u64,
        command_path: &'a str,
        reason: &'a str,
    },
    /// 框架级错误（非指令级）。
    FrameworkError { detail: &'a str },
    /// 注册 SIGTERM 处理失败。
    SigtermRegisterFailed { reason: &'a str },
    /// 看门狗判定首次握手超时，停止机器人后重连。
    GatewayLostNeverReady,
    /// 看门狗判定探活持续失败，停止机器人后重连。
    GatewayLostUnreachable,
    /// 计划在指定延迟后重连。
    ReconnectScheduled { attempt: u32, delay: Duration },
    /// 重连失败，将在指定延迟后重试。
    ReconnectFailed {
        attempt: u32,
        delay: Duration,
        reason: &'a str,
    },
    /// 掉线后重新连接，报告通知发送结果。
    ReconnectedAfterOffline {
        offline_secs: u64,
        notified_guilds: usize,
        failed_guilds: usize,
    },
    /// 发送恢复通知失败。
    ReconnectNoticeFailed {
        guild_name: &'a str,
        reason: &'a str,
    },
    /// 服务器没有可发通知的频道。
    ReconnectNoticeNoChannel,
    /// Discord 重连通知正文。
    ReconnectNoticeBody {
        offline_since: &'a str,
        offline_secs: u64,
    },
    /// Discord 客户端初始化失败。
    DiscordInitFailed { error: &'a str },
    /// 初始化失败：机器人未被邀请，或缺少 `applications.commands` 权限。
    DiscordInitFailedNotInvited { error: &'a str },
    /// 初始化失败：Discord Token 无效或已重置。
    DiscordInitFailedBadToken { error: &'a str },
    /// 已清理服务器遗留的 guild 指令。
    GuildCommandsCleared { guild_name: &'a str, count: usize },
    /// 清理某个服务器上遗留的 guild 指令失败；需手动再次执行同步。
    GuildCommandsClearFailed {
        guild_name: &'a str,
        reason: &'a str,
    },
    /// 与 Discord 的连接正常结束（客户端返回 `Ok`）。
    DiscordClientEnded,
    /// 与 Discord 的连接出错。
    DiscordClientError { error: &'a str },
    /// 掉线或永久错误导致进程退出时的收尾报告标题。
    ShutdownReportTitle,
    /// 收到退出信号后收尾报告的标题（正常退出，与 `/stopall` 同一份明细）。
    ShutdownSignalReportTitle,
    /// 发送退出信号收尾报告失败。
    ShutdownReportNoticeFailed {
        guild_name: &'a str,
        reason: &'a str,
    },
    /// 收到退出信号时的收尾报告已发往各服务器。
    ShutdownReportNotified {
        notified_guilds: usize,
        failed_guilds: usize,
    },
    /// 无可用 Discord 上下文，退出收尾报告仅写入日志。
    ShutdownReportNoContext,
    /// 退出收尾报告发送超时，跳过剩余消息。
    ShutdownReportTimeout { seconds: u64 },

    // ============ 机器人目录与房间（`handlers/bot_manage/*`、`utils/*`） ============
    /// 收到单机器人重启指令。
    BotRestartRequested { bot_name: &'a str },
    /// 机器人已停止。
    BotStopped { bot_name: &'a str },
    /// 机器人目录创建成功。
    BotAdded { bot_name: &'a str },
    /// 机器人添加失败。
    BotAddFailed {
        raw_bot_name: &'a str,
        reason: &'a str,
    },
    /// 机器人删除失败。
    BotDeleteFailed { bot_name: &'a str, reason: &'a str },
    /// 机器人已删除。
    BotDeleted { bot_name: &'a str },
    /// 机器人日志文件的第一行。
    BotLogFirstLine,
    /// 创建机器人日志文件失败。
    StartLogFileFailed { bot_name: &'a str, reason: &'a str },
    /// 正在启动会话。
    StartSessionLaunched {
        bot_name: &'a str,
        session_name: &'a str,
        log_path: &'a str,
    },
    /// 启动结果复核失败。
    StartVerificationFailed { bot_name: &'a str, reason: &'a str },
    /// 单机器人启动发生 panic，失败明细已写入账本。
    StartLaunchPanicked { bot_name: &'a str, reason: &'a str },
    /// 等待启动结果超时：该机器人的"未确认"明细已写回账本。
    StartResultTimeout {
        bot_name: &'a str,
        timeout: Duration,
    },
    /// 启动线程发生 panic，准备重启。
    StarterRestarting { reason: &'a str },
    /// FishingBot 日志目录包含意外文件。
    UnexpectedGameLogFile { dir: &'a str, file_name: &'a str },
    /// 清空 FishingBot 日志目录失败。
    GameLogsDirRemoveFailed { dir: &'a str, reason: &'a str },
    /// 创建 FishingBot 日志目录失败。
    GameLogsDirCreateFailed { dir: &'a str, reason: &'a str },
    /// 将 FishingBot 日志文件指向丢弃设备失败。
    GameLogDiscardFailed {
        file: &'a str,
        device: &'a str,
        reason: &'a str,
    },

    // ============ 登录流程（`handlers/bot_manage/login.rs`） ============
    /// 登录被微软拒绝，会话已停止。
    BotLoginRejected { bot_name: &'a str },
    /// 登录结束后停止会话失败。
    BotLoginStopFailed { bot_name: &'a str, reason: &'a str },
    /// 删除旧凭据失败。
    BotLoginOldTokenRemoveFailed { path: &'a str, reason: &'a str },
    /// 恢复旧凭据失败。
    BotLoginOldTokenRestoreFailed { path: &'a str, reason: &'a str },
    /// 将凭据文件权限设为 0600 失败。
    BotLoginChmodFailed { path: &'a str, reason: &'a str },

    // ============ 游戏侧配置的生成（`utils/fishingbot_config.rs`） ============
    /// 已删除旧游戏侧配置文件。
    FishingbotConfigRemoved { path: &'a str },
    /// 已生成游戏侧配置文件。
    FishingbotConfigWritten { path: &'a str },
    /// 删除旧游戏侧配置文件失败。
    FishingbotConfigRemoveFailed { path: &'a str, reason: &'a str },
    /// 写入游戏侧配置文件失败。
    FishingbotConfigWriteFailed { path: &'a str, reason: &'a str },
    /// 游戏侧配置序列化为 JSON 失败。
    FishingbotConfigSerializeFailed { reason: &'a str },
    /// 游戏侧配置模板不是有效 JSON。
    FishingbotConfigTemplateInvalid { reason: &'a str },
    /// 游戏侧配置模板缺少托管路径。
    FishingbotConfigTemplatePathMissing { path: &'a str },

    // ============ 日志文件读写（`utils/bot_logs.rs`） ============
    /// 创建日志目录失败。
    CreateLogDirFailed { dir: &'a str, reason: &'a str },
    /// 创建日志文件失败。
    CreateLogFileFailed { path: &'a str, reason: &'a str },
    /// 写入日志首行失败。
    WriteLogFirstLineFailed { path: &'a str, reason: &'a str },
    /// 打开日志文件失败。
    OpenLogFileFailed { reason: &'a str },
    /// 读取日志文件元数据失败。
    ReadLogMetadataFailed { reason: &'a str },
    /// 定位日志文件失败。
    SeekLogFileFailed { reason: &'a str },
    /// 读取日志内容失败。
    ReadLogContentFailed { reason: &'a str },

    // ============ 配置层（`config.rs` 与 `config/*`） ============
    // 配置加载前的引导期文案由 [`crate::text::bootstrap`] 提供。
    /// 字段级报错：`配置项 <字段> <要求>`。
    ConfigFieldInvalid {
        field: &'a str,
        requirement: ConfigRequirement<'a>,
    },
    /// 带取值的字段级报错：`配置项 <字段> = <取值> <要求>`。
    ConfigFieldValueInvalid {
        field: &'a str,
        value: &'a str,
        requirement: ConfigRequirement<'a>,
    },
}
