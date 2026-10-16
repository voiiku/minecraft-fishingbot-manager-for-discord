//! 简体中文文案。

use super::{CommandText, ConfigRequirement, LogText, Text};

/// 简体中文的实现（不需要实例状态）。
pub(super) struct ZhCn;

pub(super) static ZH_CN: ZhCn = ZhCn;

/// 全部指令与子指令的说明（把命令注册到 Discord 时用，表结构见 [`super::CommandText`]）。
pub(super) const COMMAND_TEXTS: &[CommandText] = &[
    CommandText {
        name_path: "help",
        description: "显示帮助菜单",
        options: &[],
    },
    CommandText {
        name_path: "whoami",
        description: "显示调用者的 Discord 用户 ID",
        options: &[],
    },
    CommandText {
        name_path: "show status",
        description: "查看全部正在运行的机器人及状态",
        options: &[],
    },
    CommandText {
        name_path: "ping",
        description: "检测钓鱼服务器至游戏服务器的延迟与在线人数",
        options: &[
            ("host", "服务器地址 (域名或 IP)，留空则使用默认目标"),
            ("port", "服务器端口 1-65535，留空则使用默认端口"),
        ],
    },
    CommandText {
        name_path: "checkserver",
        description: "查看钓鱼服务器自身的性能状态",
        options: &[],
    },
    CommandText {
        name_path: "show",
        description: "查看各种信息",
        options: &[],
    },
    CommandText {
        name_path: "show botconfig",
        description: "查看所有机器人共用的游戏侧配置 (以附件发送)",
        options: &[],
    },
    CommandText {
        name_path: "show mybots",
        description: "查看自己名下的机器人及 Token 配置情况",
        options: &[],
    },
    CommandText {
        name_path: "show allbots",
        description: "查看全服机器人及 Token 配置情况",
        options: &[],
    },
    CommandText {
        name_path: "show admins",
        description: "查看管理员名单",
        options: &[],
    },
    CommandText {
        name_path: "bot",
        description: "管理机器人账号",
        options: &[],
    },
    CommandText {
        name_path: "bot add",
        description: "添加一个机器人 (之后还需要添加 Token)",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "bot del",
        description: "删除机器人及其全部数据 (请先停止机器人)",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "bot rename",
        description: "给机器人改名 (请先停止机器人)",
        options: &[("old_name", "原机器人名"), ("new_name", "新机器人名")],
    },
    CommandText {
        name_path: "bot login",
        description: "用微软账号登录机器人 (会给出链接与代码)",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "token",
        description: "配置机器人的 Token",
        options: &[],
    },
    CommandText {
        name_path: "log",
        description: "读取机器人的最新日志",
        options: &[
            ("name", "机器人名"),
            (
                "lines",
                "读取最新多少行 (默认 50，上限见配置 runtime.max_reply_log_lines)",
            ),
        ],
    },
    CommandText {
        name_path: "start",
        description: "启动指定机器人",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "stop",
        description: "停止指定机器人",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "restart",
        description: "重启指定机器人",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "pause",
        description: "暂停钓鱼但不下线",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "unpause",
        description: "取消暂停钓鱼",
        options: &[("name", "机器人名")],
    },
    CommandText {
        name_path: "command",
        description: "向机器人发送一条游戏内指令",
        options: &[("name", "机器人名"), ("command", "游戏内指令")],
    },
    CommandText {
        name_path: "startall",
        description: "启动全部已配置 Token 的机器人 (仅管理员)",
        options: &[],
    },
    CommandText {
        name_path: "stopall",
        description: "停止全部机器人 (仅管理员)",
        options: &[],
    },
    CommandText {
        name_path: "restartall",
        description: "重启全部正在运行的机器人 (仅管理员)",
        options: &[],
    },
    CommandText {
        name_path: "commandall",
        description: "向全部运行中的机器人发送同一条游戏内指令 (仅管理员)",
        options: &[(
            "command",
            "游戏内指令 (带斜杠是指令，不带是聊天；直接写，含空格也不用加引号)",
        )],
    },
];

