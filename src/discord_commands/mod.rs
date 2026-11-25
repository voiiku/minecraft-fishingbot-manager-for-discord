//! Discord 命令层：解析参数、调用业务入口并发送回复；鉴权由 operations 负责。
//! 同步业务操作使用 `spawn_blocking`，指令说明在注册前按配置语言填充。

mod all_bots;
mod anti_afk;
mod autocomplete;
mod bot_control;
mod bot_manage;
mod general;
mod log;
mod registration;
mod show;
mod token;

use std::borrow::Cow;
use std::collections::HashSet;
use std::future::Future;

use crate::config;
use crate::operations::PreparedControl;
use crate::text::report::IntoReplyText;
use crate::text::{CommandText, CommandTextProblem, Text};

pub use registration::try_register_missing_sync_commands;

use autocomplete::{
    autocomplete_any_bot_names, autocomplete_running_bot_names, autocomplete_stopped_bot_names,
};

/// 命令层用到的上下文别名。
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Modal 指令使用的上下文；弹窗必须是交互的第一个响应。
pub(crate) type ApplicationContext<'a> = poise::ApplicationContext<'a, Data, Error>;

/// 命令层没有跨指令共享状态。
pub struct Data;

/// 命令层错误类型；业务错误文案由 `on_error` 记入日志。
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// 返回已按配置语言校验的指令列表。
pub fn all_commands() -> Result<Vec<poise::Command<Data, Error>>, String> {
    let mut commands = declared_commands();
    let i18n_text = crate::config::Config::get().text();
    let constants = &config::Config::get().constants;
    localize_command_texts(
        &mut commands,
        i18n_text.command_texts(),
        crate::bootstrap_error_text(),
        constants.max_command_description_utf16,
    )?;
    localize_log_lines_description(
        &mut commands,
        i18n_text,
        crate::bootstrap_error_text(),
        constants.default_log_lines,
        constants.max_command_description_utf16,
    )?;

    Ok(commands)
}

/// 创建不含说明的指令树。
fn declared_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        general::help(),
        general::whoami(),
        general::ping(),
        general::checkserver(),
        registration::sync_commands(),
        show::show(),
        bot_manage::bot(),
        token::token(),
        log::log(),
        bot_control::start(),
        bot_control::stop(),
        bot_control::restart(),
        bot_control::pause(),
        bot_control::unpause(),
        bot_control::command(),
        all_bots::startall(),
        all_bots::stopall(),
        all_bots::restartall(),
        all_bots::pauseall(),
        all_bots::unpauseall(),
        all_bots::commandall(),
        anti_afk::anti_afk(),
    ]
}

/// 校验指令树与文案表一一对应，再填入说明。
fn localize_command_texts(
    commands: &mut [poise::Command<Data, Error>],
    texts: &[CommandText],
    i18n_text: &dyn Text,
    max_length: usize,
) -> Result<(), String> {
    let mut used = HashSet::new();
    localize_command_tree(commands, texts, i18n_text, "", &mut used, max_length)?;
    for entry in texts {
        if !used.contains(entry.name_path) {
            return Err(
                i18n_text.command_text_invalid(entry.name_path, CommandTextProblem::Unknown)
            );
        }
    }
    Ok(())
}

fn localize_command_tree(
    commands: &mut [poise::Command<Data, Error>],
    texts: &[CommandText],
    i18n_text: &dyn Text,
    parent_path: &str,
    used: &mut HashSet<String>,
    max_length: usize,
) -> Result<(), String> {
    for command in commands {
        let name_path = if parent_path.is_empty() {
            command.name.to_string()
        } else {
            format!("{parent_path} {}", command.name)
        };

        let mut matches = texts.iter().filter(|entry| entry.name_path == name_path);
        let command_text = matches.next().ok_or_else(|| {
            i18n_text.command_text_invalid(&name_path, CommandTextProblem::Missing)
        })?;
        if matches.next().is_some() || !used.insert(name_path.clone()) {
            return Err(i18n_text.command_text_invalid(&name_path, CommandTextProblem::Duplicate));
        }
        validate_description(&name_path, command_text.description, i18n_text, max_length)?;
        command.description = Some(Cow::Borrowed(command_text.description));

        let mut parameter_names = HashSet::new();
        for parameter in &command.parameters {
            if !parameter_names.insert(parameter.name.as_ref()) {
                return Err(i18n_text.command_text_invalid(
                    &format!("{name_path} {}", parameter.name),
                    CommandTextProblem::Duplicate,
                ));
            }
        }
        let mut option_names = HashSet::new();
        for (option_name, description) in command_text.options {
            let path = format!("{name_path} {option_name}");
            if !option_names.insert(*option_name) {
                return Err(i18n_text.command_text_invalid(&path, CommandTextProblem::Duplicate));
            }
            if !parameter_names.contains(option_name) {
                return Err(i18n_text.command_text_invalid(&path, CommandTextProblem::Unknown));
            }
            validate_description(&path, description, i18n_text, max_length)?;
        }
        drop(parameter_names);
        for parameter in &mut command.parameters {
            let (_, description) = command_text
                .options
                .iter()
                .find(|(name, _)| *name == parameter.name)
                .ok_or_else(|| {
                    i18n_text.command_text_invalid(
                        &format!("{name_path} {}", parameter.name),
                        CommandTextProblem::Missing,
                    )
                })?;
            parameter.description = Some(Cow::Borrowed(description));
        }
        localize_command_tree(
            &mut command.subcommands,
            texts,
            i18n_text,
            &name_path,
            used,
            max_length,
        )?;
    }
    Ok(())
}

