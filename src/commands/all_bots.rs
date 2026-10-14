//! 全服批量指令；权限由路由层检查。

use super::Error;
use super::{Context, all_bots_manage};
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{AllBotsControlAction, handle_all_bots_control};

/// Start every bot that has a token (admins only)
#[poise::command(slash_command)]
pub(crate) async fn startall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        text::get().command_start_queued_notice("/startall"),
        move || handle_all_bots_control(AllBotsControlAction::StartAll, caller_id),
    )
    .await
}

/// Stop every bot (admins only)
#[poise::command(slash_command)]
pub(crate) async fn stopall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        text::get().command_started_notice("/stopall"),
        move || handle_all_bots_control(AllBotsControlAction::StopAll, caller_id),
    )
    .await
}

/// Restart every running bot (admins only)
#[poise::command(slash_command)]
pub(crate) async fn restartall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        text::get().command_start_queued_notice("/restartall"),
        move || handle_all_bots_control(AllBotsControlAction::RestartAll, caller_id),
    )
    .await
}

/// Send the same in-game command to every running bot (admins only)
#[poise::command(slash_command)]
pub(crate) async fn commandall(
    ctx: Context<'_>,
    #[description = "in-game command (leading slash = command, otherwise chat; write it as is, no quotes)"]
    command: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        text::get().command_started_notice(&format!("/commandall {command}")),
        move || {
            let raw_bot_command_args = [command.as_str()];
            handle_all_bots_control(
                AllBotsControlAction::CommandAll(&raw_bot_command_args),
                caller_id,
            )
        },
    )
    .await
}