impl Text for ZhCn {
    fn report_succeeded_label(&self) -> &'static str {
        "[成功明细]"
    }

    fn report_skipped_label(&self) -> &'static str {
        "[跳过明细]"
    }

    fn report_queued_label(&self) -> &'static str {
        "[排队中]"
    }

    fn report_failed_label(&self) -> &'static str {
        "[失败明细]"
    }

    fn report_content_label(&self) -> &'static str {
        "[内容]"
    }

    fn report_omitted_lines_note(&self, dropped_line_count: usize) -> String {
        format!("…(内容过长，已省略 {dropped_line_count} 行)")
    }

    fn bot_status_log_not_found(&self, bot_name: &str) -> String {
        format!("找不到机器人 {bot_name} 的日志文件")
    }

    fn bot_status_unreadable_log(&self, bot_name: &str) -> String {
        format!("无法读取机器人 {bot_name} 的日志文件")
    }

    fn bot_status_empty_log(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 的日志文件是空的")
    }

    fn bot_status_initializing(&self) -> &'static str {
        "就绪/初始中"
    }

    fn bot_status_online(&self) -> &'static str {
        "服务器在线"
    }

    fn bot_status_reconnecting(&self) -> &'static str {
        "重连/本人在线"
    }

    fn bot_status_stopped(&self) -> &'static str {
        "进程已结束"
    }

    fn bot_pause_fishing(&self) -> &'static str {
        "钓鱼中"
    }

    fn bot_pause_paused(&self) -> &'static str {
        "已暂停"
    }

    // ===== FishingBot.jar 自己打印的固定文案（`utils/bot_status.rs` 拿它们在日志里认重连）=====
    // 这两句不是 JAR 的可调项：它按自己的语言资源打印，所以"中文日志 ↔ 这里的中文"、
    // "英文日志 ↔ `en_us.rs` 里的英文"是一对；换 JAR 的语言时把 `[i18n] language` 一起换掉，
    // 两边就对齐了。
    // 来源：`docs/refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md` 的 `bot-automatic-reconnect`。
    // `/pause` 与 `/unpause` 的回显同属 JAR 打印，但已挪到配置 `[command_echo]`（要配 jar 的构建）。

    fn jar_reconnect_prefix(&self) -> &'static str {
        "钓鱼机器人将在"
    }

    fn jar_reconnect_suffix(&self) -> &'static str {
        "秒内重新连接"
    }

    fn bot_name_empty(&self) -> &'static str {
        "机器人名不能为空(只允许字母、数字、下划线与连字符)"
    }

    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String {
        format!(
            "机器人名 '{raw_bot_name}' 含非法字符 '{illegal_char}'：只允许字母、数字、下划线与连字符"
        )
    }

    // ============ 单机器人指令（`bot_control/*`） ============
    fn bot_start_missing_token(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 缺少 Token 文件，无法启动！")
    }

    fn bot_start_already_running(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 已在运行中，无需启动！")
    }

    fn bot_start_already_queued(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 已在启动队列中，无需重复启动")
    }

    fn bot_start_command_sent(&self, bot_name: &str) -> String {
        format!("已发送机器人 '{bot_name}' 的启动命令（启动状态请用 /show status 确认）")
    }

    fn bot_stop_not_running(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 未在运行，无需停止")
    }

    fn bot_stop_succeeded(&self, bot_name: &str) -> String {
        format!("已成功停止机器人 '{bot_name}'")
    }

    fn bot_stop_cancelled_queue(&self, bot_name: &str) -> String {
        format!("已取消机器人 '{bot_name}' 的排队启动")
    }

    fn bot_restart_command_sent(&self, bot_name: &str) -> String {
        format!(
            "已发送机器人 '{bot_name}' 的重启命令（已确认成功停止，启动状态请用 /show status 确认）"
        )
    }

    fn bot_command_empty(&self) -> &'static str {
        "错误:发送的指令不能为空！"
    }

    fn bot_command_not_running(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 未运行，无法发送指令！")
    }

    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String {
        format!("已尝试向机器人 '{bot_name}' 发送指令: {bot_command}")
    }

    fn bot_pause_confirmed(&self, bot_name: &str) -> String {
        format!("已确认机器人 '{bot_name}' 暂停钓鱼")
    }

    fn bot_unpause_confirmed(&self, bot_name: &str) -> String {
        format!("已确认机器人 '{bot_name}' 恢复钓鱼")
    }

    fn bot_pause_superseded(&self, my_command: &str, current_pause_status: &str) -> String {
        format!("发送 {my_command}，但最新状态是 {current_pause_status}")
    }

    fn bot_log_zero_lines(&self) -> &'static str {
        "错误:日志读取行数必须大于 0！"
    }

    fn bot_log_missing(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 日志文件不存在")
    }

    // ============ 账号管理指令（`bot_manage/*`） ============
    fn bot_add_succeeded(&self, bot_name: &str) -> String {
        format!(
            "添加机器人 '{bot_name}' 成功！\n\
             注意：还需为其配置 Token 后才可启动。\n\
             建议使用 /bot login 在线登录\n\
             只有 /bot login 登录失败时，才应当使用 /token 命令手动设置 token"
        )
    }

    fn bot_add_failed(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("错误:添加机器人 '{raw_bot_name}' 失败: {reason}")
    }

    fn bot_delete_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("错误:删除机器人 '{bot_name}' 失败，原因如下:\n{reason}")
    }

    fn bot_delete_succeeded(&self, bot_name: &str) -> String {
        format!("已成功删除机器人 '{bot_name}' 及其全部数据")
    }

    fn bot_rename_same_name(&self) -> &'static str {
        "错误:新旧名称一致，无需修改！"
    }

    fn bot_rename_name_taken(&self, clean_new_bot_name: &str) -> String {
        format!(
            "错误:机器人名 '{clean_new_bot_name}' 已被占用，请使用 /show allbots 查看全部机器人"
        )
    }

    fn bot_rename_no_parent_dir(&self) -> &'static str {
        "错误:无法获取原机器人目录的父目录！"
    }

    fn bot_rename_failed(&self, reason: &str) -> String {
        format!("错误:重命名失败: {reason}")
    }

    fn bot_rename_succeeded(&self, old_bot_name: &str, new_bot_name: &str) -> String {
        format!("已成功将机器人 '{old_bot_name}' 重命名为 '{new_bot_name}'")
    }

    fn bot_token_write_failed(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("创建/更新机器人 {raw_bot_name} 的 Token 文件失败: {reason}")
    }

    fn bot_token_updated(&self, raw_bot_name: &str) -> String {
        format!("已成功更新机器人 '{raw_bot_name}' 的 Token 文件")
    }

    fn bot_login_prompt(&self, bot_name: &str, verification_url: &str, user_code: &str) -> String {
        format!(
            "尝试登录机器人 {bot_name}：请在浏览器打开 {verification_url} 并输入代码 \
             {user_code}（此代码约 10 分钟内有效）。\n\
             授权成功后会再次回复结果。"
        )
    }

    fn bot_login_succeeded(&self, bot_name: &str) -> String {
        format!(
            "机器人 {bot_name} 的登录凭据已保存（若存在旧凭据会被删除），正在启动（可用 /show status 查看状态）"
        )
    }

    fn bot_login_already_running(&self, bot_name: &str) -> String {
        format!(
            "机器人 {bot_name} 正在尝试登录：若它正在执行/bot login，请等待结果消息；若希望重新登录请先 /stop"
        )
    }

    fn bot_login_rejected(&self, bot_name: &str) -> String {
        format!("错误:机器人 {bot_name} 的登录被微软拒绝。请重试或使用 /token")
    }

    fn bot_login_prompt_missing(&self, bot_name: &str) -> String {
        format!(
            "错误:机器人 {bot_name} 的会话已启动，但没有设备码。请用 /log {bot_name} 查看输出：常见原因是 jar 参数不对、内存不足，或这份构建没有设备码登录"
        )
    }

    fn bot_login_timed_out(&self, bot_name: &str, waited_minutes: u64) -> String {
        format!(
            "错误:获取机器人 {bot_name} 凭据超时（{waited_minutes} 分钟），已取消登录会话。请重试或用 /token"
        )
    }

    // ============ 批量指令（`all_bots_control/*`） ============
    fn count_valid_accounts(&self) -> &'static str {
        "有效账号总数"
    }

    fn count_start_command_sent(&self) -> &'static str {
        "已发送启动命令"
    }

    fn count_skipped_running(&self) -> &'static str {
        "跳过（已在运行）"
    }

    fn count_skipped_missing_token(&self) -> &'static str {
        "跳过（缺少 Token）"
    }

    fn count_start_cancelled(&self) -> &'static str {
        "已取消"
    }

    fn count_start_failed(&self) -> &'static str {
        "启动失败"
    }

    fn count_running_accounts(&self) -> &'static str {
        "运行中账号"
    }

    fn count_restart_command_sent(&self) -> &'static str {
        "已发送重启命令"
    }

    fn count_restart_failed(&self) -> &'static str {
        "重启失败"
    }

    fn count_sent_ok(&self) -> &'static str {
        "成功发送"
    }

    fn count_send_failed(&self) -> &'static str {
        "发送失败"
    }

    fn startall_no_valid_accounts(&self) -> &'static str {
        "全部启动执行结果: 未配置任何有效账号，未启动"
    }

    fn startall_no_valid_accounts_with_missing(&self, missing_token_count: usize) -> String {
        format!(
            "全部启动执行结果: 无有效机器人，未启动。\n(另有 {missing_token_count} 个账号缺少 Token 文件)"
        )
    }

    fn startall_all_running(&self) -> &'static str {
        "全部启动执行结果: 全部有效机器人均已运行，无需重复启动"
    }

    fn startall_title(&self) -> &'static str {
        "全部启动执行结果"
    }

    fn startall_skipped_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 已在运行")
    }

    fn startall_skipped_missing_token(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 缺少 Token 文件，未启动")
    }

    fn stopall_title(&self) -> &'static str {
        "全服机器人停止结果"
    }

    fn restartall_nothing_running(&self) -> &'static str {
        "全部重启执行结果: 没有运行的机器人实例，无需重启"
    }

    fn restartall_title(&self) -> &'static str {
        "全部重启执行结果"
    }

    fn commandall_title(&self) -> &'static str {
        "全部发送指令结果"
    }

    fn commandall_sending(&self, bot_command: &str) -> String {
        format!("发送指令: {bot_command}")
    }

    // ============ 只读查询（`show/*`） ============
    fn status_title(&self) -> &'static str {
        "机器人运行状态"
    }

    fn status_no_running_bots(&self) -> &'static str {
        "目前没有正在运行的机器人"
    }

    fn status_column_bot_name(&self) -> &'static str {
        "机器人"
    }

    fn status_column_connection(&self) -> &'static str {
        "服务器状态"
    }

    fn status_column_pause(&self) -> &'static str {
        "暂停状态"
    }

    fn status_column_uptime(&self) -> &'static str {
        "运行时长"
    }

    fn common_total_count(&self, count: usize) -> String {
        format!("共 {count} 个")
    }

    fn mybots_title(&self, caller_id: u64) -> String {
        format!("Discord 账号 {caller_id} 的机器人列表")
    }

    fn mybots_empty(&self) -> &'static str {
        "该账号名下暂无机器人（可用 /bot add 添加）"
    }

    fn mybots_row(&self, bot_name: &str, token_status: &str) -> String {
        format!("{bot_name} (Token文件: {token_status})")
    }

    fn allbots_title(&self) -> &'static str {
        "全服机器人列表"
    }

    fn allbots_empty(&self) -> &'static str {
        "暂无机器人"
    }

    fn allbots_ready_token_count(&self, ready_token_count: usize) -> String {
        format!("已配置 Token: {ready_token_count} 个")
    }

    fn allbots_missing_token_count(&self, missing_token_count: usize) -> String {
        format!("待添加 Token: {missing_token_count} 个")
    }

    fn allbots_row(&self, bot_name: &str, owner_id: u64, token_status: &str) -> String {
        format!("机器人: {bot_name} (所属用户: {owner_id}, Token: {token_status})")
    }

    fn admins_title(&self) -> &'static str {
        "管理员名单"
    }

    fn admins_empty(&self) -> &'static str {
        "未配置管理员：请服务器管理员在配置文件中的 [admins] 项内添加"
    }

    fn admins_row(&self, name: &str, user_id: &str) -> String {
        format!("{name} : {user_id}")
    }

    fn botconfig_read_failed(&self, path: &str, reason: &str) -> String {
        format!("错误:读取钓鱼配置 {path} 失败: {reason}(它由本程序在启动时按 [fishingbot] 生成)")
    }

    fn botconfig_too_large(&self, bytes: usize, max_bytes: usize) -> String {
        format!("错误:钓鱼配置 {bytes} 字节，超过附件上限 {max_bytes} 字节，无法作为附件发送。")
    }

    // ============ 通用指令（`general/*`） ============
    fn help_title(&self) -> &'static str {
        "帮助菜单"
    }

    fn ping_title(&self, host: &str, port: u16) -> String {
        format!("MC 服务器 ({host}:{port})")
    }

    fn ping_status_online(&self) -> &'static str {
        "状态: 在线"
    }

    fn ping_players(&self, online_players: usize, max_players: usize) -> String {
        format!("人数: {online_players}/{max_players}")
    }

    fn ping_latency(&self, latency_ms: u128) -> String {
        format!("延迟: {latency_ms} ms(钓鱼服务器到游戏服务器的延迟)")
    }

    fn ping_connect_failed(&self, host: &str, port: u16, reason: &str) -> String {
        format!("服务器 ({host}:{port}) 连接失败：\n{reason}")
    }

    fn ping_timeout(&self, host: &str, port: u16, timeout_ms: u64) -> String {
        format!("连接服务器 ({host}:{port}) 超时({timeout_ms} 毫秒无响应)")
    }

    fn ping_protocol_failed(&self, host: &str, port: u16, reason: &str) -> String {
        format!("服务器 ({host}:{port}) 协议解析失败：\n{reason}")
    }

    fn checkserver_title(&self) -> &'static str {
        "钓鱼服务器状态"
    }

    fn checkserver_cpu_usage(&self, cpu_usage: f32) -> String {
        format!("CPU 占用率: {cpu_usage:.1}%")
    }

    fn checkserver_memory_usage(&self, mem_usage_pct: f64) -> String {
        format!("内存占用率: {mem_usage_pct:.1}%")
    }

    fn checkserver_available_memory(&self, available_mem_mb: u64) -> String {
        format!("可用内存: {available_mem_mb} MB")
    }

    fn checkserver_disk_usage(&self, disk_usage_pct: f64) -> String {
        format!("磁盘占用率: {disk_usage_pct:.1}%")
    }

    fn checkserver_uptime(&self, uptime_text: &str) -> String {
        format!("服务器物理运行时间: {uptime_text}")
    }

    // ============ 命令层的固定话术（`commands/*`） ============
    fn command_started_notice(&self, command_tag: &str) -> String {
        format!("已开始执行 {command_tag}，执行完毕后会返回完整结果")
    }

    fn command_unexpected_failure(&self) -> &'static str {
        "命令遇到意外的内部错误，未能完成。错误已记录至进程日志，请稍后重试。"
    }

    fn command_start_queued_notice(&self, command_tag: &str) -> String {
        format!(
            "已接受 {command_tag} 请求；涉及启动的操作将由全局启动队列执行，完成后会返回完整结果"
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
            "已接受 /startall 请求；本次启动 {bot_count} 台，其中启动队列中已有 {queued_bot_count} 台在等待；\
             按当前分批设置，预计最长耗时 {estimated_minutes} 分钟，等待启动结果的超时时间为 {timeout_minutes} 分钟；\
             完成后将返回完整结果"
        )
    }

    fn start_internal_error_reason(&self) -> &'static str {
        "内部错误（见程序日志）"
    }

    fn command_start_queue_summary(
        &self,
        batch_size: usize,
        bot_login_interval_ms: u64,
        batch_login_interval_ms: u64,
    ) -> String {
        format!(
            "启动队列: 每批最多 {batch_size} 台，批内相邻两台间隔 {bot_login_interval_ms} 毫秒，批与批之间间隔 {batch_login_interval_ms} 毫秒"
        )
    }

    fn command_log_attachment_body(
        &self,
        bot_name: &str,
        file_name: &str,
        line_count: usize,
    ) -> String {
        format!("机器人 '{bot_name}' 的最新日志 ({file_name})：共 {line_count} 行(见附件)")
    }

    fn command_log_dropped_note(&self, dropped_line_count: usize) -> String {
        format!("\n附件大小达到上限，最早的 {dropped_line_count} 行被省略了")
    }

    fn command_botconfig_attachment_body(&self, file_name: &str) -> String {
        format!("钓鱼配置 ({file_name}，见附件)")
    }

    fn command_log_lines_usage(&self) -> &'static str {
        "用法错误：行数必须是大于 0 的整数，用法: /log <机器人名> [行数]"
    }

    fn command_ping_port_usage(&self) -> &'static str {
        "用法错误：端口必须是 1-65535 之间的整数，用法: /ping [地址] [端口]"
    }

    fn command_whoami(&self, caller_id: u64) -> String {
        format!("你的 Discord 用户 ID 是 `{caller_id}`")
    }

    // ============ 权限与共享的拼接片段（`utils/*`） ============
    fn count_line(&self, label: &str, count: usize) -> String {
        format!("{label}: {count} 个")
    }

    fn admin_required(&self, caller_id: u64) -> String {
        format!("Discord 用户 ID {caller_id} 不是管理员，无权执行此操作！")
    }

    fn token_status_ready(&self) -> &'static str {
        "已存在"
    }

    fn token_status_missing(&self) -> &'static str {
        "待添加"
    }

    fn bot_dir_not_found(&self, clean_bot_name: &str) -> String {
        format!("错误:未找到机器人 '{clean_bot_name}' 对应的目录，或你没有权限访问")
    }

    fn bot_add_name_taken(&self, bot_name: &str) -> String {
        format!("错误:机器人名 '{bot_name}' 已被占用")
    }

    fn create_bot_dir_failed(&self, reason: &str) -> String {
        format!("错误:创建机器人目录失败: {reason}")
    }

    fn read_bot_list_failed(&self, reason: &str) -> String {
        format!("错误:读取机器人列表失败: {reason}")
    }

    fn read_bots_root_failed(&self, reason: &str) -> String {
        format!("错误:读取机器人根目录失败: {reason}")
    }

    fn refusal_bot_running(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 正在运行，请先停止它")
    }

    fn refusal_bot_queued(&self, bot_name: &str) -> String {
        format!("错误:机器人 '{bot_name}' 正在启动队列中排队，请先取消它的排队启动")
    }

    fn send_command_failed(&self, bot_name: &str, reason: Option<&str>) -> String {
        match reason {
            Some(reason) => format!("错误:向机器人 '{bot_name}' 发送指令失败: {reason}"),
            None => format!("错误:向机器人 '{bot_name}' 发送指令失败"),
        }
    }

    fn stop_signal_failed(&self, reason: &str) -> String {
        format!("错误:发送停止信号失败: {reason}")
    }

    fn force_quit_failed(&self, reason: &str) -> String {
        format!("错误:强制终止机器人失败: {reason}")
    }

    // ============ 启动 / 停止流程的明细行 ============
    fn count_scanned_dirs(&self) -> &'static str {
        "扫描机器人目录"
    }

    fn count_stopped(&self) -> &'static str {
        "成功停止"
    }

    fn count_stop_failed(&self) -> &'static str {
        "停止失败"
    }

    fn stop_succeeded_line(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 已停止")
    }

    fn stop_still_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 停止后会话仍然存在")
    }

    fn start_log_file_failed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 无法创建日志文件")
    }

    fn start_command_sent_confirmed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 已确认进服")
    }

    fn start_command_sent_unconfirmed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 会话已存在但尚未确认状态（可稍后用/show status 确认）")
    }

    fn start_failed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 启动失败(常见原因是微软拒绝频繁登录或内存不足)")
    }

    fn start_cancelled(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 启动被取消")
    }

    fn start_result_timeout(&self, bot_name: &str, seconds: u64) -> String {
        format!(
            "机器人 {bot_name}: 等待启动结果超时（{seconds} 秒），已按未确认处理（请查看日志或用 /show status 确认）"
        )
    }

    fn restart_skipped_missing_token(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 缺少 Token 文件，拒绝重启，若希望停止请用/stop 命令")
    }

    fn restart_not_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 未在运行，无需重启")
    }

    fn bot_candidate_running_note(&self) -> &'static str {
        "(运行中)"
    }

    fn bot_candidate_stopped_note(&self) -> &'static str {
        "(已停止)"
    }

    fn bot_candidate_queued_note(&self) -> &'static str {
        "(排队中)"
    }

    fn restart_skipped_session_alive(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 在停止阶段未能注销会话，已跳过重启")
    }

    fn start_launch_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("启动机器人 {bot_name} 失败: {reason}")
    }

    fn bot_line(&self, bot_name: &str, message: &str) -> String {
        format!("机器人 {bot_name}: {message}")
    }

    // ============ 指令元数据（注册到 Discord 的说明文字） ============
    fn command_texts(&self) -> &'static [CommandText] {
        COMMAND_TEXTS
    }

    fn token_modal_title(&self) -> &'static str {
        "写入机器人 refreshToken"
    }

    fn token_modal_bot_name_label(&self) -> &'static str {
        "机器人名"
    }

    fn token_modal_bot_name_placeholder(&self) -> &'static str {
        "例如 duck"
    }

    fn token_modal_token_label(&self) -> &'static str {
        "refreshToken 内容"
    }

    fn token_modal_token_placeholder(&self) -> &'static str {
        "把 refreshToken 文件中的内容全选复制，粘贴到这里"
    }

    // ============ 日志、启动期控制台与配置报错（与 Discord 文案共用同一套语言） ============
    fn log_text(&self, message: LogText<'_>) -> String {
        match message {
            // 程序日志
            LogText::ProcessLogFirstLine => {
                "--- 正在启动 Minecraft FishingBot Manager for Discord 进程 ---".to_string()
            }
            LogText::ProcessLogPath { path } => format!("本次程序日志存储于: {path}"),
            LogText::ProcessLogInstallFailed { reason } => {
                format!("日志初始化失败： {reason}")
            }
            LogText::ProcessLogAlreadyInstalled => {
                "日志已经装过一次，本次保留已装的日志系统。".to_string()
            }

            // 进程入口与看门狗
            LogText::ConfigLoaded { path } => format!("已加载配置: {path}"),
            LogText::ProcessId { pid } => format!("进程 PID: {pid}"),
            LogText::DiscordConnected { application_id } => {
                format!("已连接 Discord (应用 ID {application_id})")
            }
            LogText::InviteUrl { url } => format!("邀请链接: {url}"),
            LogText::ShutdownSignal => {
                "收到退出信号，正在停止全部机器人 (请勿重复按 Ctrl+C)……".to_string()
            }
            LogText::ProbeFailed { reason } => format!("REST 探活失败: {reason}"),
            LogText::WatchdogReady {
                window_secs,
                probe_interval_secs,
                first_handshake_timeout_secs,
            } => format!(
                "网关看门狗已就绪：判定窗口 {window_secs} 秒、探活间隔 {probe_interval_secs} 秒、\
                 首次握手超时 {first_handshake_timeout_secs} 秒"
            ),
            LogText::CommandRunFailed { error } => format!("指令执行出错: {error}"),
            LogText::CommandReplied {
                caller_id,
                command_path,
                reply,
            } => format!("回复 /{command_path} (调用者 {caller_id}): {reply}"),
            LogText::ReportPostedToChannel {
                caller_id,
                command_path,
                reason,
            } => format!(
                "回复 /{command_path} (调用者 {caller_id}): 交互令牌已过期（{reason}），发送频道消息"
            ),
            LogText::FrameworkError { detail } => format!("框架错误: {detail}"),
            LogText::SigtermRegisterFailed { reason } => {
                format!("注册 SIGTERM 处理失败: {reason}")
            }
            LogText::GatewayLostNeverReady => {
                "未能在启动后与 Discord 完成握手（可能需要在客户端启用 TUN 模式代理）；正在停止全部机器人，随后尝试重建连接……".to_string()
            }
            LogText::GatewayLostUnreachable => {
                "已经连续一个判定窗口无法访问 Discord，正在停止全部机器人，随后尝试重建连接……"
                    .to_string()
            }
            LogText::ReconnectScheduled { attempt, seconds } => {
                format!("第 {attempt} 次重建连接将于 {seconds} 秒后开始")
            }
            LogText::ReconnectFailed {
                attempt,
                seconds,
                reason,
            } => format!("第 {attempt} 次重建连接失败: {reason}，将于 {seconds} 秒后重试"),
            LogText::ReconnectedAfterOffline {
                offline_secs,
                notified_guilds,
                failed_guilds,
            } => format!(
                "已重新连接 Discord (期间掉线 {offline_secs} 秒)；恢复通知已发往 {notified_guilds} 个服务器(发送失败 {failed_guilds} 次)"
            ),
            LogText::ReconnectNoticeFailed { guild_name, reason } => {
                format!("向服务器 {guild_name} 发掉线恢复通知失败: {reason}")
            }
            LogText::ReconnectNoticeNoChannel => "此服务器既没有系统频道、也没有文字频道".to_string(),
            LogText::ReconnectNoticeBody {
                offline_since,
                offline_secs,
            } => format!(
                "⚠️ 本机器人于 {offline_since} 起与 Discord 网络连接中断，目前已自动恢复(掉线 {offline_secs} 秒)。\
                 掉线期间已停止全部游戏机器人"
            ),
            LogText::DiscordInitFailed { error } => format!("Discord 初始化失败: {error}"),
            LogText::DiscordInitFailedNotInvited { error } => format!(
                "Discord 初始化失败: {error} [可能是机器人还没被邀请进服务器，或邀请时缺少 \
                 applications.commands 权限：你可以用启动日志中的邀请链接再邀请一次]"
            ),
            LogText::DiscordInitFailedBadToken { error } => format!(
                "Discord 初始化失败: {error} [可能是 discord.token 不对或已被 Reset：\
                 开发者面板 → 你的应用 → Bot → Reset Token，修改为新 token 再重启本进程]"
            ),
            LogText::GuildCommandsCleared {
                guild_name,
                count,
            } => format!(
                "已清掉服务器 {guild_name} 上遗留的 {count} 条 guild 指令 (本程序只做全局注册)"
            ),
            LogText::GuildCommandsClearFailed { guild_name, reason } => format!(
                "清掉服务器 {guild_name} 上遗留的 guild 指令失败: {reason} (不影响全局注册，下次启动会再试)"
            ),
            LogText::GlobalCommandsReadFailed { reason } => {
                format!("读不到现有的全局指令，本轮跳过 guild 指令残留清理: {reason}")
            }
            LogText::GuildCommandsCleanupWaiting { minutes } => format!(
                "这套全局指令在 Discord 侧还没满 1 小时 (还差约 {minutes} 分钟)，本轮不清各服务器上\
                 遗留的 guild 指令；满 1 小时后的下一次启动会自动清理"
            ),
            LogText::DiscordClientEnded => "与 Discord 的连接已结束 (客户端正常返回)".to_string(),
            LogText::DiscordClientError { error } => format!("与 Discord 的连接发送错误: {error}"),
            LogText::ShutdownReportTitle => "进程退出报告".to_string(),
            LogText::ShutdownSignalReportTitle => "收到来自系统的退出信号，停止结果".to_string(),
            LogText::ShutdownReportNoticeFailed { guild_name, reason } => {
                format!("向服务器 {guild_name} 发退出收尾报告失败: {reason}")
            }
            LogText::ShutdownReportNotified {
                notified_guilds,
                failed_guilds,
            } => format!(
                "退出收尾报告已发往 {notified_guilds} 个服务器(发送失败 {failed_guilds} 次)"
            ),
            LogText::ShutdownReportNoContext => {
                "本次连接未完成握手，没有可用的 Discord 上下文；退出收尾报告仅写入程序日志".to_string()
            }
            LogText::ShutdownReportTimeout { seconds } => format!(
                "退出收尾报告 {seconds} 秒内没发完，跳过剩余发送并退出 (下次启动不补发)"
            ),

            // 机器人目录与房间
            LogText::BotRestartRequested { bot_name } => {
                format!("已接收到机器人 {bot_name} 的重启命令")
            }
            LogText::BotStopped { bot_name } => format!("机器人 {bot_name} 已成功停止"),
            LogText::BotAdded { bot_name } => format!("机器人 {bot_name} 创建成功"),
            LogText::BotAddFailed {
                raw_bot_name,
                reason,
            } => format!("添加机器人 {raw_bot_name} 失败: {reason}"),
            LogText::BotDeleteFailed { bot_name, reason } => {
                format!("删除机器人 {bot_name} 失败: {reason}")
            }
            LogText::BotDeleted { bot_name } => format!("成功删除机器人 {bot_name} "),
            LogText::BotLogFirstLine => "--- 启动机器人 ---".to_string(),
            LogText::StartLogFileFailed { bot_name, reason } => {
                format!("机器人 {bot_name} 的日志文件创建失败: {reason}")
            }
            LogText::StartSessionLaunched {
                bot_name,
                session_name,
                log_path,
            } => format!(
                "尝试启动机器人 {bot_name} ， Screen 会话 ({session_name})，日志: {log_path}"
            ),
            LogText::StartLaunchPanicked { bot_name, reason } => format!(
                "拉起机器人 {bot_name} 时发生内部错误：{reason}"
            ),
            LogText::StartResultTimeout { bot_name, seconds } => format!(
                "等待机器人 {bot_name} 的启动结果超时（{seconds} 秒），已按未确认处理"
            ),
            LogText::StarterRestarting { reason } => {
                format!("启动线程异常退出（{reason}），正在重启它")
            }
            LogText::StartVerificationFailed { bot_name, reason } => format!("机器人 {bot_name} 启动复核没过: {reason}"),
            LogText::UnexpectedGameLogFile { dir, file_name } => format!(
                "FishingBot 日志目录 {dir} 内存在意料之外的文件 '{file_name}' (若反复出现说明 FishingBot 更改了日志文件名，\
                 请把配置 [raw_game_logs] 的 file_name 更新成这个名字)"
            ),
            LogText::GameLogsDirRemoveFailed { dir, reason } => {
                format!("清空 FishingBot 原始日志目录 {dir} 失败: {reason}")
            }
            LogText::GameLogsDirCreateFailed { dir, reason } => {
                format!("创建 FishingBot 原始日志目录 {dir} 失败: {reason} (钓鱼机器人可能因此拒绝启动)")
            }
            LogText::GameLogDiscardFailed {
                file,
                device,
                reason,
            } => format!(
                "将 FishingBot 的原始日志文件 {file} 指向 {device} 失败: {reason}"
            ),
            LogText::BotLoginRejected { bot_name } => {
                format!("机器人 {bot_name} 的登录被微软拒绝(未能创建凭据)")
            }
            LogText::BotLoginStopFailed { bot_name, reason } => {
                format!("停止机器人 {bot_name} 的登录会话失败: {reason}")
            }
            LogText::BotLoginOldTokenRemoveFailed { path, reason } => {
                format!("清理旧的凭据文件 {path} 失败: {reason}")
            }
            LogText::BotLoginOldTokenRestoreFailed { path, reason } => {
                format!("把旧凭据文件 {path} 放回原位失败: {reason}")
            }
            LogText::BotLoginChmodFailed { path, reason } => {
                format!("把凭据文件 {path} 的权限改成 0600 失败: {reason}")
            }
            LogText::FishingbotConfigRemoved { path } => {
                format!("已删除旧的游戏侧配置文件 {path}")
            }
            LogText::FishingbotConfigWritten { path } => {
                format!("已生成游戏侧配置文件 {path}")
            }
            LogText::FishingbotConfigRemoveFailed { path, reason } => {
                format!("删除旧的游戏侧配置文件 {path} 失败: {reason}")
            }
            LogText::FishingbotConfigWriteFailed { path, reason } => {
                format!("写游戏侧配置文件 {path} 失败: {reason}")
            }
            LogText::FishingbotConfigSerializeFailed { reason } => {
                format!("游戏侧配置序列化成 JSON 失败: {reason}")
            }
            LogText::FishingbotConfigTemplateInvalid { reason } => {
                format!("随仓库发布的游戏侧配置模板用不了: {reason}")
            }
            LogText::FishingbotConfigTemplatePathMissing { path } => {
                format!("尝试在游戏侧配置模板中寻找{path}，但此路径不存在")
            }
            LogText::CreateLogDirFailed { dir, reason } => {
                format!("创建日志目录 {dir} 失败: {reason}")
            }
            LogText::CreateLogFileFailed { path, reason } => {
                format!("创建日志文件 {path} 失败: {reason}")
            }
            LogText::WriteLogFirstLineFailed { path, reason } => {
                format!("写入日志首行 {path} 失败: {reason}")
            }
            LogText::OpenLogFileFailed { reason } => format!("无法打开日志文件: {reason}"),
            LogText::ReadLogMetadataFailed { reason } => {
                format!("读取日志文件信息失败: {reason}")
            }
            LogText::SeekLogFileFailed { reason } => format!("定位日志文件失败: {reason}"),
            LogText::ReadLogContentFailed { reason } => format!("读取日志内容失败: {reason}"),

            // 配置层（"配置还没读成功"的那几条提示走 `text::bootstrap`，固定英文，不在这张表里）
            LogText::ConfigFieldInvalid { field, requirement } => {
                format!("配置项 {field} {}", requirement_text(requirement))
            }
            LogText::ConfigFieldValueInvalid {
                field,
                value,
                requirement,
            } => format!("配置项 {field} = {value} {}", requirement_text(requirement)),
        }
    }
}

