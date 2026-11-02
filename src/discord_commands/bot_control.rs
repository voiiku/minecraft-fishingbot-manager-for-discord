//! 单机器人控制指令：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`。
//!
//! 六个指令统一进入 [`super::single_bot_control`]；权限由路由层检查。

use super::Error;
use super::{Context, single_bot_control};
use minecraft_fishingbot_manager_for_discord::BotControlAction;
use minecraft_fishingbot_manager_for_discord::text;

#[poise::command(slash_command)]
pub(crate) async fn start(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_start_queued_notice(&format!("/start {name}"));
    single_bot_control(ctx, BotControlAction::Start, name, caller_id, notice).await
}

#[poise::command(slash_command)]
pub(crate) async fn stop(
    ctx: Context<'_>,
    // 排队中的启动请求也可取消。
    #[autocomplete = "super::autocomplete_any_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_started_notice(&format!("/stop {name}"));
    single_bot_control(ctx, BotControlAction::Stop, name, caller_id, notice).await
}

#[poise::command(slash_command)]
pub(crate) async fn restart(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_start_queued_notice(&format!("/restart {name}"));
    single_bot_control(ctx, BotControlAction::Restart, name, caller_id, notice).await
}

#[poise::command(slash_command)]
pub(crate) async fn pause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_started_notice(&format!("/pause {name}"));
    single_bot_control(ctx, BotControlAction::Pause, name, caller_id, notice).await
}

#[poise::command(slash_command)]
pub(crate) async fn unpause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_started_notice(&format!("/unpause {name}"));
    single_bot_control(ctx, BotControlAction::Unpause, name, caller_id, notice).await
}

#[poise::command(slash_command)]
pub(crate) async fn command(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
    command: String,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    let notice = text::get().command_started_notice(&format!("/command {name}"));
    let args = [command.as_str()];
    single_bot_control(
        ctx,
        BotControlAction::Command(&args),
        name,
        caller_id,
        notice,
    )
    .await
}
