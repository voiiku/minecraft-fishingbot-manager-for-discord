//! 程序日志、启动提示与配置错误使用的文案类型。

use super::ConfigRequirement;

/// 一条日志或控制台文案及其参数。
#[derive(Debug, Clone, Copy)]
pub enum LogText<'a> {
    // ============ 程序日志（`process_log.rs`） ============
    /// 新日志文件的第一行（"页首"）。
    ProcessLogFirstLine,
    /// 程序日志文件的位置。
    ProcessLogPath { path: &'a str },
    /// 装 logger 失败，本次只能输出到终端。
    ProcessLogInstallFailed { reason: &'a str },
    /// `RUST_LOG` 写了个不认识的值，按回落级别处理。
    RustLogInvalidLevel { raw: &'a str, fallback: &'a str },

    // ============ 进程入口与看门狗（`main.rs` / `watchdog.rs`） ============
    /// 配置加载成功。
    ConfigLoaded { path: &'a str },
    /// 进程 PID。
    ProcessId { pid: u32 },
    /// 已连上 Discord。
    DiscordConnected { application_id: u64 },
    /// 邀请链接。
    InviteUrl { url: &'a str },
    /// 收到退出信号。
    ShutdownSignal,
    /// REST 探活失败（还在重试）。
    ProbeFailed { reason: &'a str },
    /// 网关看门狗就绪：把在用的三个时序参数一起报出来（排查时以这一行为准）。
    WatchdogReady {
        window_secs: u64,
        probe_interval_secs: u64,
        first_handshake_timeout_secs: u64,
    },
    /// 指令回调返回了 `Err`。
    CommandRunFailed { error: &'a str },
    /// 每次回复用户都记一条：谁调的、哪条指令、回了什么。
    ///
    /// 记录回复内容，供排查问题时核对。
    /// `command_path` 是 poise 的 `qualified_name`（如 `start`、`bot add`，不带斜杠）。
    CommandReplied {
        caller_id: u64,
        command_path: &'a str,
        reply: &'a str,
    },
    /// 最终报告没能作为交互回复发出，改发普通频道消息。
    ///
    /// 交互令牌只有 15 分钟（见 `docs/CONTEXTS/CONTEXT-zh_cn.md` 第 3 节）：批量启动跑得比它久时
    /// followup 一定失败，报告改走普通频道消息，用户至少能看到结果。
    ReportPostedToChannel {
        caller_id: u64,
        command_path: &'a str,
        reason: &'a str,
    },
    /// 框架级错误（非指令级）。
    FrameworkError { detail: &'a str },
    /// 注册 SIGTERM 处理失败。
    SigtermRegisterFailed { reason: &'a str },
    /// 看门狗判定"启动后从未握手"：正在停机器人，随后重建连接。
    GatewayLostNeverReady,
    /// 看门狗判定"探活持续失败"：正在停机器人，随后重建连接。
    GatewayLostUnreachable,
    /// 掉线后开始重建连接（第 N 次尝试，等 M 秒）。
    ReconnectScheduled { attempt: u32, seconds: u64 },
    /// 重建连接失败（建连或装配出错），等 M 秒再试。
    ReconnectFailed {
        attempt: u32,
        seconds: u64,
        reason: &'a str,
    },
    /// 掉线之后重新连上了：通知已发往各服务器。
    ReconnectedAfterOffline {
        offline_secs: u64,
        notified_guilds: usize,
        failed_guilds: usize,
    },
    /// 给某个服务器发恢复通知失败。
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
    /// 装配阶段就失败了（例如 token 被拒），还没跑到任何事情。
    DiscordInitFailed { error: &'a str },
    /// 同上，但成因多半是机器人还没被邀请进服务器，或邀请时缺少 `applications.commands` 权限
    ///（Discord 对注册斜杠指令回 `50001 Missing Access`）。
    DiscordInitFailedNotInvited { error: &'a str },
    /// 同上，但成因多半是 `discord.token` 不对或已被 Reset（Discord 回 401）。
    DiscordInitFailedBadToken { error: &'a str },
    /// 清掉了某个服务器上遗留的 guild 指令（以前注册到该服务器的那些）。
    GuildCommandsCleared { guild_name: &'a str, count: usize },
    /// 清理某个服务器上遗留的 guild 指令失败；只影响这次清理，下次启动会再试一次。
    GuildCommandsClearFailed {
        guild_name: &'a str,
        reason: &'a str,
    },
    /// 读不到现有的全局指令，于是跳过 guild 指令残留清理（注册本身不受影响）。
    GlobalCommandsReadFailed { reason: &'a str },
    /// 这套全局指令还不满 1 小时（或本轮刚改过），本轮不动 guild 指令残留。
    GuildCommandsCleanupWaiting { minutes: u64 },
    /// 与 Discord 的连接正常结束（客户端返回 `Ok`）。
    DiscordClientEnded,
    /// 与 Discord 的连接出错。
    DiscordClientError { error: &'a str },
    /// 进程退出收尾报告的标题。
    ShutdownReportTitle,

    // ============ 机器人目录与房间（`bot_manage/*`、`utils/*`） ============
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
    /// 启动时缺 Token，未拉起。
    StartSkippedMissingToken { bot_name: &'a str },
    /// 建机器人日志文件失败（带原因）。
    StartLogFileFailed { bot_name: &'a str, reason: &'a str },
    /// 正在拉起会话。
    StartSessionLaunched {
        bot_name: &'a str,
        session_name: &'a str,
        log_path: &'a str,
    },
    /// FishingBot 日志目录里有意料之外的文件。
    UnexpectedGameLogFile { dir: &'a str, file_name: &'a str },
    /// 清空 FishingBot 日志目录失败。
    GameLogsDirRemoveFailed { dir: &'a str, reason: &'a str },
    /// 创建 FishingBot 日志目录失败。
    GameLogsDirCreateFailed { dir: &'a str, reason: &'a str },
    /// 把 FishingBot 的日志文件指向丢弃设备失败。
    GameLogDiscardFailed {
        file: &'a str,
        device: &'a str,
        reason: &'a str,
    },

    // ============ 登录流程（`bot_manage/login.rs`） ============
    /// 登录被微软拒绝（凭据没建出来），会话已停。
    BotLoginRejected { bot_name: &'a str },
    /// 登录超时后停会话没停干净。
    BotLoginStopFailed { bot_name: &'a str, reason: &'a str },
    /// 清掉残留的旧凭据失败。
    BotLoginOldTokenRemoveFailed { path: &'a str, reason: &'a str },
    /// 把挪开的旧凭据放回原位失败。
    BotLoginOldTokenRestoreFailed { path: &'a str, reason: &'a str },
    /// 把凭据文件权限收紧到 0600 失败。
    BotLoginChmodFailed { path: &'a str, reason: &'a str },

    // ============ 游戏侧配置的生成（`utils/fishingbot_config.rs`） ============
    /// 生成前删掉了旧的游戏侧配置文件。
    FishingbotConfigRemoved { path: &'a str },
    /// 已生成游戏侧配置文件。
    FishingbotConfigWritten { path: &'a str },
    /// 删旧的游戏侧配置文件失败。
    FishingbotConfigRemoveFailed { path: &'a str, reason: &'a str },
    /// 写游戏侧配置文件失败。
    FishingbotConfigWriteFailed { path: &'a str, reason: &'a str },
    /// 把游戏侧配置序列化成 JSON 失败（理论上不会发生，理由写进调用点）。
    FishingbotConfigSerializeFailed { reason: &'a str },
    /// 仓库里那份游戏侧配置模板用不了：不是合法 JSON，或与生成代码对不上（上游改了键名）。
    FishingbotConfigTemplateInvalid { reason: &'a str },

    // ============ 日志文件读写（`utils/bot_log_files.rs`） ============
    /// 建日志目录失败。
    CreateLogDirFailed { dir: &'a str, reason: &'a str },
    /// 建日志文件失败。
    CreateLogFileFailed { path: &'a str, reason: &'a str },
    /// 写日志首行失败。
    WriteLogFirstLineFailed { path: &'a str, reason: &'a str },
    /// 打开日志文件失败（`/log` 与状态判定都会碰到）。
    OpenLogFileFailed { reason: &'a str },
    /// 读日志文件信息（长度）失败。
    ReadLogMetadataFailed { reason: &'a str },
    /// 在日志文件里定位失败。
    SeekLogFileFailed { reason: &'a str },
    /// 读日志内容失败。
    ReadLogContentFailed { reason: &'a str },

    // ============ 配置层（`config.rs` 与 `config/*`） ============
    // "配置还没读成功"的那几条提示不在这里：它们属于引导期文案，走 [`crate::text::bootstrap`]（固定英文）。
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
