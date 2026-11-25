//! 机器人目录管理指令。

use super::{Context, Error, run_blocking_and_reply};
use crate::operations::accounts;

#[poise::command(slash_command, subcommands("add", "del", "rename", "login"))]
pub(crate) async fn bot(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn add(ctx: Context<'_>, name: String) -> Result<(), Error> {
    run_blocking_and_reply(ctx, move |caller_id| accounts::add_bot(&name, caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn del(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] name: String,
) -> Result<(), Error> {
    run_blocking_and_reply(ctx, move |caller_id| accounts::delete_bot(&name, caller_id)).await
}

#[poise::command(slash_command)]
pub(crate) async fn rename(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] old_name: String,
    new_name: String,
) -> Result<(), Error> {
    run_blocking_and_reply(ctx, move |caller_id| {
        accounts::rename_bot(&old_name, &new_name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn login(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();
    let log_bot_name = name.clone();

    ctx.defer_ephemeral().await?;

    // 启动会话并等待设备码。
    let prompt =
        match tokio::task::spawn_blocking(move || accounts::begin_bot_login(&name, caller_id))
            .await?
        {
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

    // 回复包含登录网址与设备码；日志只记录脱敏摘要。
    super::log_reply(
        caller_id,
        &command_path,
        &crate::config::Config::get()
            .text()
            .bot_login_prompt_logged(&log_bot_name),
    );
    ctx.send(
        poise::CreateReply::default()
            .content(prompt.message.clone())
            .ephemeral(true),
    )
    .await?;

    // 等待授权结果并发送第二条回复。
    let reply = tokio::task::spawn_blocking(move || accounts::finish_bot_login(prompt)).await?;
    super::log_reply(caller_id, &command_path, &reply);
    ctx.send(poise::CreateReply::default().content(reply).ephemeral(true))
        .await?;

    Ok(())
}
