//! 只读查询指令：`/show status|botconfig|mybots|allbots|admins`。

use poise::serenity_prelude as serenity;

use super::{Context, Error, read_only_query};
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{ShowAction, handle_fishingbot_config, handle_show};

#[poise::command(
    slash_command,
    subcommands("status", "botconfig", "mybots", "allbots", "admins")
)]
pub(crate) async fn show(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    read_only_query(ctx, move || handle_show(ShowAction::Status, caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn botconfig(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    ctx.defer().await?;

    // 文件读取放进阻塞线程池。
    let config_view =
        tokio::task::spawn_blocking(move || handle_fishingbot_config(caller_id)).await?;

    let config_view = match config_view {
        Ok(config_view) => config_view,
        Err(message) => {
            super::say_and_log(ctx, message).await?;
            return Ok(());
        }
    };

    // 附件回复单独记录。
    let body = text::get().command_botconfig_attachment_body(&config_view.file_name);
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
    let caller_id = ctx.author().id.get();

    read_only_query(ctx, move || handle_show(ShowAction::MyBots, caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn allbots(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    read_only_query(ctx, move || handle_show(ShowAction::AllBots, caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn admins(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    read_only_query(ctx, move || handle_show(ShowAction::Admins, caller_id)).await
}