/// 配置校验里那一句"要求"（`ConfigFieldInvalid` / `ConfigFieldValueInvalid` 用）。
///
/// 单独一个函数是为了让"多条规则共用同一句话"只翻一次；`match` 穷尽，漏一条编译不过。
fn requirement_text(requirement: ConfigRequirement<'_>) -> String {
    match requirement {
        ConfigRequirement::NonEmpty => "不能为空".to_string(),
        ConfigRequirement::PlainName => "不能包含空格或斜杠，也不能写成 . 或 ..".to_string(),
        ConfigRequirement::PortRange => "必须是 1-65535 之间的端口号".to_string(),
        ConfigRequirement::GreaterThanZero => "必须大于 0".to_string(),
        ConfigRequirement::DiscordTokenEmpty => {
            "不能为空 (在Discord开发者面板 → 你的应用 → Bot → Reset Token 中获取)".to_string()
        }
        ConfigRequirement::NoSingleQuote => "不能包含单引号".to_string(),
        ConfigRequirement::ShellSafe => "不能包含引号、反引号、$、;、&、|、<、> 或换行".to_string(),
        ConfigRequirement::BaseDirIsDir => "不存在或不是目录".to_string(),
        ConfigRequirement::JarIsFile => "不存在或不是文件 (相对路径以 base_dir 为基准)".to_string(),
        ConfigRequirement::BaseDirAbsolute => "此项必须是绝对路径 (例如 /your/folder/)".to_string(),
        ConfigRequirement::PathRelative => {
            "此项禁止绝对路径，必须写成相对路径 (相对于 base_dir)".to_string()
        }
        ConfigRequirement::AdminsKeyUserId { user_id } => format!(
            "的键 {user_id:?} 不是 Discord 用户 ID：应写成纯数字，例如 \"123456789012345678\" = \"名字\""
        ),
        ConfigRequirement::AdminsNameEmpty { user_id } => {
            format!("里用户 {user_id} 的名字不能为空")
        }
        ConfigRequirement::ReplyCharsPositive => "必须大于 0".to_string(),
        ConfigRequirement::AutocompleteChoicesInRange => {
            "必须在 1-25 之间 (Discord 对候选的上限)".to_string()
        }
        ConfigRequirement::WatchdogTimingPositive => "必须大于 0".to_string(),
        ConfigRequirement::EchoTextsOverlap => {
            "不能与 command_echo.pause_echo 相同或互相包含".to_string()
        }
        ConfigRequirement::OneOf(allowed) => {
            format!("必须是以下之一: {}", allowed.join(", "))
        }
        ConfigRequirement::McVersionOrAutomatic => {
            "必须写 AUTOMATIC (自动探测) 或版本号 (例如 1.20.4)".to_string()
        }

        ConfigRequirement::LanguageUnknown { message } => message.to_string(),
    }
}
