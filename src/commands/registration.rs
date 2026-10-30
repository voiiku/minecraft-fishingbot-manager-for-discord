//! 全局指令最长需要一小时传播；稳定后清理遗留 guild 指令，未稳定时后台重试。

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;

use minecraft_fishingbot_manager_for_discord::text;

use super::{Data, Error};

/// 超过这个时间则认为用户已经能看到全局指令。
const GLOBAL_PROPAGATION_MS: u64 = 60 * 60 * 1000;

/// Discord 时间起点（2015-01-01T00:00:00Z），单位毫秒。
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// 注册全局指令，并在传播完成后清除各服务器上遗留的 guild 指令。
pub(crate) async fn register_commands(
    ctx: &serenity::Context,
    commands: &[poise::Command<Data, Error>],
    guilds: &[serenity::UnavailableGuild],
) -> Result<(), Error> {
    let registered = serenity::Command::get_global_commands(ctx).await;
    poise::builtins::register_globally(ctx, commands).await?;

    let wanted_names = command_names(commands);

    match &registered {
        Ok(registered) => {
            let now = now_ms();

            if global_commands_settled(registered, commands, now) {
                clear_guild_scope_leftovers(ctx, guilds).await;
            } else {
                let wait_minutes = minutes_until_cleanup(registered, commands, now);
                log::info!(
                    "{}",
                    text::get().log_text(text::LogText::GuildCommandsCleanupWaiting {
                        minutes: wait_minutes,
                    })
                );
                schedule_guild_cleanup(ctx.clone(), guilds.to_vec(), wanted_names, wait_minutes);
            }
        }
        Err(error) => {
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::GlobalCommandsReadFailed {
                    reason: &error.to_string(),
                })
            );
            schedule_guild_cleanup(
                ctx.clone(),
                guilds.to_vec(),
                wanted_names,
                full_propagation_minutes(),
            );
        }
    }

    Ok(())
}

/// 判断全局指令是否已经注册完成：名称与数量一致，且最新一条已满传播窗口。
/// 已存在的指令不会被重建，指令 ID 中编码的创建时间可用于判断其存在时长。
fn global_commands_settled(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
    now_ms: u64,
) -> bool {
    same_command_names(registered, commands) && propagation_wait_minutes(registered, now_ms) == 0
}

/// 计算清理遗留指令前的等待分钟数。
fn minutes_until_cleanup(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
    now_ms: u64,
) -> u64 {
    minutes_until_cleanup_by_names(registered, &command_names(commands), now_ms)
}

fn minutes_until_cleanup_by_names(
    registered: &[serenity::Command],
    wanted_names: &BTreeSet<String>,
    now_ms: u64,
) -> u64 {
    if same_command_names_by_names(registered, wanted_names) {
        propagation_wait_minutes(registered, now_ms)
    } else {
        full_propagation_minutes()
    }
}

/// 计算当前全局指令距离满足传播窗口还需等待的分钟数；不足 1 分钟按 1 分钟计。
fn propagation_wait_minutes(registered: &[serenity::Command], now_ms: u64) -> u64 {
    let youngest = registered
        .iter()
        .map(|command| created_at_ms(command.id))
        .max()
        .unwrap_or(now_ms);

    GLOBAL_PROPAGATION_MS
        .saturating_sub(now_ms.saturating_sub(youngest))
        .div_ceil(60 * 1000)
}

/// 将传播窗口换算为分钟（`GLOBAL_PROPAGATION_MS` 的唯一换算点）。
fn full_propagation_minutes() -> u64 {
    GLOBAL_PROPAGATION_MS.div_ceil(60 * 1000)
}

/// 本次启动尚未满足清理条件时，在后台等待传播完成后重试，避免依赖下一次重启。
fn schedule_guild_cleanup(
    ctx: serenity::Context,
    guilds: Vec<serenity::UnavailableGuild>,
    wanted_names: BTreeSet<String>,
    wait_minutes: u64,
) {
    tokio::spawn(async move {
        let mut wait = Duration::from_secs(wait_minutes.saturating_mul(60));

        loop {
            tokio::time::sleep(wait).await;
            let now = now_ms();

            match serenity::Command::get_global_commands(&ctx).await {
                Ok(registered) if same_command_names_by_names(&registered, &wanted_names) => {
                    let remaining = propagation_wait_minutes(&registered, now);
                    if remaining == 0 {
                        clear_guild_scope_leftovers(&ctx, &guilds).await;
                        return;
                    }
                    wait = Duration::from_secs(remaining.saturating_mul(60));
                }
                Ok(registered) => {
                    wait = Duration::from_secs(
                        minutes_until_cleanup_by_names(&registered, &wanted_names, now)
                            .saturating_mul(60),
                    );
                }
                Err(error) => {
                    log::warn!(
                        "{}",
                        text::get().log_text(text::LogText::GlobalCommandsReadFailed {
                            reason: &error.to_string(),
                        })
                    );
                    wait = Duration::from_secs(full_propagation_minutes().saturating_mul(60));
                }
            }
        }
    });
}

