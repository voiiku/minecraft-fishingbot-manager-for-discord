//! 简体中文文案。

use std::time::Duration;

use super::super::{
    CommandText, CommandTextProblem, LogMessages, Text, WatchdogText, format_seconds,
};

/// 简体中文文案实现。
pub(in crate::text) struct ZhCn;

pub(in crate::text) static ZH_CN: ZhCn = ZhCn;

/// Discord 指令与子指令说明。
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
        description: "查看运行中的机器人与反 AFK 状态",
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
        name_path: "sync-commands",
        description: "按配置更新指令并清理旧范围 (仅管理员)",
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
        options: &[("name", "机器人名"), ("lines", "读取最新日志行数")],
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
        name_path: "pauseall",
        description: "暂停全部运行中机器人的钓鱼 (仅管理员)",
        options: &[],
    },
    CommandText {
        name_path: "unpauseall",
        description: "恢复全部运行中机器人的钓鱼 (仅管理员)",
        options: &[],
    },
    CommandText {
        name_path: "commandall",
        description: "向全部运行中的机器人发送同一条游戏内指令 (仅管理员)",
        options: &[(
            "command",
            "游戏内指令 (带斜杠为指令，不带为聊天；直接写入，含空格时无需加引号)",
        )],
    },
    CommandText {
        name_path: "anti-afk",
        description: "启动或停止反 AFK (仅管理员)",
        options: &[("command", "选择 on 启动或 off 停止")],
    },
];

