//! 根据 Discord 错误文本分类装配失败，并判断是否可重试。

use super::LogText;

/// 将装配错误映射为日志文案；无法识别时保留原始错误。
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

/// 判断装配失败是否不可重试。
pub fn failure_is_permanent(failure: &LogText<'_>) -> bool {
    matches!(
        failure,
        LogText::DiscordInitFailedNotInvited { .. } | LogText::DiscordInitFailedBadToken { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按错误文本分类装配失败。
    #[test]
    fn setup_failures_get_an_actionable_hint() {
        for not_invited in ["Missing Access", "Unknown Guild"] {
            assert!(
                matches!(
                    setup_failure_log_text(not_invited),
                    LogText::DiscordInitFailedNotInvited { .. }
                ),
                "{not_invited} 应当判成「没被邀请进服务器」"
            );
        }

        for bad_token in ["Unauthorized", "Invalid Token", "HTTP 401"] {
            assert!(
                matches!(
                    setup_failure_log_text(bad_token),
                    LogText::DiscordInitFailedBadToken { .. }
                ),
                "{bad_token} 应当判成「token 不对」"
            );
        }

        assert!(matches!(
            setup_failure_log_text("something else went wrong"),
            LogText::DiscordInitFailed { .. }
        ));
    }

    /// 只有配置错误不可重试，其他错误均可重试。
    #[test]
    fn only_configuration_failures_are_permanent() {
        assert!(failure_is_permanent(
            &LogText::DiscordInitFailedNotInvited {
                error: "Missing Access"
            }
        ));
        assert!(failure_is_permanent(&LogText::DiscordInitFailedBadToken {
            error: "401"
        }));
        assert!(!failure_is_permanent(&LogText::DiscordInitFailed {
            error: "connection refused"
        }));

        // 与"按成因分文案"那一层对得上：这两个短语分别落到两种永久错上。
        assert!(failure_is_permanent(&setup_failure_log_text(
            "Unknown Guild"
        )));
        assert!(!failure_is_permanent(&setup_failure_log_text(
            "connection refused"
        )));
    }
}
