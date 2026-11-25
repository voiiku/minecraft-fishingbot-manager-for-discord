//! `/token`：通过私密弹窗更新 refreshToken。

use crate::operations::accounts;
use crate::text::report::IntoReplyText;
use poise::Modal;
use poise::serenity_prelude as serenity;

use super::{ApplicationContext, Error};
use crate::config;

/// Token 弹窗表单。
#[derive(Debug)]
struct TokenModal {
    /// 机器人名。
    bot_name: String,
    /// refreshToken 文件的全部内容（多行文本框）。
    token: String,
}

/// 输入框 ID，供 `create` 与 `parse` 共用。
const BOT_NAME_INPUT_ID: &str = "bot_name";
const TOKEN_INPUT_ID: &str = "token";

impl poise::Modal for TokenModal {
    fn create(_defaults: Option<Self>, custom_id: String) -> serenity::CreateInteractionResponse {
        let i18n_text = crate::config::Config::get().text();

        let bot_name_input = serenity::CreateInputText::new(
            serenity::InputTextStyle::Short,
            i18n_text.token_modal_bot_name_label(),
            BOT_NAME_INPUT_ID,
        )
        .placeholder(i18n_text.token_modal_bot_name_placeholder())
        .required(true)
        .max_length(config::Config::get().constants.bot_name_modal_max_length);

        let token_input = serenity::CreateInputText::new(
            serenity::InputTextStyle::Paragraph,
            i18n_text.token_modal_token_label(),
            TOKEN_INPUT_ID,
        )
        .placeholder(i18n_text.token_modal_token_placeholder())
        .required(true)
        .max_length(config::Config::get().constants.token_modal_max_length);

        serenity::CreateInteractionResponse::Modal(
            serenity::CreateModal::new(custom_id, i18n_text.token_modal_title()).components(vec![
                serenity::CreateActionRow::InputText(bot_name_input),
                serenity::CreateActionRow::InputText(token_input),
            ]),
        )
    }

    /// 解析弹窗字段。
    fn parse(mut data: serenity::ModalInteractionData) -> Result<Self, &'static str> {
        Ok(Self {
            bot_name: poise::find_modal_text(&mut data, BOT_NAME_INPUT_ID)
                .ok_or("missing bot_name")?,
            token: poise::find_modal_text(&mut data, TOKEN_INPUT_ID).ok_or("missing token")?,
        })
    }
}

#[poise::command(slash_command)]
pub(crate) async fn token(ctx: ApplicationContext<'_>) -> Result<(), Error> {
    // Modal 必须是首次响应，用户关闭或未提交时直接结束。
    let Some(form) = TokenModal::execute(ctx).await? else {
        return Ok(());
    };

    let caller_id = ctx.author().id.get();

    let reply = tokio::task::spawn_blocking(move || {
        accounts::update_bot_token(&form.bot_name, form.token, caller_id)
    })
    .await?
    .to_reply_text();

    // Modal 提交后以私密 followup 回复并记录内容。
    super::log_reply(caller_id, &ctx.command().qualified_name, &reply);
    ctx.send(poise::CreateReply::default().content(reply).ephemeral(true))
        .await?;

    Ok(())
}
