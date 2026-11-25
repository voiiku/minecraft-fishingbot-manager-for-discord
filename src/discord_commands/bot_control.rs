//! 单机器人控制指令：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`。
//!
//! 同步操作使用阻塞线程池；业务入口鉴权成功后才发送进度回执。

use super::{Context, Error, run_control_with_notice};
use crate::operations::single_bot_control;

#[poise::command(slash_command)]
pub(crate) async fn start(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_stopped_bot_names"] name: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_start_single_bot(&name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn stop(
    ctx: Context<'_>,
    // 排队中的启动请求也可取消。
    #[autocomplete = "super::autocomplete_any_bot_names"] name: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_stop_single_bot(&name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn restart(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_restart_single_bot(&name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn pause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_pause_single_bot(&name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn unpause(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_unpause_single_bot(&name, caller_id)
    })
    .await
}

#[poise::command(slash_command)]
pub(crate) async fn command(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_running_bot_names"] name: String,
    command: String,
) -> Result<(), Error> {
    run_control_with_notice(ctx, move |caller_id| {
        single_bot_control::prepare_send_single_bot_command(&name, &[command.as_str()], caller_id)
    })
    .await
}
