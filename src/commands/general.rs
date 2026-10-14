//! 通用指令：`/help`、`/whoami`、`/ping`、`/checkserver`。

use super::Context;
use super::Error;
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{
    handle_checkserver, handle_help, handle_ping, handle_whoami,
};

/// Show the help menu
#[poise::command(slash_command)]
pub(crate) async fn help(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    super::say_and_log(ctx, handle_help(caller_id)).await?;
    Ok(())
}

/// Show the caller's Discord user ID
#[poise::command(slash_command)]
pub(crate) async fn whoami(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    super::say_and_log(ctx, handle_whoami(caller_id)).await?;

    Ok(())
}

/// Show this fishing host's performance
#[poise::command(slash_command)]
pub(crate) async fn checkserver(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    super::server_probe(ctx, handle_checkserver(caller_id)).await
}

/// Check the latency from this fishing host to the game server, plus the player count
#[poise::command(slash_command)]
pub(crate) async fn ping(
    ctx: Context<'_>,
    #[description = "server address (domain or IP), empty uses the default target"] host: Option<
        String,
    >,
    #[description = "server port 1-65535, empty uses the default port"] port: Option<i64>,
) -> Result<(), Error> {
    let port = match port {
        Some(port) => match u16::try_from(port) {
            Ok(port) if port > 0 => Some(port),
            _ => {
                super::say_and_log(ctx, text::get().command_ping_port_usage()).await?;
                return Ok(());
            }
        },
        None => None,
    };

    let caller_id = ctx.author().id.get();

    super::server_probe(ctx, handle_ping(caller_id, host.as_deref(), port)).await
}
