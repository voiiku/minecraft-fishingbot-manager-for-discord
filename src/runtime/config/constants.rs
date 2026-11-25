//! 不由配置文件提供的固定运行参数。

use std::num::NonZeroUsize;
use std::time::Duration;

pub(super) const CONFIG_FILE_NAME: &str = "minecraft-fishingbot-manager-for-discord.conf.toml";
pub(super) const PRIVATE_FILE_MODE: u32 = 0o600;
pub(super) const SHELL_UNSAFE: &[char] =
    &['\'', '"', '`', '$', ';', '&', '|', '<', '>', '\n', '\r'];
pub(super) const ANNOUNCE_TYPES: &[&str] = &[
    "ALL",
    "ALL_BUT_FISH",
    "ONLY_ENCHANTED",
    "ONLY_BOOKS",
    "NONE",
];

/// 与文件配置一起加载的内置运行参数。
#[derive(Debug)]
pub struct Constants {
    /// Discord 附件大小上限（字节）。
    pub max_attachment_bytes: usize,
    /// Discord 自动候选数量的协议上限。
    pub max_autocomplete_choices: usize,
    /// Discord 命令说明的 UTF-16 长度上限。
    pub max_command_description_utf16: usize,
    /// `/log` 未指定行数时读取的行数。
    pub default_log_lines: usize,
    /// Token 弹窗中机器人名的长度上限。
    pub bot_name_modal_max_length: u16,
    /// Token 弹窗中凭据的长度上限。
    pub token_modal_max_length: u16,
    /// 等待登录设备码的时间。
    pub login_code_timeout: Duration,
    /// 等待用户完成登录授权的时间（分钟）。
    pub login_authorization_timeout_minutes: u64,
    /// 登录状态轮询间隔。
    pub login_poll_interval: Duration,
    /// 登录状态每次扫描的日志行数。
    pub login_log_tail_lines: usize,
    /// FishingBot 输出设备码的标记。
    pub login_device_code_marker: &'static str,
    /// FishingBot 报告凭据生成失败的标记。
    pub login_auth_failure_marker: &'static str,
    /// 登录时旧凭据的备份后缀。
    pub token_backup_suffix: &'static str,
    /// 单次状态查询的超时，也用于批量启动耗时估算。
    pub status_timeout: Duration,
    /// 状态查询的轮询间隔。
    pub status_poll_interval: Duration,
    /// 状态查询每次扫描的日志行数。
    pub status_scan_lines: usize,
    /// 按机器人名分片的 screen 指令锁数量。
    pub screen_command_lock_count: NonZeroUsize,
    /// 倒序读取日志时的读块大小（字节）。
    pub log_read_block_bytes: NonZeroUsize,
    /// 日志文件名的时间戳格式，须可解析完整的日期和时间。
    pub log_filename_time_format: &'static str,
    /// 配置及凭据文件的 Unix 权限。
    pub private_file_mode: u32,
    /// 队列满载启动时间的结果超时余量倍数。
    pub startup_result_timeout_factor: u64,
    /// 启动线程异常退出后的退避时间。
    pub starter_restart_backoff: Duration,
    /// 退出时发送 Discord 报告的总时限。
    pub shutdown_report_timeout: Duration,
    /// `/checkserver` 两次 CPU 采样的间隔。
    pub cpu_sample_interval: Duration,
    /// Discord 邀请链接申请的权限位。
    pub discord_invite_permissions: u64,
    /// 部署目录下的程序日志目录名。
    pub process_log_dir_name: &'static str,
    /// 程序日志记录的时间格式。
    pub process_log_timestamp_format: &'static str,
    /// 用户可见的系统时间和掉线时间格式。
    pub display_timestamp_format: &'static str,
    /// 程序日志多行记录的续行缩进。
    pub process_log_continuation_indent: &'static str,
    /// `/show status` 表格的字段分隔符。
    pub status_column_separator: &'static str,
}

pub(super) fn defaults() -> Constants {
    Constants {
        max_attachment_bytes: 8 * 1024 * 1024,
        max_autocomplete_choices: 25,
        max_command_description_utf16: 100,
        default_log_lines: 50,
        bot_name_modal_max_length: 64,
        token_modal_max_length: 4000,
        login_code_timeout: Duration::from_secs(60),
        login_authorization_timeout_minutes: 10,
        login_poll_interval: Duration::from_millis(500),
        login_log_tail_lines: 200,
        login_device_code_marker: "using the following code",
        login_auth_failure_marker: "Error while creating refresh token",
        token_backup_suffix: "renewing",
        status_timeout: Duration::from_secs(2),
        status_poll_interval: Duration::from_millis(50),
        status_scan_lines: 100,
        screen_command_lock_count: NonZeroUsize::new(64)
            .expect("screen_command_lock_count must be nonzero"),
        log_read_block_bytes: NonZeroUsize::new(16 * 1024)
            .expect("log_read_block_bytes must be nonzero"),
        log_filename_time_format: "%Y-%m-%d-%H-%M-%S",
        private_file_mode: PRIVATE_FILE_MODE,
        startup_result_timeout_factor: 2,
        starter_restart_backoff: Duration::from_secs(1),
        shutdown_report_timeout: Duration::from_secs(30),
        cpu_sample_interval: Duration::from_millis(200),
        discord_invite_permissions: 60416,
        process_log_dir_name: "logs",
        process_log_timestamp_format: "%Y-%m-%d %H:%M:%S",
        display_timestamp_format: "%Y-%m-%d %H:%M:%S %Z (UTC%:z)",
        process_log_continuation_indent: "  ",
        status_column_separator: " | ",
    }
}