impl Text for ZhCn {
    fn watchdog_text(&self, message: WatchdogText) -> &'static str {
        match message {
            WatchdogText::StartedLockReset => "看门狗 started 锁中毒：重置连接起始时间失败",
            WatchdogText::LastHandshakeLockClear => {
                "看门狗 last_handshake 锁中毒：清除握手状态失败"
            }
            WatchdogText::LastHandshakeLockRecord => {
                "看门狗 last_handshake 锁中毒：记录握手状态失败"
            }
            WatchdogText::LastHandshakeLockRead => "看门狗 last_handshake 锁中毒：读取握手状态失败",
            WatchdogText::UnhealthySinceLockClear => {
                "看门狗 unhealthy_since 锁中毒：清除探活失败状态失败"
            }
            WatchdogText::UnhealthySinceLockRecord => {
                "看门狗 unhealthy_since 锁中毒：记录探活失败状态失败"
            }
            WatchdogText::UnhealthySinceLockRead => {
                "看门狗 unhealthy_since 锁中毒：读取探活失败状态失败"
            }
            WatchdogText::StartedLockRead => "看门狗 started 锁中毒：读取连接起始时间失败",
            WatchdogText::OfflineSinceLockRecord => "看门狗 offline_since 锁中毒：记录掉线状态失败",
            WatchdogText::OfflineSinceLockRead => "看门狗 offline_since 锁中毒：读取掉线状态失败",
            WatchdogText::PendingDisconnectReasonLockRecord => {
                "看门狗 pending_disconnect_reason 锁中毒：记录掉线原因失败"
            }
            WatchdogText::PendingDisconnectReasonLockRead => {
                "看门狗 pending_disconnect_reason 锁中毒：读取掉线原因失败"
            }
            WatchdogText::NoticeCtxLockSendReport => {
                "看门狗 notice_ctx 锁中毒：读取收尾通知上下文失败"
            }
            WatchdogText::NoticeCtxLockRecord => "看门狗 notice_ctx 锁中毒：记录通知上下文失败",
        }
    }

    fn report_succeeded_label(&self) -> &'static str {
        "[完成明细]"
    }

    fn report_skipped_label(&self) -> &'static str {
        "[跳过明细]"
    }

    fn report_queued_label(&self) -> &'static str {
        "[排队中]"
    }

    fn report_failed_label(&self) -> &'static str {
        "[异常明细]"
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

    fn bot_status_joined(&self) -> &'static str {
        "已进服"
    }

    fn bot_status_not_joined(&self) -> &'static str {
        "未进服"
    }

    fn bot_status_unavailable(&self) -> &'static str {
        "状态不可用（会话异常）"
    }

    fn bot_status_stopped(&self) -> &'static str {
        "进程已结束"
    }

    fn bot_pause_unpaused(&self) -> &'static str {
        "未暂停"
    }

    fn bot_pause_paused(&self) -> &'static str {
        "已暂停"
    }

    fn bot_status_field_unavailable(&self) -> &'static str {
        "不可用"
    }

    fn bot_status_field_unknown(&self) -> &'static str {
        "未确认"
    }

    fn bot_status_log_changed(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 的日志在状态查询期间发生变化")
    }

    fn bot_status_query_timeout(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 状态查询超时，未收到有效回显")
    }

    fn bot_status_query_after_command_failed(&self, command: &str, error: &str) -> String {
        format!("已发送 {command}，但状态查询失败: {error}")
    }

    fn jar_status_prefix(&self) -> &'static str {
        "钓鱼状态："
    }

    fn jar_status_unavailable(&self) -> &'static str {
        "钓鱼状态：不可用"
    }

    fn bot_name_empty(&self) -> &'static str {
        "机器人名不能为空"
    }

    fn bot_name_illegal_char(&self, raw_bot_name: &str, illegal_char: char) -> String {
        format!(
            "机器人名 '{raw_bot_name}' 含非法字符 '{illegal_char}'：只允许字母、数字、下划线与连字符"
        )
    }

    // ============ 单机器人指令（`operations/*`） ============
    fn bot_start_missing_token(&self, bot_name: &str) -> String {
        format!("错误: 机器人 '{bot_name}' 缺少 Token 文件，无法启动！")
    }

    fn bot_start_already_running(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 的 screen 会话已存在，无法启动")
    }

    fn bot_start_already_queued(&self, bot_name: &str) -> String {
        format!("错误: 机器人 '{bot_name}' 已在启动队列中，无需重复启动")
    }

    fn bot_stop_not_running(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 没有 screen 会话，无需停止")
    }

    fn bot_stop_succeeded(&self, bot_name: &str) -> String {
        format!("已成功停止机器人 '{bot_name}'")
    }

    fn bot_stop_cancelled_queue(&self, bot_name: &str) -> String {
        format!("已取消机器人 '{bot_name}' 的排队启动")
    }

    fn bot_restart_online_confirmed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 重启后已确认进服")
    }

    fn bot_restart_not_joined(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 重启后会话已存在，当前未进服（可稍后使用 /show status 查看）")
    }

    fn bot_command_empty(&self) -> &'static str {
        "错误: 发送的指令不能为空！"
    }

    fn bot_command_not_running(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 没有 screen 会话，无法发送指令")
    }

    fn bot_command_sent(&self, bot_name: &str, bot_command: &str) -> String {
        format!("已尝试向机器人 '{bot_name}' 发送指令: {bot_command}")
    }

    fn bot_pause_confirmed(&self, bot_name: &str) -> String {
        format!("已确认机器人 '{bot_name}' 处于暂停状态")
    }

    fn bot_unpause_confirmed(&self, bot_name: &str) -> String {
        format!("已确认机器人 '{bot_name}' 未暂停钓鱼")
    }

    fn bot_pause_query_unavailable(&self, bot_name: &str, bot_command: &str) -> String {
        format!("机器人 '{bot_name}': 已发送 {bot_command}，状态不可用（会话异常）")
    }

    fn bot_pause_status_mismatch(&self, my_command: &str, current_pause_status: &str) -> String {
        format!("发送 {my_command}，但最新状态是 {current_pause_status}")
    }

    fn bot_log_zero_lines(&self) -> &'static str {
        "错误: 日志读取行数必须大于 0！"
    }

    fn bot_log_missing(&self, bot_name: &str) -> String {
        format!("错误: 机器人 '{bot_name}' 日志文件不存在")
    }

    // ============ 账号管理指令（`operations/accounts/*`） ============
    fn bot_add_succeeded(&self, bot_name: &str) -> String {
        format!("已添加机器人 '{bot_name}'；请使用 /bot login 配置凭据，失败时可使用 /token。")
    }

    fn bot_add_failed(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("错误: 添加机器人 '{raw_bot_name}' 失败: {reason}")
    }

    fn bot_delete_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("错误: 删除机器人 '{bot_name}' 失败，原因如下:\n{reason}")
    }

    fn bot_delete_succeeded(&self, bot_name: &str) -> String {
        format!("已成功删除机器人 '{bot_name}' 及其全部数据")
    }

    fn bot_rename_same_name(&self) -> &'static str {
        "错误: 新旧名称一致，无需修改！"
    }

    fn bot_rename_name_taken(&self, clean_new_bot_name: &str) -> String {
        format!(
            "错误: 机器人名 '{clean_new_bot_name}' 已被占用，请使用 /show allbots 查看全部机器人"
        )
    }

    fn bot_rename_no_parent_dir(&self) -> &'static str {
        "错误: 无法获取原机器人目录的父目录！"
    }

    fn bot_rename_failed(&self, reason: &str) -> String {
        format!("错误: 重命名失败: {reason}")
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

    fn bot_login_prompt(
        &self,
        bot_name: &str,
        verification_url: &str,
        user_code: &str,
        minutes: u64,
    ) -> String {
        format!(
            "机器人 {bot_name}: 请在 {minutes} 分钟内打开 {verification_url} 并输入代码 {user_code}；授权结果将另行通知。"
        )
    }

    fn bot_login_prompt_logged(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 已生成登录提示（内容不写入程序日志）")
    }

    fn bot_login_succeeded(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 登录凭据已保存；可使用 /show status 查看状态")
    }

    fn bot_login_already_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 已存在会话且缺少 Token；若正在登录，请等待结果或停止会话后重试")
    }

    fn bot_login_rejected(&self, bot_name: &str) -> String {
        format!("错误: 机器人 {bot_name} 的登录被微软拒绝。请重试或使用 /token")
    }

    fn bot_login_prompt_missing(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 未找到设备码，请使用 /log {bot_name} 查看日志")
    }

    fn bot_login_timed_out(&self, bot_name: &str, waited_minutes: u64) -> String {
        format!(
            "机器人 {bot_name}: 等待凭据超时（{waited_minutes} 分钟）；请检查会话状态后重试或使用 /token"
        )
    }

    // ============ 批量指令（`operations/*`） ============
    fn count_valid_accounts(&self) -> &'static str {
        "已配置 Token"
    }

    fn count_start_verified(&self) -> &'static str {
        "启动复核通过"
    }

    fn count_skipped_running(&self) -> &'static str {
        "跳过（会话已存在）"
    }

    fn count_skipped_missing_token(&self) -> &'static str {
        "跳过（缺少 Token）"
    }

    fn count_start_cancelled(&self) -> &'static str {
        "已取消"
    }

    fn count_start_failed(&self) -> &'static str {
        "启动异常"
    }

    fn count_running_accounts(&self) -> &'static str {
        "目标机器人"
    }

    fn count_restart_verified(&self) -> &'static str {
        "重启复核通过"
    }

    fn count_restart_failed(&self) -> &'static str {
        "重启异常"
    }

    fn count_sent_ok(&self) -> &'static str {
        "已发送"
    }

    fn count_send_failed(&self) -> &'static str {
        "发送失败"
    }

    fn count_processed_successfully(&self) -> &'static str {
        "处理完成"
    }

    fn count_processing_failed(&self) -> &'static str {
        "处理异常"
    }

    fn pauseall_title(&self) -> &'static str {
        "批量暂停钓鱼结果"
    }

    fn unpauseall_title(&self) -> &'static str {
        "批量恢复钓鱼结果"
    }

    fn startall_no_valid_accounts(&self) -> &'static str {
        "没有可启动的机器人"
    }

    fn startall_no_valid_accounts_with_missing(&self, missing_token_count: usize) -> String {
        format!("没有可启动的机器人（{missing_token_count} 个缺少 Token）")
    }

    fn startall_all_running(&self) -> &'static str {
        "已配置 Token 的机器人均存在会话"
    }

    fn startall_title(&self) -> &'static str {
        "全部启动执行结果"
    }

    fn startall_skipped_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 会话已存在")
    }

    fn stopall_title(&self) -> &'static str {
        "全服机器人停止结果"
    }

    fn anti_afk_started(&self) -> &'static str {
        "反 AFK 已启动（不保证已有匹配会话或输入已送达）"
    }

    fn anti_afk_already_running(&self) -> &'static str {
        "反 AFK 已在运行，无需重复启动"
    }

    fn anti_afk_stopped(&self) -> &'static str {
        "反 AFK 已停止"
    }

    fn anti_afk_status(&self, running: bool) -> &'static str {
        if running {
            "反 AFK: 运行中"
        } else {
            "反 AFK: 未运行"
        }
    }

    fn anti_afk_stop_not_running(&self) -> &'static str {
        "错误: 反 AFK 功能未在运行，无需停止"
    }

    fn anti_afk_lock_failed(&self, reason: &str) -> String {
        format!("获取反 AFK 状态锁失败: {reason}")
    }

    fn anti_afk_config_failed(&self, reason: &str) -> String {
        format!("构建反 AFK 配置失败: {reason}")
    }

    fn anti_afk_worker_failed(&self, reason: &str) -> String {
        format!("反 AFK 线程运行失败: {reason}")
    }

    fn anti_afk_start_failed(&self, reason: &str) -> String {
        format!("启动反 AFK 失败: {reason}")
    }

    fn anti_afk_stop_failed(&self, reason: &str) -> String {
        format!("停止反 AFK 失败: {reason}")
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

    // ============ 只读查询（`queries/show_*`） ============
    fn status_title(&self) -> &'static str {
        "机器人运行状态"
    }

    fn status_no_running_bots(&self) -> &'static str {
        "没有已配置 Token 且存在会话或排队的机器人"
    }

    fn status_column_bot_name(&self) -> &'static str {
        "机器人"
    }

    fn status_column_joined(&self) -> &'static str {
        "进服状态"
    }

    fn status_column_pause(&self) -> &'static str {
        "暂停状态"
    }

    fn status_column_no_rod(&self) -> &'static str {
        "无可用鱼竿"
    }

    fn status_column_bobber(&self) -> &'static str {
        "存在鱼漂"
    }

    fn status_column_last_fish(&self) -> &'static str {
        "上次钓鱼动作（Unix 毫秒）"
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
        "未配置管理员，请在配置文件的 [admins] 中添加"
    }

    fn admins_row(&self, name: &str, user_id: &str) -> String {
        format!("{name} : {user_id}")
    }

    fn botconfig_read_failed(&self, path: &str, reason: &str) -> String {
        format!("读取游戏配置 {path} 失败: {reason}")
    }

    fn botconfig_too_large(&self, bytes: usize, max_bytes: usize) -> String {
        format!("错误: 钓鱼配置 {bytes} 字节，超过附件上限 {max_bytes} 字节，无法作为附件发送。")
    }

    // ============ 通用指令（`queries/`） ============
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

    fn ping_no_address_reason(&self) -> &'static str {
        "域名解析未返回地址"
    }

    fn ping_connection_failed_reason(&self) -> &'static str {
        "连接失败"
    }

    fn ping_timeout(&self, host: &str, port: u16, timeout: Duration) -> String {
        let seconds = format_seconds(timeout);
        format!("连接服务器 ({host}:{port}) 超时({seconds} 秒无响应)")
    }

    fn ping_local_target(&self, host: &str, port: u16) -> String {
        format!("拒绝探测本地或私网地址 ({host}:{port})")
    }

    fn ping_protocol_failed(&self, host: &str, port: u16, reason: &str) -> String {
        format!("服务器 ({host}:{port}) 协议解析失败：\n{reason}")
    }

    fn checkserver_title(&self) -> &'static str {
        "钓鱼服务器状态"
    }

    fn checkserver_system_failed(&self, reason: &str) -> String {
        format!("获取系统信息失败: {reason}")
    }

    fn checkserver_disks_failed(&self, reason: &str) -> String {
        format!("获取磁盘信息失败: {reason}")
    }

    fn checkserver_uptime_failed(&self, reason: &str) -> String {
        format!("获取系统运行时间失败: {reason}")
    }

    fn checkserver_cpu_usage(&self, cpu_usage: f32) -> String {
        format!("CPU 占用率: {cpu_usage:.1}%")
    }

    fn checkserver_memory_used_total(&self, used_mem_mb: u64, total_mem_mb: u64) -> String {
        if total_mem_mb >= 1024 {
            format!(
                "已用内存/总内存: {:.1} GB / {:.1} GB",
                used_mem_mb as f64 / 1024.0,
                total_mem_mb as f64 / 1024.0
            )
        } else {
            format!("已用内存/总内存: {used_mem_mb} MB / {total_mem_mb} MB")
        }
    }

    fn checkserver_memory_usage(&self, mem_usage_pct: f64) -> String {
        format!("内存占用率: {mem_usage_pct:.1}%")
    }

    fn checkserver_disk_usage(&self, disk_usage_pct: f64) -> String {
        format!("磁盘占用率: {disk_usage_pct:.1}%")
    }

    fn checkserver_uptime(&self, uptime_text: &str) -> String {
        format!("服务器物理运行时间: {uptime_text}")
    }

    fn checkserver_process_uptime(&self, uptime_text: &str) -> String {
        format!("本次运行时间: {uptime_text}")
    }

    fn checkserver_system_time(&self, time_text: &str) -> String {
        format!("当前系统时间: {time_text}")
    }

    // ============ Discord 指令文案 ============
    fn command_sync_title(&self) -> &'static str {
        "Discord 指令同步结果"
    }

    fn command_sync_registered(&self) -> &'static str {
        "全局指令已更新；新指令传播可能需要最多一小时，服务器旧指令已立即尝试清理"
    }

    fn command_sync_summary(&self, guilds: usize, cleared: usize, failed: usize) -> String {
        format!("已检查 {guilds} 个服务器；清理成功 {cleared} 个，失败 {failed} 个")
    }

    fn command_sync_guild_updated(&self, guild_name: &str) -> String {
        format!("服务器 {guild_name} 的指令已更新")
    }

    fn command_sync_guild_failed(&self, guild_name: &str, reason: &str) -> String {
        format!("服务器 {guild_name} 的指令更新失败: {reason}")
    }

    fn command_sync_bootstrap_failed(&self, guild_name: &str, reason: &str) -> String {
        format!("服务器 {guild_name} 的 /sync-commands 入口注册失败: {reason}")
    }

    fn command_sync_guild_summary(&self, guilds: usize, updated: usize, failures: usize) -> String {
        format!("目标服务器 {guilds} 个；更新成功 {updated} 个，更新失败 {failures} 个")
    }

    fn command_sync_cleanup_skipped(&self) -> &'static str {
        "同步未完成，未继续清理旧范围；修复后重新执行 /sync-commands"
    }

    fn command_sync_global_cleared(&self, count: usize) -> String {
        format!("已清除 {count} 条旧全局指令")
    }

    fn command_sync_global_clear_failed(&self, reason: &str) -> String {
        format!("清除旧全局指令失败: {reason}")
    }

    fn command_sync_failed(&self, reason: &str) -> String {
        format!("全局指令更新失败，未清理服务器旧指令: {reason}")
    }

    fn command_sync_in_progress(&self) -> &'static str {
        "已有指令同步正在执行，请稍后重试"
    }

    fn command_started_notice(&self, command_tag: &str) -> String {
        format!("已开始 {command_tag}，完成后将返回结果")
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
            "已接受 /startall 请求 : 本次请求启动 {bot_count} 台，其中启动队列中已有 {queued_bot_count} 台在等待；\
             预计耗时 {estimated_minutes} 分钟，等待启动结果的超时时间为 {timeout_minutes} 分钟，完成后将返回完整结果"
        )
    }

    fn start_internal_error_reason(&self) -> &'static str {
        "内部错误（见程序日志）"
    }

    fn start_queue_full_reason(&self) -> &'static str {
        "启动队列已满"
    }

    fn command_start_queue_summary(
        &self,
        batch_size: usize,
        bot_login_interval: Duration,
        batch_login_interval: Duration,
    ) -> String {
        let bot_login_interval = format_seconds(bot_login_interval);
        let batch_login_interval = format_seconds(batch_login_interval);
        format!(
            "启动队列设置: 每批最多启动 {batch_size} 台，批内间隔 {bot_login_interval} 秒，批间隔 {batch_login_interval} 秒"
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

    // ============ 权限与共享的拼接片段 ============
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
        format!("错误: 未找到机器人 '{clean_bot_name}' 对应的目录，或你没有权限访问")
    }

    fn bot_add_name_taken(&self, bot_name: &str) -> String {
        format!("错误: 机器人名 '{bot_name}' 已被占用")
    }

    fn create_bot_dir_failed(&self, reason: &str) -> String {
        format!("错误: 创建机器人目录失败: {reason}")
    }

    fn read_bot_list_failed(&self, reason: &str) -> String {
        format!("错误: 读取机器人列表失败: {reason}")
    }

    fn read_bots_root_failed(&self, reason: &str) -> String {
        format!("错误: 读取机器人根目录失败: {reason}")
    }

    fn screen_list_failed(&self, reason: &str) -> String {
        format!("查询 screen 会话失败: {reason}")
    }

    fn screen_unrecognized_output(&self, output: &str) -> String {
        format!("无法识别的输出: {output}")
    }

    fn refusal_bot_running(&self, bot_name: &str) -> String {
        format!("机器人 '{bot_name}' 存在 screen 会话，请先停止")
    }

    fn refusal_bot_queued(&self, bot_name: &str) -> String {
        format!("错误: 机器人 '{bot_name}' 正在启动队列中排队，请先取消它的排队启动")
    }

    fn send_command_failed(&self, bot_name: &str, reason: Option<&str>) -> String {
        match reason {
            Some(reason) => format!("错误: 向机器人 '{bot_name}' 发送指令失败: {reason}"),
            None => format!("错误: 向机器人 '{bot_name}' 发送指令失败"),
        }
    }

    fn stop_signal_failed(&self, reason: &str) -> String {
        format!("错误: 发送停止信号失败: {reason}")
    }

    fn force_quit_failed(&self, reason: &str) -> String {
        format!("错误: 强制终止机器人失败: {reason}")
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

    fn start_join_confirmed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 已确认进服")
    }

    fn start_not_joined(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 会话已存在，当前未进服（稍后用 /show status 查看）")
    }

    fn start_failed(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 启动失败")
    }

    fn start_cancelled(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 启动被取消")
    }

    fn start_result_timeout(&self, bot_name: &str, timeout: Duration) -> String {
        let seconds = format_seconds(timeout);
        format!(
            "机器人 {bot_name}: 等待启动结果超时（{seconds} 秒），状态未确认；请检查日志或 /show status"
        )
    }

    fn restart_skipped_missing_token(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name}: 缺少 Token，已跳过重启（如需停止请使用 /stop）")
    }

    fn restart_not_running(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 没有 screen 会话，无需重启")
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

    fn command_log_lines_description(&self, default_lines: usize) -> String {
        format!("读取最新日志行数 (默认 {default_lines}，上限见配置 runtime.max_reply_log_lines)")
    }

    fn command_text_invalid(&self, path: &str, problem: CommandTextProblem) -> String {
        let reason = match problem {
            CommandTextProblem::Missing => "缺少说明".to_string(),
            CommandTextProblem::Duplicate => "说明重复".to_string(),
            CommandTextProblem::Unknown => "不存在对应的指令或选项".to_string(),
            CommandTextProblem::InvalidLength { length, max } => {
                format!("说明必须为 1-{max} 个 UTF-16 码元且不能全为空白，当前长度为 {length}")
            }
        };
        format!("指令文案 {path} 无效: {reason}")
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
}

impl LogMessages for ZhCn {
    fn process_log_first_line(&self) -> String {
        "--- 正在启动 Minecraft FishingBot Manager for Discord 进程 ---".to_string()
    }

    fn process_log_path(&self, path: &str) -> String {
        format!("本次程序日志存储于: {path}")
    }

    fn process_log_install_failed(&self, reason: &str) -> String {
        format!("日志初始化失败： {reason}")
    }

    fn process_log_already_installed(&self) -> String {
        "日志已经装过一次，本次保留已装的日志系统。".to_string()
    }

    fn process_log_output_failed(
        &self,
        destination: &str,
        operation: &str,
        reason: &str,
    ) -> String {
        format!("警告：程序日志 {destination} {operation} 失败：{reason}")
    }

    fn config_loaded(&self, path: &str) -> String {
        format!("已加载配置: {path}")
    }

    fn process_id(&self, pid: u32) -> String {
        format!("进程 PID: {pid}")
    }

    fn discord_connected(&self, application_id: u64) -> String {
        format!("已连接 Discord (应用 ID {application_id})")
    }

    fn invite_url(&self, url: &str) -> String {
        format!("邀请链接: {url}")
    }

    fn shutdown_signal(&self) -> String {
        "收到退出信号，正在停止全部机器人 (请勿重复按 Ctrl+C)……".to_string()
    }

    fn probe_failed(&self, reason: &str) -> String {
        format!("REST 探活失败: {reason}")
    }

    fn watchdog_ready(
        &self,
        window: Duration,
        probe_interval: Duration,
        first_handshake_timeout: Duration,
    ) -> String {
        format!(
            "网关看门狗已就绪：判定窗口 {} 秒、探活间隔 {} 秒、首次握手超时 {} 秒",
            format_seconds(window),
            format_seconds(probe_interval),
            format_seconds(first_handshake_timeout)
        )
    }

    fn command_run_failed(&self, error: &str) -> String {
        format!("指令执行出错: {error}")
    }

    fn command_fallback_reply_failed(
        &self,
        caller_id: u64,
        command_path: &str,
        reason: &str,
    ) -> String {
        format!("向 Discord 用户 {caller_id} 发送 /{command_path} 兜底回复失败: {reason}")
    }

    fn autocomplete_lookup_failed(&self, reason: &str) -> String {
        format!("自动候选查询失败: {reason}")
    }

    fn autocomplete_task_failed(&self, reason: &str) -> String {
        format!("自动候选查询任务失败: {reason}")
    }

    fn command_replied(&self, caller_id: u64, command_path: &str, reply: &str) -> String {
        format!("回复 /{command_path} (调用者 {caller_id}): {reply}")
    }

    fn report_posted_to_channel(&self, caller_id: u64, command_path: &str, reason: &str) -> String {
        format!(
            "回复 /{command_path} (调用者 {caller_id}): 交互令牌已过期（{reason}），发送频道消息"
        )
    }

    fn framework_error(&self, detail: &str) -> String {
        format!("框架错误: {detail}")
    }

    fn sigterm_register_failed(&self, reason: &str) -> String {
        format!("注册 SIGTERM 处理失败: {reason}")
    }

    fn gateway_lost_never_ready(&self) -> String {
        "未能在启动后与 Discord 完成握手（可能需要在客户端启用 TUN 模式代理）；正在停止全部机器人，随后尝试重建连接……".to_string()
    }

    fn gateway_lost_unreachable(&self) -> String {
        "已经连续一个判定窗口无法访问 Discord，正在停止全部机器人，随后尝试重建连接……".to_string()
    }

    fn reconnect_scheduled(&self, attempt: u32, delay: Duration) -> String {
        format!(
            "第 {attempt} 次重建连接将于 {} 秒后开始",
            format_seconds(delay)
        )
    }

    fn reconnect_failed(&self, attempt: u32, delay: Duration, reason: &str) -> String {
        format!(
            "第 {attempt} 次重建连接失败: {reason}，将于 {} 秒后重试",
            format_seconds(delay)
        )
    }

    fn reconnected_after_offline(
        &self,
        offline_secs: u64,
        notified_guilds: usize,
        failed_guilds: usize,
    ) -> String {
        format!(
            "已重新连接 Discord (期间掉线 {offline_secs} 秒)；恢复通知已发往 {notified_guilds} 个服务器 (发送失败 {failed_guilds} 次)"
        )
    }

    fn reconnect_notice_failed(&self, guild_name: &str, reason: &str) -> String {
        format!("向服务器 {guild_name} 发掉线恢复通知失败: {reason}")
    }

    fn reconnect_notice_no_channel(&self) -> String {
        "此服务器既没有系统频道、也没有文字频道".to_string()
    }

    fn reconnect_notice_body(&self, offline_since: &str, offline_secs: u64) -> String {
        format!(
            "⚠️ 本机器人于 {offline_since} 起与 Discord 网络连接中断，目前已自动恢复(掉线 {offline_secs} 秒)。\
                 掉线期间已停止全部游戏机器人"
        )
    }

    fn discord_init_failed(&self, error: &str) -> String {
        format!("Discord 初始化失败: {error}")
    }

    fn discord_init_failed_not_invited(&self, error: &str) -> String {
        format!(
            "Discord 初始化失败: {error} [可能是机器人还没被邀请进服务器，或邀请时缺少 \
                 applications.commands 权限：你可以用启动日志中的邀请链接再邀请一次]"
        )
    }

    fn discord_init_failed_bad_token(&self, error: &str) -> String {
        format!(
            "Discord 初始化失败: {error} [可能是 discord.token 不对或已被 Reset：\
                 开发者面板 → 你的应用 → Bot → Reset Token，修改为新 token 再重启本进程]"
        )
    }

    fn guild_commands_cleared(&self, guild_name: &str, count: usize) -> String {
        format!("已清除服务器 {guild_name} 上遗留的 {count} 条 guild 指令")
    }

    fn guild_commands_clear_failed(&self, guild_name: &str, reason: &str) -> String {
        format!(
            "清除服务器 {guild_name} 上遗留的 guild 指令失败: {reason} (请再次执行 /sync-commands)"
        )
    }

    fn discord_client_ended(&self) -> String {
        "与 Discord 的连接已结束 (客户端正常返回)".to_string()
    }

    fn discord_client_error(&self, error: &str) -> String {
        format!("与 Discord 的连接发送错误: {error}")
    }

    fn shutdown_report_title(&self) -> String {
        "进程退出报告".to_string()
    }

    fn shutdown_signal_report_title(&self) -> String {
        "收到来自系统的退出信号，停止结果".to_string()
    }

    fn shutdown_report_notice_failed(&self, guild_name: &str, reason: &str) -> String {
        format!("向服务器 {guild_name} 发退出收尾报告失败: {reason}")
    }

    fn shutdown_report_notified(&self, notified_guilds: usize, failed_guilds: usize) -> String {
        format!("退出收尾报告已发往 {notified_guilds} 个服务器(发送失败 {failed_guilds} 次)")
    }

    fn shutdown_report_no_context(&self) -> String {
        "本次连接未完成握手，没有可用的 Discord 上下文；退出收尾报告仅写入程序日志".to_string()
    }

    fn shutdown_report_timeout(&self, seconds: u64) -> String {
        format!("退出收尾报告 {seconds} 秒内没发完，跳过剩余发送并退出 (下次启动不补发)")
    }

    fn bot_restart_requested(&self, bot_name: &str) -> String {
        format!("已接收到机器人 {bot_name} 的重启命令")
    }

    fn bot_stopped(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 已成功停止")
    }

    fn bot_added(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 创建成功")
    }

    fn bot_add_failed_log_message(&self, raw_bot_name: &str, reason: &str) -> String {
        format!("添加机器人 {raw_bot_name} 失败: {reason}")
    }

    fn bot_delete_failed_log_message(&self, bot_name: &str, reason: &str) -> String {
        format!("删除机器人 {bot_name} 失败: {reason}")
    }

    fn bot_deleted(&self, bot_name: &str) -> String {
        format!("成功删除机器人 {bot_name} ")
    }

    fn bot_log_first_line(&self) -> String {
        "--- 启动机器人 ---".to_string()
    }

    fn start_log_file_failed_log_message(&self, bot_name: &str, reason: &str) -> String {
        format!("机器人 {bot_name} 的日志文件创建失败: {reason}")
    }

    fn start_session_launched(&self, bot_name: &str, session_name: &str, log_path: &str) -> String {
        format!("尝试启动机器人 {bot_name} ， Screen 会话 ({session_name})，日志: {log_path}")
    }

    fn start_launch_panicked(&self, bot_name: &str, reason: &str) -> String {
        format!("拉起机器人 {bot_name} 时发生内部错误：{reason}")
    }

    fn start_result_timeout_log_message(&self, bot_name: &str, timeout: Duration) -> String {
        format!(
            "等待机器人 {bot_name} 的启动结果超时（{} 秒），已按未确认处理",
            format_seconds(timeout)
        )
    }

    fn starter_restarting(&self, reason: &str) -> String {
        format!("启动线程异常退出（{reason}），正在重启它")
    }

    fn start_verification_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("机器人 {bot_name} 启动复核未通过: {reason}")
    }

    fn unexpected_game_log_file(&self, dir: &str, file_name: &str) -> String {
        format!(
            "FishingBot 日志目录 {dir} 内存在意料之外的文件 '{file_name}' (若反复出现说明 FishingBot 更改了日志文件名，\
                 请把配置 [raw_game_logs] 的 file_name 更新成这个名字)"
        )
    }

    fn game_logs_dir_remove_failed(&self, dir: &str, reason: &str) -> String {
        format!("清空 FishingBot 原始日志目录 {dir} 失败: {reason}")
    }

    fn game_logs_dir_create_failed(&self, dir: &str, reason: &str) -> String {
        format!("创建 FishingBot 原始日志目录 {dir} 失败: {reason} (钓鱼机器人可能因此拒绝启动)")
    }

    fn game_log_discard_failed(&self, file: &str, device: &str, reason: &str) -> String {
        format!("将 FishingBot 的原始日志文件 {file} 指向 {device} 失败: {reason}")
    }

    fn bot_login_rejected_log_message(&self, bot_name: &str) -> String {
        format!("机器人 {bot_name} 的登录被微软拒绝(未能创建凭据)")
    }

    fn bot_login_stop_failed(&self, bot_name: &str, reason: &str) -> String {
        format!("停止机器人 {bot_name} 的登录会话失败: {reason}")
    }

    fn bot_login_old_token_remove_failed(&self, path: &str, reason: &str) -> String {
        format!("清理旧的凭据文件 {path} 失败: {reason}")
    }

    fn bot_login_old_token_restore_failed(&self, path: &str, reason: &str) -> String {
        format!("把旧凭据文件 {path} 放回原位失败: {reason}")
    }

    fn bot_login_chmod_failed(&self, path: &str, reason: &str) -> String {
        format!("把凭据文件 {path} 的权限改成 0600 失败: {reason}")
    }

    fn fishingbot_config_removed(&self, path: &str) -> String {
        format!("已删除旧的游戏侧配置文件 {path}")
    }

    fn fishingbot_config_written(&self, path: &str) -> String {
        format!("已生成游戏侧配置文件 {path}")
    }

    fn create_log_dir_failed(&self, dir: &str, reason: &str) -> String {
        format!("创建日志目录 {dir} 失败: {reason}")
    }

    fn create_log_file_failed(&self, path: &str, reason: &str) -> String {
        format!("创建日志文件 {path} 失败: {reason}")
    }

    fn write_log_first_line_failed(&self, path: &str, reason: &str) -> String {
        format!("写入日志首行 {path} 失败: {reason}")
    }

    fn open_log_file_failed(&self, reason: &str) -> String {
        format!("无法打开日志文件: {reason}")
    }

    fn read_log_metadata_failed(&self, reason: &str) -> String {
        format!("读取日志文件信息失败: {reason}")
    }

    fn seek_log_file_failed(&self, reason: &str) -> String {
        format!("定位日志文件失败: {reason}")
    }

    fn read_log_content_failed(&self, reason: &str) -> String {
        format!("读取日志内容失败: {reason}")
    }
}
