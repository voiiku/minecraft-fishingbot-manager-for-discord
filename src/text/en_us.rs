//! English 文案，与 `zh_cn.rs` 的接口和占位符一一对应。

use super::{CommandText, ConfigRequirement, LogText, Text};

/// 英文实现（不需要实例状态）。
pub(super) struct EnUs;

/// 进程内唯一的英文实现（[`super::install`] 用它）。
pub(super) static EN_US: EnUs = EnUs;

/// 全部指令与子指令的说明（把命令注册到 Discord 时用，表结构见 [`super::CommandText`]）。
pub(super) const COMMAND_TEXTS: &[CommandText] = &[
    CommandText {
        name_path: "help",
        description: "Show the help menu",
        options: &[],
    },
    CommandText {
        name_path: "whoami",
        description: "Show the caller's Discord user ID",
        options: &[],
    },
    CommandText {
        name_path: "show status",
        description: "Show every running bot and its status",
        options: &[],
    },
    CommandText {
        name_path: "ping",
        description: "Check the latency from this fishing host to the game server, plus the player count",
        options: &[
            (
                "host",
                "server address (domain or IP), empty uses the default target",
            ),
            ("port", "server port 1-65535, empty uses the default port"),
        ],
    },
    CommandText {
        name_path: "checkserver",
        description: "Show this fishing host's performance",
        options: &[],
    },
    CommandText {
        name_path: "show",
        description: "Show various information",
        options: &[],
    },
    CommandText {
        name_path: "show botconfig",
        description: "Show the game-side config shared by every bot (sent as an attachment)",
        options: &[],
    },
    CommandText {
        name_path: "show mybots",
        description: "Show the bots you own and their token status",
        options: &[],
    },
    CommandText {
        name_path: "show allbots",
        description: "Show every bot on this host and its token status",
        options: &[],
    },
    CommandText {
        name_path: "show admins",
        description: "Show the admin list",
        options: &[],
    },
    CommandText {
        name_path: "bot",
        description: "Manage bot accounts",
        options: &[],
    },
    CommandText {
        name_path: "bot add",
        description: "Add a bot (its token is added afterwards)",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "bot del",
        description: "Delete a bot and all of its data (stop it first)",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "bot rename",
        description: "Rename a bot (stop it first)",
        options: &[
            ("old_name", "current bot name"),
            ("new_name", "new bot name"),
        ],
    },
    CommandText {
        name_path: "bot login",
        description: "Log a bot in with Microsoft (returns a link and a code)",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "token",
        description: "Configure the bot's token",
        options: &[],
    },
    CommandText {
        name_path: "log",
        description: "Read a bot's latest log",
        options: &[
            ("name", "bot name"),
            (
                "lines",
                "how many trailing lines to read (default 50, cap in runtime.max_reply_log_lines)",
            ),
        ],
    },
    CommandText {
        name_path: "start",
        description: "Start the given bot",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "stop",
        description: "Stop the given bot",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "restart",
        description: "Restart the given bot",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "pause",
        description: "Pause fishing without going offline",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "unpause",
        description: "Resume fishing",
        options: &[("name", "bot name")],
    },
    CommandText {
        name_path: "command",
        description: "Send an in-game command to the given bot",
        options: &[
            ("name", "bot name"),
            (
                "command",
                "in-game command (leading slash = command, otherwise chat; write it as is, no quotes)",
            ),
        ],
    },
    CommandText {
        name_path: "startall",
        description: "Start every bot that has a token (admins only)",
        options: &[],
    },
    CommandText {
        name_path: "stopall",
        description: "Stop every bot (admins only)",
        options: &[],
    },
    CommandText {
        name_path: "restartall",
        description: "Restart every running bot (admins only)",
        options: &[],
    },
    CommandText {
        name_path: "commandall",
        description: "Send the same in-game command to every running bot (admins only)",
        options: &[(
            "command",
            "in-game command (leading slash = command, otherwise chat; write it as is, no quotes)",
        )],
    },
];

