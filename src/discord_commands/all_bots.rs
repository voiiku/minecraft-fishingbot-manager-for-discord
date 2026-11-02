//! 全服批量指令；权限由路由层检查。

use super::Error;
use super::{Context, all_bots_manage, run_async_and_reply};
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, AntiAfkAction, handle_anti_afk, handle_startall_notice,
};

#[poise::command(slash_command)]
pub(crate) async fn startall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        AllBotsControlAction::StartAll,
        caller_id,
        handle_startall_notice(),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn stopall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        AllBotsControlAction::StopAll,
        caller_id,
        text::get().command_started_notice("/stopall"),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn restartall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        AllBotsControlAction::RestartAll,
        caller_id,
        text::get().command_start_queued_notice("/restartall"),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn pauseall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        AllBotsControlAction::PauseAll,
        caller_id,
        text::get().command_started_notice("/pauseall"),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn unpauseall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(
        ctx,
        AllBotsControlAction::UnpauseAll,
        caller_id,
        text::get().command_started_notice("/unpauseall"),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn commandall(ctx: Context<'_>, command: String) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_started_notice(&format!("/commandall {command}"));
    let args = [command.as_str()];
    all_bots_manage(
        ctx,
        AllBotsControlAction::CommandAll(&args),
        caller_id,
        notice,
    )
    .await
}

#[derive(Debug, poise::ChoiceParameter)]
pub(crate) enum AntiAfkOption {
    #[name = "on"]
    On,
    #[name = "off"]
    Off,
}

#[poise::command(slash_command, rename = "anti-afk")]
pub(crate) async fn anti_afk(ctx: Context<'_>, command: AntiAfkOption) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let action = match command {
        AntiAfkOption::On => AntiAfkAction::On,
        AntiAfkOption::Off => AntiAfkAction::Off,
    };
    run_async_and_reply(ctx, handle_anti_afk(action, caller_id)).await
}