/// 当前实现期望 Discord 上存在的指令名集合。
fn command_names(commands: &[poise::Command<Data, Error>]) -> BTreeSet<String> {
    commands
        .iter()
        .map(|command| command.name.to_string())
        .collect()
}

/// 判断已注册的全局指令与本次要注册的指令是否一致（数量与名称均需相同）。
fn same_command_names(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
) -> bool {
    same_command_names_by_names(registered, &command_names(commands))
}

fn same_command_names_by_names(
    registered: &[serenity::Command],
    wanted_names: &BTreeSet<String>,
) -> bool {
    let only_slash_commands = registered
        .iter()
        .all(|command| command.kind == serenity::CommandType::ChatInput);
    let registered_names: BTreeSet<String> = registered
        .iter()
        .map(|command| command.name.clone())
        .collect();

    only_slash_commands
        && registered.len() == wanted_names.len()
        && registered_names == *wanted_names
}

/// 指令 Id 中编码的创建时间（Unix 毫秒），高 42 位是相对 Discord 纪元的毫秒数。
fn created_at_ms(id: serenity::CommandId) -> u64 {
    (id.get() >> 22) + DISCORD_EPOCH_MS
}

/// 当前时间（Unix 毫秒）；时钟回退时取 0，于是"稳定"判定为假，本轮不清理。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_millis() as u64)
        .unwrap_or_default()
}

/// 清除各服务器上遗留的 guild 指令。
/// 已无残留时不发写请求；失败记录 WARN，由后续启动或重试任务处理。
async fn clear_guild_scope_leftovers(
    ctx: &serenity::Context,
    guilds: &[serenity::UnavailableGuild],
) {
    for guild_id in guild_ids_for_cleanup(guilds) {
        let guild_name = guild_name(ctx, guild_id);

        match left_commands(ctx, guild_id).await {
            Ok(0) => {}
            Ok(count) => match guild_id.set_commands(ctx, Vec::new()).await {
                Ok(_) => log::info!(
                    "{}",
                    text::get().log_text(text::LogText::GuildCommandsCleared {
                        guild_name: &guild_name,
                        count,
                    })
                ),
                Err(error) => log_when_clear_failed(&guild_name, &error.to_string()),
            },
            Err(error) => log_when_clear_failed(&guild_name, &error.to_string()),
        }
    }
}

/// `Ready` 事件中的 guild 条目通常标记为 unavailable；清理仍应使用其 ID。
fn guild_ids_for_cleanup(guilds: &[serenity::UnavailableGuild]) -> Vec<serenity::GuildId> {
    guilds.iter().map(|guild| guild.id).collect()
}

/// 返回服务器上现有 guild 指令数量；读取失败时返回错误文本，供调用方记录日志。
async fn left_commands(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<usize, String> {
    guild_id
        .get_commands(ctx)
        .await
        .map(|commands| commands.len())
        .map_err(|error| error.to_string())
}

/// 记录一次清理失败的 WARN 日志。
fn log_when_clear_failed(guild_name: &str, reason: &str) {
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::GuildCommandsClearFailed { guild_name, reason })
    );
}