impl Text for EnUs {
    fn report_succeeded_label(&self) -> &'static str {
        "[Success]"
    }

    fn report_skipped_label(&self) -> &'static str {
        "[Skipped]"
    }

    fn report_queued_label(&self) -> &'static str {
        "[Queued]"
    }

    fn report_failed_label(&self) -> &'static str {
        "[Failed]"
    }

    fn report_content_label(&self) -> &'static str {
        "[Content]"
    }

    fn report_omitted_lines_note(&self, dropped_line_count: usize) -> String {
        format!("... (content too long, {dropped_line_count} lines omitted)")
    }

    fn bot_status_log_not_found(&self, bot_name: &str) -> String {
        format!("Cannot find the log file of bot {bot_name}")
    }

    fn bot_status_unreadable_log(&self, bot_name: &str) -> String {
        format!("Cannot read the log file of bot {bot_name}")
    }

    fn bot_status_empty_log(&self, bot_name: &str) -> String {
        format!("The log file of bot {bot_name} is empty")
    }

    fn bot_status_initializing(&self) -> &'static str {
        "ready / initializing"
    }

    fn bot_status_online(&self) -> &'static str {
        "server online"
    }

    fn bot_status_reconnecting(&self) -> &'static str {
        "reconnecting / account online"
    }

    fn bot_status_stopped(&self) -> &'static str {
        "process ended"
    }

    fn bot_pause_fishing(&self) -> &'static str {
        "fishing"
    }

    fn bot_pause_paused(&self) -> &'static str {
        "paused"
    }

    // FishingBot.jar's own fixed messages (the Chinese pair is in `zh_cn.rs`; the JAR prints
    // these by its own language resource, so "中文日志 ↔ `zh_cn.rs`" / "英文日志 ↔ 这里" 是一对)。
    // 来源：`docs/refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md` 的 `bot-automatic-reconnect`。
    // `/pause` 与 `/unpause` 的回显同属 JAR 打印，但已挪到配置 `[command_echo]`（要配 jar 的构建）。
    // 后缀带前导空格：它要卡住数字与单位（`... in 5 seconds...`），单写 `seconds` 太松。

    fn jar_reconnect_prefix(&self) -> &'static str {
        "FishingBot will reconnect in"
    }

    fn jar_reconnect_suffix(&self) -> &'static str {
        " seconds"
    }

    fn bot_name_empty(&self) -> &'static str {
        "Bot name cannot be empty (letters, digits, underscore and hyphen only)"
    }

    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String {
        format!(
            "Bot name '{raw_bot_name}' contains an illegal character '{illegal_char}': only letters, digits, underscore and hyphen"
        )
    }

    // ============ 单机器人指令（`bot_control/*`） ============
    fn bot_start_missing_token(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' has no token file and cannot be started.")
    }

    fn bot_start_already_running(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is already running.")
    }

    fn bot_start_already_queued(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is already in the start queue; no need to start it again.")
    }

    fn bot_start_command_sent(&self, bot_name: &str) -> String {
        format!("Start command sent for bot '{bot_name}'. Check /show status to confirm its state.")
    }

    fn bot_stop_not_running(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is not running.")
    }

    fn bot_stop_succeeded(&self, bot_name: &str) -> String {
        format!("Bot '{bot_name}' stopped successfully")
    }

    fn bot_stop_cancelled_queue(&self, bot_name: &str) -> String {
        format!("Bot '{bot_name}' was cancelled from the start queue")
    }

    fn bot_restart_command_sent(&self, bot_name: &str) -> String {
        format!(
            "Restart command sent for bot '{bot_name}'. The stop was confirmed; check /show status to confirm its state."
        )
    }

    fn bot_command_empty(&self) -> &'static str {
        "Error: the command to send must not be empty."
    }

    fn bot_command_not_running(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is not running and cannot receive commands.")
    }

    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String {
        format!("Attempted to send a command to bot '{bot_name}': {bot_command}")
    }

    fn bot_pause_confirmed(&self, bot_name: &str) -> String {
        format!("Confirmed bot '{bot_name}' paused fishing")
    }

    fn bot_unpause_confirmed(&self, bot_name: &str) -> String {
        format!("Confirmed bot '{bot_name}' resumed fishing")
    }

    fn bot_pause_superseded(&self, my_command: &str, current_pause_status: &str) -> String {
        format!("Sent {my_command}, but the latest status is {current_pause_status}")
    }

    fn bot_log_zero_lines(&self) -> &'static str {
        "Error: the line count must be greater than 0."
    }

    fn bot_log_missing(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' has no log file")
    }

    // ============ 账号管理指令（`bot_manage/*`） ============
    fn bot_add_succeeded(&self, bot_name: &str) -> String {
        format!(
            "Bot '{bot_name}' was added.\n\
             Configure its token before starting it.\n\
             Use /bot login to authenticate with Microsoft.\n\
             If /bot login fails, use /token to set the refreshToken manually."
        )
    }

    fn bot_add_failed(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("Error: failed to add bot '{raw_bot_name}': {reason}")
    }

    fn bot_delete_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("Error: failed to delete bot '{bot_name}', reason:\n{reason}")
    }

    fn bot_delete_succeeded(&self, bot_name: &str) -> String {
        format!("Bot '{bot_name}' and all of its data deleted successfully")
    }

    fn bot_rename_same_name(&self) -> &'static str {
        "Error: the new name matches the current name."
    }

    fn bot_rename_name_taken(&self, clean_new_bot_name: &str) -> String {
        format!(
            "Error: bot name '{clean_new_bot_name}' is already in use. Use /show allbots to view all bots."
        )
    }

    fn bot_rename_no_parent_dir(&self) -> &'static str {
        "Error: the parent directory of the bot directory could not be determined."
    }

    fn bot_rename_failed(&self, reason: &str) -> String {
        format!("Error: rename failed: {reason}")
    }

    fn bot_rename_succeeded(&self, old_bot_name: &str, new_bot_name: &str) -> String {
        format!("Bot '{old_bot_name}' renamed to '{new_bot_name}' successfully")
    }

    fn bot_token_write_failed(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("Failed to create/update the token file of bot {raw_bot_name}: {reason}")
    }

    fn bot_token_updated(&self, raw_bot_name: &str) -> String {
        format!("Token file of bot '{raw_bot_name}' updated successfully")
    }

    fn bot_login_prompt(&self, bot_name: &str, verification_url: &str, user_code: &str) -> String {
        format!(
            "Sign-in requested for bot '{bot_name}'. Open {verification_url} in a browser and enter \
             code {user_code}. The code is valid for approximately 10 minutes.\n\
             The result will be sent in a follow-up message."
        )
    }

    fn bot_login_succeeded(&self, bot_name: &str) -> String {
        format!(
            "Bot '{bot_name}': the sign-in credential is saved (any previous token file was removed) and \
             the bot is starting; check /show status for its state."
        )
    }

    fn bot_login_already_running(&self, bot_name: &str) -> String {
        format!(
            "Bot '{bot_name}' is already attempting to sign in. If a /bot login request is in \
             progress, wait for its result. To sign in again, stop the bot with /stop first."
        )
    }

    fn bot_login_rejected(&self, bot_name: &str) -> String {
        format!("Error: Microsoft rejected the sign-in for bot '{bot_name}'. Retry or use /token.")
    }

    fn bot_login_prompt_missing(&self, bot_name: &str) -> String {
        format!(
            "Error: the session for bot '{bot_name}' started, but no device code was displayed. \
             Check its output with /log {bot_name}. Common causes include incorrect JAR arguments, \
             insufficient memory, or a build without device-code sign-in support."
        )
    }

    fn bot_login_timed_out(&self, bot_name: &str, waited_minutes: u64) -> String {
        format!(
            "Error: obtaining credentials for bot '{bot_name}' timed out after {waited_minutes} \
             minutes. The sign-in session was cancelled. Retry or use /token."
        )
    }

    // ============ 批量指令（`all_bots_control/*`） ============
    fn count_valid_accounts(&self) -> &'static str {
        "total valid accounts"
    }

    fn count_start_command_sent(&self) -> &'static str {
        "start commands sent"
    }

    fn count_skipped_running(&self) -> &'static str {
        "skipped (already running)"
    }

    fn count_skipped_missing_token(&self) -> &'static str {
        "skipped (no token file)"
    }

    fn count_start_cancelled(&self) -> &'static str {
        "cancelled"
    }

    fn count_start_failed(&self) -> &'static str {
        "start failed"
    }

    fn count_running_accounts(&self) -> &'static str {
        "running accounts"
    }

    fn count_restart_command_sent(&self) -> &'static str {
        "restart commands sent"
    }

    fn count_restart_failed(&self) -> &'static str {
        "restart failed"
    }

    fn count_sent_ok(&self) -> &'static str {
        "sent successfully"
    }

    fn count_send_failed(&self) -> &'static str {
        "send failed"
    }

    fn startall_no_valid_accounts(&self) -> &'static str {
        "Start-all result: no eligible accounts are configured; no bots were started."
    }

    fn startall_no_valid_accounts_with_missing(&self, missing_token_count: usize) -> String {
        format!(
            "Start-all result: no eligible bots; none were started.\n\
             {missing_token_count} accounts have no token file."
        )
    }

    fn startall_all_running(&self) -> &'static str {
        "Start-all result: all eligible bots are already running."
    }

    fn startall_title(&self) -> &'static str {
        "Start-all result"
    }

    fn startall_skipped_running(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: already running")
    }

    fn startall_skipped_missing_token(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: no token file, not started")
    }

    fn stopall_title(&self) -> &'static str {
        "Stop result for all bots"
    }

    fn restartall_nothing_running(&self) -> &'static str {
        "Restart-all result: no bots are running."
    }

    fn restartall_title(&self) -> &'static str {
        "Restart-all result"
    }

    fn commandall_title(&self) -> &'static str {
        "Command-all result"
    }

    fn commandall_sending(&self, bot_command: &str) -> String {
        format!("Sending command: {bot_command}")
    }

    // ============ 只读查询（`show/*`） ============
    fn status_title(&self) -> &'static str {
        "Bot status"
    }

    fn status_no_running_bots(&self) -> &'static str {
        "No bots are currently running."
    }

    fn status_column_bot_name(&self) -> &'static str {
        "bot"
    }

    fn status_column_connection(&self) -> &'static str {
        "server status"
    }

    fn status_column_pause(&self) -> &'static str {
        "pause status"
    }

    fn status_column_uptime(&self) -> &'static str {
        "uptime"
    }

    fn common_total_count(&self, count: usize) -> String {
        format!("{count} in total")
    }

    fn mybots_title(&self, caller_id: u64) -> String {
        format!("Bot list of Discord account {caller_id}")
    }

    fn mybots_empty(&self) -> &'static str {
        "No bots under this account yet. Use /bot add to add one."
    }

    fn mybots_row(&self, bot_name: &str, token_status: &str) -> String {
        format!("{bot_name} (token file: {token_status})")
    }

    fn allbots_title(&self) -> &'static str {
        "List of every bot on this host"
    }

    fn allbots_empty(&self) -> &'static str {
        "No bots are configured on this host."
    }

    fn allbots_ready_token_count(&self, ready_token_count: usize) -> String {
        format!("Token configured: {ready_token_count}")
    }

    fn allbots_missing_token_count(&self, missing_token_count: usize) -> String {
        format!("Token to add: {missing_token_count}")
    }

    fn allbots_row(&self, bot_name: &str, owner_id: u64, token_status: &str) -> String {
        format!("bot: {bot_name} (owner: {owner_id}, token: {token_status})")
    }

    fn admins_title(&self) -> &'static str {
        "Admin list"
    }

    fn admins_empty(&self) -> &'static str {
        "No admin configured: the server admin has to add one to [admins] in the config file"
    }

    fn admins_row(&self, name: &str, user_id: &str) -> String {
        format!("{name} : {user_id}")
    }

    fn botconfig_read_failed(&self, path: &str, reason: &str) -> String {
        format!(
            "Error: could not read the game-side config {path}: {reason} (generated at startup \
             from [fishingbot])"
        )
    }

    fn botconfig_too_large(&self, bytes: usize, max_bytes: usize) -> String {
        format!(
            "Error: the game-side config is {bytes} bytes, exceeding the attachment limit of \
             {max_bytes} bytes; it cannot be sent as an attachment."
        )
    }

    // ============ 通用指令（`general/*`） ============
    fn help_title(&self) -> &'static str {
        "Help menu"
    }

    fn ping_title(&self, host: &str, port: u16) -> String {
        format!("MC server ({host}:{port})")
    }

    fn ping_status_online(&self) -> &'static str {
        "status: online"
    }

    fn ping_players(&self, online_players: usize, max_players: usize) -> String {
        format!("players: {online_players}/{max_players}")
    }

    fn ping_latency(&self, latency_ms: u128) -> String {
        format!("latency: {latency_ms} ms (from this fishing host to the game server)")
    }

    fn ping_connect_failed(&self, host: &str, port: u16, reason: &str) -> String {
        format!("Cannot connect to server ({host}:{port}):\n{reason}")
    }

    fn ping_timeout(&self, host: &str, port: u16, timeout_ms: u64) -> String {
        format!(
            "Timed out connecting to server ({host}:{port}) ({timeout_ms} ms without a response)"
        )
    }

    fn ping_protocol_failed(&self, host: &str, port: u16, reason: &str) -> String {
        format!("Failed to parse the reply of server ({host}:{port}):\n{reason}")
    }

    fn checkserver_title(&self) -> &'static str {
        "Fishing host status"
    }

    fn checkserver_cpu_usage(&self, cpu_usage: f32) -> String {
        format!("CPU usage: {cpu_usage:.1}%")
    }

    fn checkserver_memory_usage(&self, mem_usage_pct: f64) -> String {
        format!("Memory usage: {mem_usage_pct:.1}%")
    }

    fn checkserver_available_memory(&self, available_mem_mb: u64) -> String {
        format!("Memory available: {available_mem_mb} MB")
    }

    fn checkserver_disk_usage(&self, disk_usage_pct: f64) -> String {
        format!("Disk usage: {disk_usage_pct:.1}%")
    }

    fn checkserver_uptime(&self, uptime_text: &str) -> String {
        format!("Host uptime: {uptime_text}")
    }

    // ============ 命令层的固定话术（`commands/*`） ============
    fn command_started_notice(&self, command_tag: &str) -> String {
        format!("{command_tag} started. The complete result will follow in a separate message.")
    }

    fn command_unexpected_failure(&self) -> &'static str {
        "The command could not be completed because of an unexpected internal error. The error has been logged; please try again later."
    }

    fn command_start_queued_notice(&self, command_tag: &str) -> String {
        format!(
            "Request accepted: {command_tag}. Any start operation will use the global queue. The complete result will follow in a separate message."
        )
    }

    fn command_startall_queued_notice(
        &self,
        bot_count: usize,
        queued_bot_count: usize,
        estimated_minutes: u64,
        timeout_minutes: u64,
    ) -> String {
        format!(
            "Request accepted: /startall. {bot_count} bots to start, {queued_bot_count} already waiting in the start \
             queue; with the current batching settings the slowest run is about {estimated_minutes} min and the wait \
             for launch results times out after {timeout_minutes} min; the complete result will follow in a separate \
             message."
        )
    }

    fn start_internal_error_reason(&self) -> &'static str {
        "internal error (see the program log)"
    }

    fn command_start_queue_summary(
        &self,
        batch_size: usize,
        bot_login_interval_ms: u64,
        batch_login_interval_ms: u64,
    ) -> String {
        format!(
            "Start queue: up to {batch_size} per batch, {bot_login_interval_ms} ms between bots in a batch, {batch_login_interval_ms} ms between batches"
        )
    }

    fn command_log_attachment_body(
        &self,
        bot_name: &str,
        file_name: &str,
        line_count: usize,
    ) -> String {
        format!(
            "Latest log for bot '{bot_name}' ({file_name}): {line_count} lines. See the attachment."
        )
    }

    fn command_log_dropped_note(&self, dropped_line_count: usize) -> String {
        format!(
            "\nThe attachment size limit was reached; {dropped_line_count} earlier lines were omitted."
        )
    }

    fn command_botconfig_attachment_body(&self, file_name: &str) -> String {
        format!("Shared game-side config ({file_name}). See the attachment.")
    }

    fn command_log_lines_usage(&self) -> &'static str {
        "Invalid usage: line count must be an integer greater than 0. Usage: /log <bot name> [lines]."
    }

    fn command_ping_port_usage(&self) -> &'static str {
        "Invalid usage: port must be an integer from 1 to 65535. Usage: /ping [host] [port]."
    }

    fn command_whoami(&self, caller_id: u64) -> String {
        format!("Your Discord user ID is `{caller_id}`")
    }

    // ============ 权限与共享的拼接片段（`utils/*`） ============
    fn count_line(&self, label: &str, count: usize) -> String {
        format!("{label}: {count}")
    }

    fn admin_required(&self, caller_id: u64) -> String {
        format!(
            "Discord user ID {caller_id} is not an administrator and cannot perform this action."
        )
    }

    fn token_status_ready(&self) -> &'static str {
        "present"
    }

    fn token_status_missing(&self) -> &'static str {
        "to add"
    }

    fn bot_dir_not_found(&self, clean_bot_name: &str) -> String {
        format!(
            "Error: no directory was found for bot '{clean_bot_name}', or you do not have permission to access it."
        )
    }

    fn bot_add_name_taken(&self, bot_name: &str) -> String {
        format!("Error: bot name '{bot_name}' is already taken")
    }

    fn create_bot_dir_failed(&self, reason: &str) -> String {
        format!("Error: failed to create the bot directory: {reason}")
    }

    fn read_bot_list_failed(&self, reason: &str) -> String {
        format!("Error: failed to read the bot list: {reason}")
    }

    fn read_bots_root_failed(&self, reason: &str) -> String {
        format!("Error: failed to read the bots root directory: {reason}")
    }

    fn refusal_bot_running(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is running. Stop it with /stop first.")
    }

    fn refusal_bot_queued(&self, bot_name: &str) -> String {
        format!("Error: bot '{bot_name}' is waiting in the start queue. Cancel that start first.")
    }

    fn send_command_failed(&self, bot_name: &str, reason: Option<&str>) -> String {
        match reason {
            Some(reason) => {
                format!("Error: failed to send a command to bot '{bot_name}': {reason}")
            }
            None => format!("Error: failed to send a command to bot '{bot_name}'"),
        }
    }

    fn stop_signal_failed(&self, reason: &str) -> String {
        format!("Error: failed to send the stop signal: {reason}")
    }

    fn force_quit_failed(&self, reason: &str) -> String {
        format!("Error: failed to force-quit the bot: {reason}")
    }

    // ============ 启动 / 停止流程的明细行 ============
    fn count_scanned_dirs(&self) -> &'static str {
        "bot directories scanned"
    }

    fn count_stopped(&self) -> &'static str {
        "stopped successfully"
    }

    fn count_stop_failed(&self) -> &'static str {
        "stop failed"
    }

    fn stop_succeeded_line(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: stopped")
    }

    fn stop_still_running(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: the session still exists after the stop")
    }

    fn start_log_file_failed(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: cannot create the log file")
    }

    fn start_command_sent_confirmed(&self, bot_name: &str) -> String {
        format!(
            "Bot {bot_name}: start command sent, the session is up and the log confirms it joined"
        )
    }

    fn start_command_sent_unconfirmed(&self, bot_name: &str) -> String {
        format!(
            "Bot {bot_name}: start command sent, the session is up but the state is not confirmed yet"
        )
    }

    fn start_cancelled(&self, bot_name: &str) -> String {
        format!("Bot {bot_name}: starting was cancelled (all bots were being stopped)")
    }

    fn start_result_timeout(&self, bot_name: &str, seconds: u64) -> String {
        format!(
            "Bot {bot_name}: timed out waiting for the launch result ({seconds} seconds); closed as unconfirmed (check the program log or /show status)"
        )
    }

    fn start_failed(&self, bot_name: &str) -> String {
        format!(
            "Bot {bot_name}: failed to start (usually because Microsoft refuses frequent logins, or out of memory)"
        )
    }

    fn restart_skipped_missing_token(&self, bot_name: &str) -> String {
        format!(
            "Bot {bot_name}: no token file, restarting would fail to log in, left as is (run /token first, then restart)"
        )
    }

    fn restart_not_running(&self, bot_name: &str) -> String {
        format!("Bot '{bot_name}' is not running, nothing to restart!")
    }

    fn bot_candidate_running_note(&self) -> &'static str {
        " (running)"
    }

    fn bot_candidate_stopped_note(&self) -> &'static str {
        " (stopped)"
    }

    fn bot_candidate_queued_note(&self) -> &'static str {
        " (queued)"
    }

    fn restart_skipped_session_alive(&self, bot_name: &str) -> String {
        format!(
            "Bot {bot_name}: the session did not unregister during the stop phase, restart skipped"
        )
    }

    fn start_launch_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("Failed to launch bot {bot_name}: {reason}")
    }

    fn bot_line(&self, bot_name: &str, message: &str) -> String {
        format!("Bot {bot_name}: {message}")
    }

    // ============ 指令元数据（注册到 Discord 的说明文字） ============
    fn command_texts(&self) -> &'static [CommandText] {
        COMMAND_TEXTS
    }

    fn token_modal_title(&self) -> &'static str {
        "Write a bot's refreshToken"
    }

    fn token_modal_bot_name_label(&self) -> &'static str {
        "Bot name"
    }

    fn token_modal_bot_name_placeholder(&self) -> &'static str {
        "e.g. duck"
    }

    fn token_modal_token_label(&self) -> &'static str {
        "refreshToken contents"
    }

    fn token_modal_token_placeholder(&self) -> &'static str {
        "Select everything in the refreshToken file, copy it and paste it here"
    }

    // ============ 日志、启动期控制台与配置报错（与 Discord 文案共用同一套语言） ============
    fn log_text(&self, message: LogText<'_>) -> String {
        match message {
            // 程序日志
            LogText::ProcessLogFirstLine => {
                "--- Minecraft FishingBot Manager for Discord process starting ---".to_string()
            }
            LogText::ProcessLogPath { path } => format!("the program log is at: {path}"),
            LogText::ProcessLogInstallFailed { reason } => {
                format!("log init failed: {reason}")
            }
            LogText::ProcessLogAlreadyInstalled => {
                "the log was already initialized; keeping the installed logger".to_string()
            }

            // 进程入口与看门狗
            LogText::ConfigLoaded { path } => format!("config loaded: {path}"),
            LogText::ProcessId { pid } => format!("process PID: {pid}"),
            LogText::DiscordConnected { application_id } => {
                format!("connected to Discord (application ID {application_id})")
            }
            LogText::InviteUrl { url } => format!("invite link: {url}"),
            LogText::ShutdownSignal => {
                "shutdown signal received, stopping every bot (do not press Ctrl+C again)..."
                    .to_string()
            }
            LogText::ProbeFailed { reason } => format!("REST probe failed: {reason}"),
            LogText::WatchdogReady {
                window_secs,
                probe_interval_secs,
                first_handshake_timeout_secs,
            } => format!(
                "gateway watchdog ready: judging window {window_secs} s, probe every {probe_interval_secs} s, \
                 first handshake timeout {first_handshake_timeout_secs} s"
            ),
            LogText::CommandRunFailed { error } => format!("command failed: {error}"),
            LogText::CommandReplied {
                caller_id,
                command_path,
                reply,
            } => format!("replied to /{command_path} (caller {caller_id}): {reply}"),
            LogText::ReportPostedToChannel {
                caller_id,
                command_path,
                reason,
            } => format!(
                "replied to /{command_path} (caller {caller_id}): the interaction token expired ({reason}), \
                 the report was posted as a channel message instead"
            ),
            LogText::FrameworkError { detail } => format!("framework error: {detail}"),
            LogText::SigtermRegisterFailed { reason } => {
                format!("failed to register the SIGTERM handler: {reason}")
            }
            LogText::GatewayLostNeverReady => {
                "the handshake with Discord never completed after startup (a TUN-mode proxy may be \
                 needed in the client); every bot is being stopped, then a reconnect is attempted..."
                    .to_string()
            }
            LogText::GatewayLostUnreachable => {
                "Discord has been unreachable for a whole judging window; every bot is being \
                 stopped, then we try to reconnect..."
                    .to_string()
            }
            LogText::ReconnectScheduled { attempt, seconds } => {
                format!("reconnect attempt #{attempt} starts in {seconds}s")
            }
            LogText::ReconnectFailed {
                attempt,
                seconds,
                reason,
            } => format!("reconnect attempt #{attempt} failed: {reason}; retrying in {seconds}s"),
            LogText::ReconnectedAfterOffline {
                offline_secs,
                notified_guilds,
                failed_guilds,
            } => format!(
                "back online (offline for {offline_secs}s); the notice went to {notified_guilds} \
                 server(s) (failed: {failed_guilds})"
            ),
            LogText::ReconnectNoticeFailed { guild_name, reason } => {
                format!("could not send the reconnect notice to {guild_name}: {reason}")
            }
            LogText::ReconnectNoticeNoChannel => {
                "it has no system channel and no text channel".to_string()
            }
            LogText::ReconnectNoticeBody {
                offline_since,
                offline_secs,
            } => format!(
                ":warning: this bot lost contact with Discord at {offline_since} and is back online \
                 now (offline for {offline_secs}s). Every game bot was stopped while offline"
            ),
            LogText::DiscordInitFailed { error } => format!("Discord setup failed: {error}"),
            LogText::DiscordInitFailedNotInvited { error } => format!(
                "Discord setup failed: {error} (the bot may not have been invited to any server \
                 yet, or the invite may have been missing the applications.commands scope: use \
                 the invite link from the startup log to invite it again)"
            ),
            LogText::DiscordInitFailedBadToken { error } => format!(
                "Discord setup failed: {error} [most likely discord.token is wrong or was reset: \
                 Developer Portal -> your app -> Bot -> Reset Token, put the new token into the config, \
                 restart]"
            ),
            LogText::GuildCommandsCleared { guild_name, count } => format!(
                "cleared {count} leftover guild command(s) on the server {guild_name} \
                 (this program only registers global commands)"
            ),
            LogText::GuildCommandsClearFailed { guild_name, reason } => format!(
                "could not clear the leftover guild commands on the server {guild_name}: {reason} \
                 (the global registration is unaffected; the next startup tries again)"
            ),
            LogText::GlobalCommandsReadFailed { reason } => format!(
                "could not read the current global commands, so the leftover guild command \
                 cleanup is skipped this run: {reason}"
            ),
            LogText::GuildCommandsCleanupWaiting { minutes } => format!(
                "this global command set has not been in place for a full hour on Discord's side \
                 yet (about {minutes} more minutes), so the leftover guild commands are left \
                 alone this run; a later startup after that clears them automatically"
            ),
            LogText::DiscordClientEnded => {
                "the Discord connection ended (the client returned normally)".to_string()
            }
            LogText::DiscordClientError { error } => {
                format!("the Discord connection failed: {error}")
            }
            LogText::ShutdownReportTitle => "shutdown wrap-up".to_string(),
            LogText::ShutdownSignalReportTitle => "shutdown signal received, wrap-up".to_string(),
            LogText::ShutdownReportNoticeFailed { guild_name, reason } => {
                format!("could not send the shutdown wrap-up to {guild_name}: {reason}")
            }
            LogText::ShutdownReportNotified {
                notified_guilds,
                failed_guilds,
            } => format!(
                "the shutdown wrap-up went to {notified_guilds} server(s) (failed: {failed_guilds})"
            ),
            LogText::ShutdownReportNoContext => {
                "this connection never completed the handshake, so no Discord context is available; \
                 the shutdown wrap-up was written to the program log only"
                    .to_string()
            }
            LogText::ShutdownReportTimeout { seconds } => format!(
                "the shutdown wrap-up did not finish sending within {seconds}s, so the rest is \
                 skipped and the process exits (nothing is resent on the next startup)"
            ),

            // 机器人目录与房间
            LogText::BotRestartRequested { bot_name } => {
                format!("restart requested for bot {bot_name}")
            }
            LogText::BotStopped { bot_name } => format!("bot {bot_name} stopped"),
            LogText::BotAdded { bot_name } => format!("bot {bot_name} created"),
            LogText::BotAddFailed {
                raw_bot_name,
                reason,
            } => format!("failed to add bot {raw_bot_name}: {reason}"),
            LogText::BotDeleteFailed { bot_name, reason } => {
                format!("failed to delete bot {bot_name}: {reason}")
            }
            LogText::BotDeleted { bot_name } => format!("bot {bot_name} deleted successfully"),
            LogText::BotLogFirstLine => "--- bot starting ---".to_string(),
            LogText::StartLogFileFailed { bot_name, reason } => {
                format!("failed to create the log file of bot {bot_name}: {reason}")
            }
            LogText::StartSessionLaunched {
                bot_name,
                session_name,
                log_path,
            } => format!(
                "launching the screen session of bot {bot_name} ({session_name}), log: {log_path}"
            ),
            LogText::StartLaunchPanicked { bot_name, reason } => {
                format!("internal error while launching bot {bot_name}: {reason}")
            }
            LogText::StartResultTimeout { bot_name, seconds } => {
                format!(
                    "timed out after {seconds} seconds waiting for the launch result of bot {bot_name}; closed as unconfirmed"
                )
            }
            LogText::StarterRestarting { reason } => {
                format!("the startup thread crashed ({reason}); restarting it")
            }
            LogText::StartVerificationFailed { bot_name, reason } => {
                format!("the start of bot {bot_name} failed the re-check: {reason}")
            }
            LogText::UnexpectedGameLogFile { dir, file_name } => format!(
                "unexpected file '{file_name}' in the FishingBot log directory {dir} (if this keeps \
                 happening, FishingBot changed its log file name: update file_name in the \
                 [raw_game_logs] section to that name)"
            ),
            LogText::GameLogsDirRemoveFailed { dir, reason } => {
                format!("failed to clear the FishingBot log directory {dir}: {reason}")
            }
            LogText::GameLogsDirCreateFailed { dir, reason } => format!(
                "failed to create the FishingBot log directory {dir}: {reason} (FishingBot may refuse to start because of this)"
            ),
            LogText::GameLogDiscardFailed {
                file,
                device,
                reason,
            } => format!("failed to point the FishingBot log file {file} at {device}: {reason}"),
            LogText::BotLoginRejected { bot_name } => {
                format!(
                    "the login of bot {bot_name} was rejected by Microsoft (no token was created)"
                )
            }
            LogText::BotLoginStopFailed { bot_name, reason } => {
                format!("failed to stop the login session of bot {bot_name}: {reason}")
            }
            LogText::BotLoginOldTokenRemoveFailed { path, reason } => {
                format!("failed to clean up the old token file {path}: {reason}")
            }
            LogText::BotLoginOldTokenRestoreFailed { path, reason } => {
                format!("failed to move the old token file {path} back in place: {reason}")
            }
            LogText::BotLoginChmodFailed { path, reason } => {
                format!("failed to set the permissions of {path} to 0600: {reason}")
            }
            LogText::FishingbotConfigRemoved { path } => {
                format!("removed the old game-side config {path}")
            }
            LogText::FishingbotConfigWritten { path } => {
                format!("wrote the game-side config {path}")
            }
            LogText::FishingbotConfigRemoveFailed { path, reason } => {
                format!("failed to remove the old game-side config {path}: {reason}")
            }
            LogText::FishingbotConfigWriteFailed { path, reason } => {
                format!("failed to write the game-side config {path}: {reason}")
            }
            LogText::FishingbotConfigSerializeFailed { reason } => {
                format!("failed to serialize the game-side config: {reason}")
            }
            LogText::FishingbotConfigTemplateInvalid { reason } => {
                format!("the bundled game-side config template is unusable: {reason}")
            }
            LogText::FishingbotConfigTemplatePathMissing { path } => {
                format!(
                    "tried to find {path} in the game-side config template, but that path does not exist"
                )
            }
            LogText::CreateLogDirFailed { dir, reason } => {
                format!("failed to create the log directory {dir}: {reason}")
            }
            LogText::CreateLogFileFailed { path, reason } => {
                format!("failed to create the log file {path}: {reason}")
            }
            LogText::WriteLogFirstLineFailed { path, reason } => {
                format!("failed to write the first line of the log file {path}: {reason}")
            }
            LogText::OpenLogFileFailed { reason } => format!("cannot open the log file: {reason}"),
            LogText::ReadLogMetadataFailed { reason } => {
                format!("failed to read the log file metadata: {reason}")
            }
            LogText::SeekLogFileFailed { reason } => {
                format!("failed to seek in the log file: {reason}")
            }
            LogText::ReadLogContentFailed { reason } => {
                format!("failed to read the log file contents: {reason}")
            }

            // Config layer (the messages for "config could not be read at all" live in
            // `text::bootstrap` — they are fixed English, so they are not in this table)
            LogText::ConfigFieldInvalid { field, requirement } => {
                format!("config item {field} {}", requirement_text(requirement))
            }
            LogText::ConfigFieldValueInvalid {
                field,
                value,
                requirement,
            } => format!(
                "config item {field} = {value} {}",
                requirement_text(requirement)
            ),
        }
    }
}

