//! FishingBot 状态日志的基础解析。

/// `/status` 返回的完整状态；不可用时没有此结构体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotStatus {
    pub paused: bool,
    pub joined: bool,
    pub no_rod_available: bool,
    pub has_bobber: bool,
    /// 上次钓鱼动作的 Unix 时间戳。
    pub last_fish: i64,
}

pub(crate) fn jar_log_message(line: &str) -> Option<&str> {
    let (_, after_timestamp) = line.strip_prefix('[')?.split_once(']')?;
    let (level_suffix, message) = after_timestamp.strip_prefix(" [INFO")?.split_once(']')?;
    level_suffix.trim().is_empty().then_some(message.trim())
}

pub(crate) fn parse_bot_status(message: &str, prefix: &str) -> Option<BotStatus> {
    let mut fields = message.strip_prefix(prefix)?.split_whitespace();
    let status = BotStatus {
        paused: fields.next()?.strip_prefix("paused=")?.parse().ok()?,
        joined: fields.next()?.strip_prefix("joined=")?.parse().ok()?,
        no_rod_available: fields
            .next()?
            .strip_prefix("no_rod_available=")?
            .parse()
            .ok()?,
        has_bobber: fields.next()?.strip_prefix("has_bobber=")?.parse().ok()?,
        last_fish: fields.next()?.strip_prefix("last_fish=")?.parse().ok()?,
    };
    fields.next().is_none().then_some(status)
}
