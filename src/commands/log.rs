//! `/log`：读取最新日志并以仅调用者可见的附件发送。

use poise::serenity_prelude as serenity;

use super::Context;
use super::Error;
use minecraft_fishingbot_manager_for_discord::handle_bot_log;
use minecraft_fishingbot_manager_for_discord::text;

/// `/log` 默认读取的行数；业务层还会按配置限制上限。
const DEFAULT_LOG_LINES: usize = 50;

/// Read a bot's latest log
#[poise::command(slash_command)]
pub(crate) async fn log(
    ctx: Context<'_>,
    #[autocomplete = "super::autocomplete_any_bot_names"]
    #[description = "bot name"]
    name: String,
    #[description = "how many trailing lines to read (default 50, cap in runtime.max_log_lines)"]
    lines: Option<i64>,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();

    // 行数无效时返回用法提示。
    let line_count = match lines {
        Some(lines) => match usize::try_from(lines) {
            Ok(lines) if lines > 0 => lines,
            _ => {
                let usage = text::get().command_log_lines_usage();
                super::log_reply(caller_id, &ctx.command().qualified_name, usage);

                ctx.send(poise::CreateReply::default().content(usage).ephemeral(true))
                    .await?;

                return Ok(());
            }
        },
        None => DEFAULT_LOG_LINES,
    };

    ctx.defer_ephemeral().await?;

    // 文件读取放进阻塞线程池；保留名字用于回复正文。
    let query_bot_name = name.clone();
    let log_view =
        tokio::task::spawn_blocking(move || handle_bot_log(caller_id, &query_bot_name, line_count))
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

    let mut body =
        text::get().command_log_attachment_body(&name, &log_view.file_name, log_view.lines.len());
    if log_view.dropped_line_count > 0 {
        body.push_str(&text::get().command_log_dropped_note(log_view.dropped_line_count));
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
