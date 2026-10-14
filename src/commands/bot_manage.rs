//! 机器人目录管理指令。权限由路由层检查。

use super::Error;
use super::{Context, single_bot_manage};
use minecraft_fishingbot_manager_for_discord::{
    BotManageAction, handle_bot_login, handle_bot_login_await, handle_bot_manage,
};

/// Manage bot accounts
#[poise::command(slash_command, subcommands("add", "del", "rename", "login"))]
pub(crate) async fn bot(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add a bot (its token is added afterwards)
#[poise::command(slash_command)]
pub(crate) async fn add(
    ctx: Context<'_>,
    #[description = "bot name"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_manage(ctx, move || {
        handle_bot_manage(BotManageAction::Add, &name, caller_id)
    })
    .await
}

/// Delete a bot and all of its data (stop it first)
#[poise::command(slash_command)]
pub(crate) async fn del(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"]
    #[description = "bot name"]
    name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_manage(ctx, move || {
        handle_bot_manage(BotManageAction::Delete, &name, caller_id)
    })
    .await
}

/// Rename a bot (stop it first)
#[poise::command(slash_command)]
pub(crate) async fn rename(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"]
    #[description = "current bot name"]
    old_name: String,
    #[description = "new bot name"] new_name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_manage(ctx, move || {
        handle_bot_manage(BotManageAction::Rename(new_name), &old_name, caller_id)
    })
    .await
}

/// Log a bot in with Microsoft (returns a link and a code)
#[poise::command(slash_command)]
pub(crate) async fn login(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"]
    #[description = "bot name"]
    name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();

    ctx.defer_ephemeral().await?;

    // 启动会话并等待设备码。
    let prompt =
        match tokio::task::spawn_blocking(move || handle_bot_login(&name, caller_id)).await? {
            Ok(prompt) => prompt,
            Err(message) => {
                super::log_reply(caller_id, &command_path, &message);
                ctx.send(
                    poise::CreateReply::default()
                        .content(message)
                        .ephemeral(true),
                )
                .await?;
                return Ok(());
            }
        };

    // 发送登录网址与设备码。
    super::log_reply(caller_id, &command_path, &prompt.message);
    ctx.send(
        poise::CreateReply::default()
            .content(prompt.message.clone())
            .ephemeral(true),
    )
    .await?;

    // 等待授权结果并发送第二条回复。
    let reply = tokio::task::spawn_blocking(move || handle_bot_login_await(prompt)).await?;
    super::log_reply(caller_id, &command_path, &reply);
    ctx.send(poise::CreateReply::default().content(reply).ephemeral(true))
        .await?;

    Ok(())
}
