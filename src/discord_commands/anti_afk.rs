use super::{Context, Error, run_async_and_reply};
use crate::operations::set_anti_afk;

#[poise::command(slash_command, rename = "anti-afk")]
pub(crate) async fn anti_afk(
    ctx: Context<'_>,
    #[choices("on", "off")] command: &'static str,
) -> Result<(), Error> {
    run_async_and_reply(ctx, set_anti_afk(command == "on", ctx.author().id.get())).await
}
