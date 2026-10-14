//! `/commandall`：把同一条游戏内指令发给所有运行中的机器人（仅管理员）。

use crate::text;
use crate::utils::{Report, ToCleanBotCommand, all_running_bot_names, send_command_to_single_bot};

/// 向所有运行中的机器人发送同一条指令；权限由路由层检查。
pub(crate) fn send_command_to_all_bots(raw_bot_command_args: &[&str]) -> Result<String, String> {
    let bot_command = raw_bot_command_args.to_clean_bot_command()?;

    // 只向已配置 Token 且正在运行的机器人发送。
    let bot_names = all_running_bot_names()?;
    let running_bot_count = bot_names.len();

    let mut succeeded_bot_count = 0usize;
    let mut failed_messages: Vec<String> = Vec::new();
    for bot_name in bot_names {
        match send_command_to_single_bot(&bot_name, &bot_command) {
            Ok(_) => succeeded_bot_count += 1,
            Err(message) => failed_messages.push(message),
        }
    }

    let i18n_text = text::get();
    let mut report = Report::new();
    report.set_title(i18n_text.commandall_title());
    report.set_counts(&[
        (i18n_text.count_running_accounts(), running_bot_count),
        (i18n_text.count_sent_ok(), succeeded_bot_count),
        (i18n_text.count_send_failed(), failed_messages.len()),
    ]);
    // 报告中附上发送的指令。
    report
        .basics
        .insert(1, i18n_text.commandall_sending(&bot_command));
    report.failed = failed_messages;

    Ok(report.render())
}
