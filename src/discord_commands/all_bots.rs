//! 全服批量指令；业务入口鉴权成功后才发送进度回执。

use super::{Context, Error, run_control_with_notice};
use crate::operations::all_bots_control;

#[poise::command(slash_command)]
pub(crate) async fn startall(ctx: Context<'_>) -> Result<(), Error> {
    run_control_with_notice(ctx, all_bots_control::prepare_start_all_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn stopall(ctx: Context<'_>) -> Result<(), Error> {
    run_control_with_notice(ctx, all_bots_control::prepare_stop_all_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn restartall(ctx: Context<'_>) -> Result<(), Error> {
    run_control_with_notice(ctx, all_bots_control::prepare_restart_all_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn pauseall(ctx: Context<'_>) -> Result<(), Error> {
    run_control_with_notice(ctx, all_bots_control::prepare_pause_all_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn unpauseall(ctx: Context<'_>) -> Result<(), Error> {
    run_control_with_notice(ctx, all_bots_control::prepare_unpause_all_bots).await
}

#[poise::command(slash_command)]
pub(crate) async fn commandall(ctx: Context<'_>, command: String) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        all_bots_control::prepare_send_all_bots_command(&[command.as_str()], caller_id)
    })
    .await
}