fn validate_description(
    path: &str,
    description: &str,
    i18n_text: &dyn Text,
    max_length: usize,
) -> Result<(), String> {
    let length = description.encode_utf16().count();
    if description.trim().is_empty() || !(1..=max_length).contains(&length) {
        return Err(i18n_text.command_text_invalid(
            path,
            CommandTextProblem::InvalidLength {
                length,
                max: max_length,
            },
        ));
    }
    Ok(())
}

fn localize_log_lines_description(
    commands: &mut [poise::Command<Data, Error>],
    language: &dyn Text,
    errors: &dyn Text,
    default_lines: usize,
    max_length: usize,
) -> Result<(), String> {
    let parameter = commands
        .iter_mut()
        .find(|command| command.name == "log")
        .and_then(|command| {
            command
                .parameters
                .iter_mut()
                .find(|parameter| parameter.name == "lines")
        })
        .ok_or_else(|| errors.command_text_invalid("log lines", CommandTextProblem::Missing))?;
    let description = language.command_log_lines_description(default_lines);
    validate_description("log lines", &description, errors, max_length)?;
    parameter.description = Some(Cow::Owned(description));
    Ok(())
}

// ======================== 指令回复入口 ========================

/// 先确认交互；只有业务准备成功才发送进度消息，并在消息发送成功后提交执行。
async fn run_control_with_notice(
    ctx: Context<'_>,
    prepare: impl FnOnce(u64) -> Result<PreparedControl, String> + Send + 'static,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    ctx.defer().await?;
    let control = match tokio::task::spawn_blocking(move || prepare(caller_id)).await? {
        Ok(control) => control,
        Err(message) => return say_and_log(ctx, message).await,
    };
    say_and_log(ctx, &control.notice).await?;
    let reply = tokio::task::spawn_blocking(move || control.execute())
        .await?
        .to_reply_text();
    say_final_report(ctx, &reply).await
}

/// 确认交互后，在阻塞线程池执行操作并发送一条普通回复。
async fn run_blocking_and_reply(
    ctx: Context<'_>,
    operation: impl FnOnce(u64) -> Result<String, String> + Send + 'static,
) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    ctx.defer().await?;
    let reply = tokio::task::spawn_blocking(move || operation(caller_id))
        .await?
        .to_reply_text();
    say_and_log(ctx, reply).await
}

/// 异步执行业务操作并发送结果。
pub(crate) async fn run_async_and_reply(
    ctx: Context<'_>,
    run: impl Future<Output = Result<String, String>>,
) -> Result<(), Error> {
    ctx.defer().await?;
    say_and_log(ctx, run.await.to_reply_text()).await?;

    Ok(())
}

/// 发送回复并记录调用者、指令名与回复内容。
pub(crate) async fn say_and_log(ctx: Context<'_>, reply: impl Into<String>) -> Result<(), Error> {
    let reply = reply.into();
    log_reply(ctx.author().id.get(), &ctx.command().qualified_name, &reply);

    ctx.say(reply).await?;

    Ok(())
}

/// 只记录回复，不发送；供 Modal 等不能使用 `ctx.say` 的指令调用。
pub(crate) fn log_reply(caller_id: u64, command_path: &str, reply: &str) {
    ::log::info!(
        "{}",
        crate::config::Config::get()
            .text()
            .command_replied(caller_id, command_path, reply)
    );
}

/// defer 后命令内部失败时，发送错误提示。
///
/// Poise 会根据上下文中记录的初始响应状态，将消息作为首次回复或 follow-up 发送。
pub async fn reply_command_failure(ctx: Context<'_>) {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();
    let reply = crate::config::Config::get()
        .text()
        .command_unexpected_failure();

    match ctx
        .send(poise::CreateReply::default().content(reply).ephemeral(true))
        .await
    {
        Ok(_) => log_reply(caller_id, &command_path, reply),
        Err(error) => ::log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .command_fallback_reply_failed(caller_id, &command_path, &error.to_string())
        ),
    }
}

/// 发送最终报告；交互回复失败时改发频道消息，以应对 15 分钟的令牌有效期。
async fn say_final_report(ctx: Context<'_>, report: &str) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();

    if let Err(error) = say_and_log(ctx, report).await {
        // 使用绝对路径区分 `log` crate 与同名指令模块。
        ::log::warn!(
            "{}",
            crate::config::Config::get()
                .text()
                .report_posted_to_channel(caller_id, &command_path, &error.to_string())
        );

        ctx.channel_id().say(ctx.http(), report).await?;
        log_reply(caller_id, &command_path, report);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
