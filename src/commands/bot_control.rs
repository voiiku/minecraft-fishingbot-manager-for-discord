//! 单机器人控制指令：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`。
//!
//! 六个指令统一进入 [`super::single_bot_control`]；权限由路由层检查。

use super::Error;
use super::{Context, single_bot_control};
use minecraft_fishingbot_manager_for_discord::text;
use minecraft_fishingbot_manager_for_discord::{BotControlAction, handle_bot_control};

#[poise::command(slash_command)]
pub(crate) async fn start(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_start_queued_notice(&format!("/start {name}")),
        move || handle_bot_control(BotControlAction::Start, &name, caller_id),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn stop(
    ctx: Context<'_>,
    // 排队等待启动的也能被 `/stop` 取消，所以候选给全部状态（标注里能看出它是在跑还是排队）。
    #[autocomplete = "super::autocomplete_any_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_started_notice(&format!("/stop {name}")),
        move || handle_bot_control(BotControlAction::Stop, &name, caller_id),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn restart(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_start_queued_notice(&format!("/restart {name}")),
        move || handle_bot_control(BotControlAction::Restart, &name, caller_id),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn pause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_started_notice(&format!("/pause {name}")),
        move || handle_bot_control(BotControlAction::Pause, &name, caller_id),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn unpause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_started_notice(&format!("/unpause {name}")),
        move || handle_bot_control(BotControlAction::Unpause, &name, caller_id),
    )
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn command(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
    command: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    single_bot_control(
        ctx,
        text::get().command_started_notice(&format!("/command {name}")),
        move || {
            let raw_bot_command_args = [command.as_str()];
            handle_bot_control(
                BotControlAction::Command(&raw_bot_command_args),
                &name,
                caller_id,
            )
        },
    )
    .await
}
