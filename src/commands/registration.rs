//! 注册斜杠指令，全局注册，并在确认安全之后清理Discord群聊中上遗留的 guild 指令。
//!
//! 残留：本程序以前可以按配置把指令注册到某一个服务器，那份 guild 指令落在 Discord 上不会自己
//! 消失；客户端把"全局指令 ∪ 该服务器的 guild 指令"一起列出来，于是同一条指令出现两次。
//! 注册成功只代表 Discord 的 API 完成了注册，但客户端最慢要 1 小时才能看到指令。
//! 所以当需要注册指令时，必须等待新指令传播完，旧注册指令超过1小时才能清理旧指令。

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;

use minecraft_fishingbot_manager_for_discord::text;

use super::{Data, Error};

/// 超过这个时间则认为用户已经能看到全局指令。
const GLOBAL_PROPAGATION_MS: u64 = 60 * 60 * 1000;

/// Discord 时间起点（2015-01-01T00:00:00Z），单位毫秒。
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// 注册指令，并在确认安全时清掉各服务器上遗留的 guild 指令。
pub(crate) async fn register_commands(
    ctx: &serenity::Context,
    commands: &[poise::Command<Data, Error>],
    guilds: &[serenity::UnavailableGuild],
) -> Result<(), Error> {
    let registered = serenity::Command::get_global_commands(ctx).await;
    poise::builtins::register_globally(ctx, commands).await?;

    match &registered {
        Ok(registered) => {
            let now = now_ms();

            if global_commands_settled(registered, commands, now) {
                clear_guild_scope_leftovers(ctx, guilds).await;
            } else {
                // 说明此次不清理的原因（等待下次清理）。
                log::info!(
                    "{}",
                    text::get().log_text(text::LogText::GuildCommandsCleanupWaiting {
                        minutes: minutes_until_cleanup(registered, commands, now),
                    })
                );
            }
        }
        Err(error) => log::warn!(
            "{}",
            text::get().log_text(text::LogText::GlobalCommandsReadFailed {
                reason: &error.to_string(),
            })
        ),
    }

    Ok(())
}

/// 检查目前全局指令是否已经在 Discord 存在：
/// 标准：名字一一对应，而且最新的一条也满 [`GLOBAL_PROPAGATION_MS`]。
/// 已存在的指令不会被重建，指令 Id 里的创建时间可得出"此指令已经存在多久"。
fn global_commands_settled(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
    now_ms: u64,
) -> bool {
    same_command_names(registered, commands) && propagation_wait_minutes(registered, now_ms) == 0
}

/// 离"可以清理残留指令"还要等多少分钟。
fn minutes_until_cleanup(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
    now_ms: u64,
) -> u64 {
    if same_command_names(registered, commands) {
        propagation_wait_minutes(registered, now_ms)
    } else {
        full_propagation_minutes()
    }
}

/// 距离现有这套指令注册满 1 小时还差多少分钟，不足 1 分钟算 1 分钟，已经满了就是 0。
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

/// 满 1 小时是多少分钟（`GLOBAL_PROPAGATION_MS` 的唯一换算点）。
fn full_propagation_minutes() -> u64 {
    GLOBAL_PROPAGATION_MS.div_ceil(60 * 1000)
}

/// 检查旧全局指令与本次要注册的指令是不是同一套（数量与名字都必须一致）。
fn same_command_names(
    registered: &[serenity::Command],
    commands: &[poise::Command<Data, Error>],
) -> bool {
    let only_slash_commands = registered
        .iter()
        .all(|command| command.kind == serenity::CommandType::ChatInput);
    let registered_names: BTreeSet<&str> = registered
        .iter()
        .map(|command| command.name.as_str())
        .collect();
    let wanted_names: BTreeSet<&str> = commands
        .iter()
        .map(|command| command.name.as_ref())
        .collect();

    only_slash_commands && registered.len() == commands.len() && registered_names == wanted_names
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

/// 清掉各服务器上遗留的 guild 指令。
/// 服务器已经干净就不发写请求。失败记 WARN，下次再试。
async fn clear_guild_scope_leftovers(
    ctx: &serenity::Context,
    guilds: &[serenity::UnavailableGuild],
) {
    for guild in guilds.iter().filter(|guild| !guild.unavailable) {
        let guild_name = guild_name(ctx, guild.id);

        match left_commands(ctx, guild.id).await {
            // 已经干净：不发写请求。
            Ok(0) => {}
            Ok(count) => match guild.id.set_commands(ctx, Vec::new()).await {
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

/// 查看这个服务器上还剩多少条 guild 指令，读取失败时把原因转成文案交给调用方记录日志。
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

/// 清理失败时计入日志 WARN。
fn log_when_clear_failed(guild_name: &str, reason: &str) {
    log::warn!(
        "{}",
        text::get().log_text(text::LogText::GuildCommandsClearFailed { guild_name, reason })
    );
}

/// 获取服务器名，没有就返回服务器 Id。
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
