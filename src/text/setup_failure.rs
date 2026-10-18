//! 根据 Discord 错误文本分类装配失败，并判断是否可重试。

use super::LogText;

/// 把不同错误映射为对应的日志文案，无法识别时保留原始错误。
pub fn setup_failure_log_text(error: &str) -> LogText<'_> {
    // 机器人未加入任何服务器（或邀请时缺少 `applications.commands` 权限）。
    const NOT_INVITED_PHRASES: [&str; 2] = ["Missing Access", "Unknown Guild"];
    // Token 无效。
    const BAD_TOKEN_PHRASES: [&str; 3] = ["Unauthorized", "Invalid Token", "401"];

    if NOT_INVITED_PHRASES
        .iter()
        .any(|phrase| error.contains(phrase))
    {
        LogText::DiscordInitFailedNotInvited { error }
    } else if BAD_TOKEN_PHRASES
        .iter()
        .any(|phrase| error.contains(phrase))
    {
        LogText::DiscordInitFailedBadToken { error }
    } else {
        LogText::DiscordInitFailed { error }
    }
}

/// 根据错误，判断错误是否是永久性错误
pub fn failure_is_permanent(failure: &LogText<'_>) -> bool {
    matches!(
        failure,
        LogText::DiscordInitFailedNotInvited { .. } | LogText::DiscordInitFailedBadToken { .. }
    )
}
