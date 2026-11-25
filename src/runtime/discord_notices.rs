//! 将运行时通知发送到各服务器：优先系统频道，否则使用 ID 最小的文字频道。
use poise::serenity_prelude as serenity;

pub(super) async fn notify_guilds(
    ctx: &serenity::Context,
    message: &str,
    on_failure: impl Fn(&str, &str),
) -> (usize, usize) {
    let mut notified = 0;
    let mut failed = 0;
    for id in ctx.cache.guilds() {
        let (guild_name, channel) = ctx.cache.guild(id).map_or_else(
            || (id.to_string(), None),
            |guild| {
                let channel = guild.system_channel_id.or_else(|| {
                    guild
                        .channels
                        .values()
                        .filter(|channel| channel.kind == serenity::ChannelType::Text)
                        .map(|channel| channel.id)
                        .min()
                });
                (guild.name.clone(), channel)
            },
        );
        let Some(channel) = channel else {
            failed += 1;
            on_failure(
                &guild_name,
                &crate::bootstrap_error_text().reconnect_notice_no_channel(),
            );
            continue;
        };
        match channel.say(&ctx.http, message).await {
            Ok(_) => notified += 1,
            Err(error) => {
                failed += 1;
                on_failure(&guild_name, &error.to_string());
            }
        }
    }
    (notified, failed)
}
