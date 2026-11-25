//! `/log`：读取最新日志并以仅调用者可见的附件发送。

use crate::operations::queries;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Error;

#[poise::command(slash_command)]
pub(crate) async fn log(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_any_bot_names"] name: String,
    lines: Option<i64>,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    ctx.defer_ephemeral().await?;

    // 文件读取放进阻塞线程池；保留名字用于回复正文。
    let query_bot_name = name.clone();
    let log_view = tokio::task::spawn_blocking(move || {
        queries::read_bot_log(&query_bot_name, lines, caller_id)
    })
    .await?;

    let log_view = match log_view {
        Ok(log_view) => log_view,
        Err(message) => {
            super::log_reply(caller_id, &ctx.command().qualified_name, &message);
            ctx.send(
                poise::CreateReply::default()
                    .content(message)
                    .ephemeral(true),
            )
            .await?;

            return Ok(());
        }
    };

    let mut body = crate::config::Config::get()
        .text()
        .command_log_attachment_body(&name, &log_view.file_name, log_view.lines.len());
    if log_view.dropped_line_count > 0 {
        body.push_str(
            &crate::config::Config::get()
                .text()
                .command_log_dropped_note(log_view.dropped_line_count),
        );
    }

    let attachment = serenity::CreateAttachment::bytes(
        log_view.lines.join("\n").into_bytes(),
        log_view.file_name.clone(),
    );

    // 附件回复单独记录。
    super::log_reply(caller_id, &ctx.command().qualified_name, &body);

    ctx.send(
        poise::CreateReply::default()
            .content(body)
            .attachment(attachment)
            // followup 不继承 defer 的 ephemeral 设置。
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