fn requirement_text(requirement: ConfigRequirement<'_>) -> String {
    match requirement {
        ConfigRequirement::NonEmpty => "must not be empty".to_string(),
        ConfigRequirement::PlainName => {
            "must not contain whitespace or slashes, and must not be . or ..".to_string()
        }
        ConfigRequirement::PortRange => "must be a port number between 1 and 65535".to_string(),
        ConfigRequirement::GreaterThanZero => "must be greater than 0".to_string(),
        ConfigRequirement::DiscordTokenEmpty => {
            "must not be empty (Developer Portal -> your app -> Bot -> Reset Token)".to_string()
        }
        ConfigRequirement::NoSingleQuote => "must not contain a single quote".to_string(),
        ConfigRequirement::ShellSafe => {
            "must not contain quotes, backticks, $, ;, &, |, <, > or newlines".to_string()
        }
        ConfigRequirement::BaseDirIsDir => "does not exist or is not a directory".to_string(),
        ConfigRequirement::JarIsFile => {
            "does not exist or is not a file (relative paths are resolved against base_dir)"
                .to_string()
        }
        ConfigRequirement::BaseDirAbsolute => {
            "must be an absolute path (for example /your/folder/)".to_string()
        }
        ConfigRequirement::PathRelative => {
            "absolute paths are not allowed here, it must be a relative path (relative to base_dir)"
                .to_string()
        }
        ConfigRequirement::AdminsKeyUserId { user_id } => format!(
            "the key {user_id:?} is not a Discord user ID: write it as plain digits, e.g. \
             \"123456789012345678\" = \"name\""
        ),
        ConfigRequirement::AdminsNameEmpty { user_id } => {
            format!("the name of user {user_id} must not be empty")
        }
        ConfigRequirement::ReplyCharsPositive => "must be greater than 0".to_string(),
        ConfigRequirement::AutocompleteChoicesInRange => {
            "must be between 1 and 25 (Discord's autocomplete limit)".to_string()
        }
        ConfigRequirement::WatchdogTimingPositive => "must be greater than 0".to_string(),
        ConfigRequirement::EchoTextsOverlap => {
            "must not be equal to or overlap with command_echo.pause_echo, otherwise the log line cannot be attributed to one command".to_string()
        }
        ConfigRequirement::OneOf(allowed) => {
            format!("must be one of: {}", allowed.join(", "))
        }
        ConfigRequirement::McVersionOrAutomatic => {
            "must be a version string like 1.20.4, or AUTOMATIC to detect it".to_string()
        }
        // 这句话由 `Language::from_name` 生成、固定英文（说"语言"的消息在语言还没定的时候没法翻译），
        // 所以两种语言都原样透传，不再包一层。
        ConfigRequirement::LanguageUnknown { message } => message.to_string(),
    }
}