/// 返回服务器名称；缓存缺失时返回服务器 ID。
fn guild_name(ctx: &serenity::Context, guild_id: serenity::GuildId) -> String {
    ctx.cache
        .guild(guild_id)
        .map(|guild| guild.name.clone())
        .unwrap_or_else(|| guild_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Snowflake ID 中编码的创建时间必须能还原为 Unix 毫秒。
    #[test]
    fn snowflakes_decode_to_their_creation_time() {
        let created_ms = DISCORD_EPOCH_MS + 1_600_000_000_000;

        assert_eq!(created_at_ms(command_id(created_ms)), created_ms);
        // 雪票低位不是时间，因此最小 Id 落在纪元那一毫秒上。
        assert_eq!(created_at_ms(serenity::CommandId::new(1)), DISCORD_EPOCH_MS);
    }

    /// 只有指令名称完全匹配，且最年轻的一条也超过传播窗口时，才允许清理残留。
    #[test]
    fn leftovers_are_only_cleared_after_the_global_set_settled() {
        let commands = super::super::declared_commands();
        let names: Vec<&str> = commands
            .iter()
            .map(|command| command.name.as_ref())
            .collect();
        let now = DISCORD_EPOCH_MS + 1_600_000_000_000;
        let settled_at = now - 2 * GLOBAL_PROPAGATION_MS;

        // 全部两小时前创建，且名字一一对应。
        assert!(global_commands_settled(
            &registered(&names, settled_at),
            &commands,
            now
        ));

        // 最年轻的一条是 5 分钟前创建的：这套指令刚变过，客户端可能还没跟上。
        let mut fresh = registered(&names, settled_at);
        fresh[0] = registered_command(names[0], now - 5 * 60 * 1000);
        assert!(!global_commands_settled(&fresh, &commands, now));

        // 少一条：说明现有集合不是这一套。
        assert!(!global_commands_settled(
            &registered(&names[1..], settled_at),
            &commands,
            now
        ));

        // 一条都没有：还没注册过全局指令。
        assert!(!global_commands_settled(&[], &commands, now));
    }

    /// 等待时间：名称不匹配时按完整传播窗口计算；否则按最年轻指令的剩余时间计算。
    #[test]
    fn waiting_time_counts_down_from_the_youngest_command() {
        let commands = super::super::declared_commands();
        let names: Vec<&str> = commands
            .iter()
            .map(|command| command.name.as_ref())
            .collect();
        let now = DISCORD_EPOCH_MS + 1_600_000_000_000;
        let hour = GLOBAL_PROPAGATION_MS;

        // 全部超过传播窗口且名称一致：无需等待。
        assert_eq!(
            minutes_until_cleanup(&registered(&names, now - hour), &commands, now),
            0
        );

        // 最年轻的一条创建于 5 分钟前：剩余 55 分钟。
        let mut young = registered(&names, now - 2 * hour);
        young[0] = registered_command(names[0], now - 5 * 60 * 1000);
        assert_eq!(minutes_until_cleanup(&young, &commands, now), 55);

        // 名称不匹配（本轮刚修改）：等待完整传播窗口。
        assert_eq!(
            minutes_until_cleanup(&registered(&names[1..], now - 2 * hour), &commands, now),
            full_propagation_minutes()
        );

        // 不足 1 分钟时向上取整为 1 分钟，避免显示 0 分钟但仍不清理。
        assert_eq!(
            propagation_wait_minutes(&registered(&names, now - hour + 30 * 1000), now),
            1
        );
    }

    /// 标记为 unavailable 的 guild 也必须纳入残留指令清理。
    #[test]
    fn unavailable_guilds_are_scheduled_for_cleanup() {
        let json = "{\"id\":\"123456789012345678\",\"unavailable\":true}";
        let guild: serenity::UnavailableGuild =
            serde_json::from_str(json).expect("Ready 里的 guild 形状应当能解析");

        assert_eq!(
            guild_ids_for_cleanup(&[guild]),
            vec![serenity::GuildId::new(123_456_789_012_345_678)]
        );
    }

    /// 构造 Discord 侧现存的全局指令列表，字段形状与 `GET /applications/<app>/commands` 一致。
    fn registered(names: &[&str], created_ms: u64) -> Vec<serenity::Command> {
        names
            .iter()
            .map(|name| registered_command(name, created_ms))
            .collect()
    }

    /// 构造一条 Discord 侧现存的全局指令。
    fn registered_command(name: &str, created_ms: u64) -> serenity::Command {
        let id = command_id(created_ms);

        serde_json::from_str(&format!(
            r#"{{
                "id": "{id}",
                "application_id": "1",
                "guild_id": null,
                "name": "{name}",
                "name_localized": null,
                "name_localizations": null,
                "description": "d",
                "description_localized": null,
                "description_localizations": null,
                "default_member_permissions": null,
                "contexts": null,
                "version": "1",
                "handler": null,
                "type": 1
            }}"#
        ))
        .expect("指令的 JSON 形状应当能解析")
    }

    /// 根据创建时间构造 Snowflake ID：保留毫秒时间并左移 22 位。
    fn command_id(created_ms: u64) -> serenity::CommandId {
        serenity::CommandId::new((created_ms - DISCORD_EPOCH_MS) << 22)
    }
}
