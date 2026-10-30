//! 全服批量指令；权限由路由层检查。

use super::Error;
use super::{Context, all_bots_manage, run_async_and_reply};
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, AntiAfkAction, handle_all_bots_control, handle_anti_afk,
    handle_startall_notice,
};

#[poise::command(slash_command)]
pub(crate) async fn startall(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    all_bots_manage(ctx, handle_startall_notice(), move || {
        handle_all_bots_control(AllBotsControlAction::StartAll, caller_id)
    })
    .await
}

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

#[poise::command(slash_command)]
pub(crate) async fn commandall(ctx: Context<'_>, command: String) -> Result<(), Error> {
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
