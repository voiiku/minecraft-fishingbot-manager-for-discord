//! 只读查询指令：`/show status|botconfig|mybots|allbots|admins`。

use crate::operations::queries;
use poise::serenity_prelude as serenity;

use super::{Context, Error, run_blocking_and_reply};

#[poise::command(
    slash_command,
    subcommands("status", "botconfig", "mybots", "allbots", "admins")
)]
pub(crate) async fn show(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn status(ctx: Context<'_>) -> Result<(), Error> {
    run_blocking_and_reply(ctx, |_| queries::show_status()).await
}

#[poise::command(slash_command)]
pub(crate) async fn botconfig(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    ctx.defer().await?;

    // 文件读取放进阻塞线程池。
    let config_view = tokio::task::spawn_blocking(queries::show_bot_config).await?;

    let config_view = match config_view {
        Ok(config_view) => config_view,
        Err(message) => {
            super::say_and_log(ctx, message).await?;
            return Ok(());
        }
    };

    // 附件回复单独记录。
    let body = crate::config::Config::get()
        .text()
        .command_botconfig_attachment_body(&config_view.file_name);
    super::log_reply(caller_id, &ctx.command().qualified_name, &body);

    let attachment = serenity::CreateAttachment::bytes(
        config_view.content.into_bytes(),
        config_view.file_name.clone(),
    );

    ctx.send(
        poise::CreateReply::default()
            .content(body)
            .attachment(attachment),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn mybots(ctx: Context<'_>) -> Result<(), Error> {
    run_blocking_and_reply(ctx, queries::show_my_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn allbots(ctx: Context<'_>) -> Result<(), Error> {
    run_blocking_and_reply(ctx, |_| queries::show_all_bots()).await
}

#[poise::command(slash_command)]
pub(crate) async fn admins(ctx: Context<'_>) -> Result<(), Error> {
    run_blocking_and_reply(ctx, |_| Ok(queries::show_admins())).await
}
