//! 通用指令：`/help`、`/whoami`、`/ping`、`/checkserver`。

use super::Context;
use super::Error;
use crate::operations::queries;

#[poise::command(slash_command)]
pub(crate) async fn help(ctx: Context<'_>) -> Result<(), Error> {
    super::say_and_log(ctx, queries::help()).await
}

#[poise::command(slash_command)]
pub(crate) async fn whoami(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    super::say_and_log(ctx, queries::whoami(caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn checkserver(ctx: Context<'_>) -> Result<(), Error> {
    super::run_async_and_reply(ctx, queries::check_server()).await
}

#[poise::command(slash_command)]
pub(crate) async fn ping(
    ctx: Context<'_>,
    host: Option<String>,
    port: Option<i64>,
) -> Result<(), Error> {
    let port = match port {
        Some(port) => match u16::try_from(port) {
            Ok(port) if port > 0 => Some(port),
            _ => {
                super::say_and_log(
                    ctx,
                    crate::config::Config::get()
                        .text()
                        .command_ping_port_usage(),
                )
                .await?;
                return Ok(());
            }
        },
        None => None,
    };

    super::run_async_and_reply(ctx, queries::ping_server(host.as_deref(), port)).await
}
