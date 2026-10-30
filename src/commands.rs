//! 命令层：定义斜杠指令，取参数、校验数字、defer，并把结果交给 `lib::handle_*`。
//! 权限与业务逻辑在下层；同步业务调用统一放进 `spawn_blocking`。
//!
//! 命令和选项说明统一来自文案表，注册前按配置语言校验并填充。
//! 每条指令都必须经过 `lib::handle_*`，不能在本层跳过权限检查或自行拼业务回复。

mod all_bots;
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

use minecraft_fishingbot_manager_for_discord::text::{self, CommandText, CommandTextProblem, Text};

pub(crate) use registration::register_commands;

pub(super) use autocomplete::{
    autocomplete_any_bot_names, autocomplete_running_bot_names, autocomplete_stopped_bot_names,
};

/// 命令层用到的上下文别名。
pub(crate) type Context<'a> = poise::Context<'a, Data, Error>;

/// Modal 指令使用的上下文；弹窗必须是交互的第一个响应。
pub(crate) type ApplicationContext<'a> = poise::ApplicationContext<'a, Data, Error>;

/// 命令层没有跨指令共享状态。
pub(crate) struct Data;

/// 命令层错误类型；业务错误文案由 `on_error` 记入日志。
pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;

/// 返回已按配置语言校验的指令列表。
pub(crate) fn all_commands() -> Result<Vec<poise::Command<Data, Error>>, String> {
    let mut commands = declared_commands();
    let i18n_text = text::get();
    localize_command_texts(&mut commands, i18n_text.command_texts(), i18n_text)?;

    Ok(commands)
}

/// 创建不含说明的指令树。
fn declared_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        general::help(),
        general::whoami(),
        general::ping(),
        general::checkserver(),
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
        all_bots::commandall(),
        all_bots::anti_afk(),
    ]
}

/// 校验指令树与文案表一一对应，再填入说明。
fn localize_command_texts(
    commands: &mut [poise::Command<Data, Error>],
    texts: &[CommandText],
    i18n_text: &dyn Text,
) -> Result<(), String> {
    let mut used = HashSet::new();
    localize_command_tree(commands, texts, i18n_text, "", &mut used)?;
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
        validate_description(&name_path, command_text.description, i18n_text)?;
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
            validate_description(&path, description, i18n_text)?;
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
        localize_command_tree(&mut command.subcommands, texts, i18n_text, &name_path, used)?;
    }
    Ok(())
}

fn validate_description(path: &str, description: &str, i18n_text: &dyn Text) -> Result<(), String> {
    let length = description.encode_utf16().count();
    if description.trim().is_empty() || !(1..=100).contains(&length) {
        return Err(
            i18n_text.command_text_invalid(path, CommandTextProblem::InvalidLength { length })
        );
    }
    Ok(())
}

// ======================== 指令回复入口 ========================

/// 单机器人控制：先发送进度回执，再发送操作结果。
pub(crate) async fn single_bot_control<F>(
    ctx: Context<'_>,
    started_notice: String,
    run: F,
) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    run_and_reply_with_progress(ctx, started_notice, run).await
}

/// 单机器人账号管理，完成后发送操作结果。`/token` 单独处理 Modal。
pub(crate) async fn single_bot_manage<F>(ctx: Context<'_>, run: F) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    run_and_reply(ctx, run).await
}

/// 全服批量管理：先发送进度回执，再发送操作结果。
pub(crate) async fn all_bots_manage<F>(
    ctx: Context<'_>,
    started_notice: String,
    run: F,
) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    run_and_reply_with_progress(ctx, started_notice, run).await
}

/// 只读查询：在线程池执行后发送结果。
pub(crate) async fn read_only_query<F>(ctx: Context<'_>, run: F) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    run_and_reply(ctx, run).await
}

/// 异步执行业务操作并发送结果。
pub(crate) async fn run_async_and_reply(
    ctx: Context<'_>,
    run: impl Future<Output = String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    say_and_log(ctx, run.await).await?;

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
        text::get().log_text(text::LogText::CommandReplied {
            caller_id,
            command_path,
            reply,
        })
    );
}

/// defer 后命令内部失败时，发送错误提示。
///
/// Poise 会根据上下文中记录的初始响应状态，将消息作为首次回复或 follow-up 发送。
pub(crate) async fn reply_command_failure(ctx: Context<'_>) {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();
    let reply = text::get().command_unexpected_failure();

    match ctx
        .send(poise::CreateReply::default().content(reply).ephemeral(true))
        .await
    {
        Ok(_) => log_reply(caller_id, &command_path, reply),
        Err(error) => ::log::warn!(
            "{}",
            text::get().log_text(text::LogText::CommandFallbackReplyFailed {
                caller_id,
                command_path: &command_path,
                reason: &error.to_string(),
            })
        ),
    }
}

/// defer 后在线程池执行阻塞业务，再发送结果。
/// `run` 必须是 `Send + 'static`，不能借用 `ctx`。
async fn run_and_reply<F>(ctx: Context<'_>, run: F) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    ctx.defer().await?;

    let reply = tokio::task::spawn_blocking(run).await?;
    say_and_log(ctx, reply).await?;

    Ok(())
}

/// 长任务先发送进度回执，再在线程池执行并发送结果。
async fn run_and_reply_with_progress<F>(
    ctx: Context<'_>,
    started_notice: String,
    run: F,
) -> Result<(), Error>
where
    F: FnOnce() -> String + Send + 'static,
{
    ctx.defer().await?;
    say_and_log(ctx, started_notice).await?;

    let reply = tokio::task::spawn_blocking(run).await?;
    say_final_report(ctx, &reply).await?;

    Ok(())
}

/// 发送长任务的最终报告；当交互回复发不出去时，改发普通频道消息。
/// 交互令牌只有 15 分钟，批量操作需要的时间比它久时一定失败，用户收不到回复。
/// 这条兜底把报告发进当前频道，并留一条 `ReportPostedToChannel` 日志
/// 说明为什么改成了频道消息。发送失败的原因写进日志。
async fn say_final_report(ctx: Context<'_>, report: &str) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();

    if let Err(error) = say_and_log(ctx, report).await {
        // `log` 是本文件的子模块（`/log` 指令），所以这里要写全路径指向 `log` crate。
        ::log::warn!(
            "{}",
            text::get().log_text(text::LogText::ReportPostedToChannel {
                caller_id,
                command_path: &command_path,
                reason: &error.to_string(),
            })
        );

        ctx.channel_id().say(ctx.http(), report).await?;
        log_reply(caller_id, &command_path, report);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试进程的语言实现是进程级单例；需要文案的测试先安装英语。
    fn install_english_text() {
        minecraft_fishingbot_manager_for_discord::text::install(
            minecraft_fishingbot_manager_for_discord::text::Language::English,
        );
    }

    /// 注册前，指令说明必须已替换为当前语言表中的文案。
    #[test]
    fn command_descriptions_come_from_the_language_table() {
        install_english_text();
        let commands = all_commands().expect("指令文案应完整");
        let command_texts = minecraft_fishingbot_manager_for_discord::text::get().command_texts();

        let start = commands
            .iter()
            .find(|command| command.name == "start")
            .expect("应当有 /start");
        let expected_start_description = command_texts
            .iter()
            .find(|command_text| command_text.name_path == "start")
            .expect("表里应当有 start")
            .description;
        assert_eq!(
            start.description.as_deref(),
            Some(expected_start_description)
        );

        // 选项说明也被覆盖（`/log` 的 `lines`）。
        let log = commands
            .iter()
            .find(|command| command.name == "log")
            .expect("应当有 /log");
        let lines = log
            .parameters
            .iter()
            .find(|parameter| parameter.name == "lines")
            .expect("应当有 lines");
        let expected_lines_description = command_texts
            .iter()
            .find(|command_text| command_text.name_path == "log")
            .expect("表里应当有 log")
            .options
            .iter()
            .find(|(name, _)| *name == "lines")
            .expect("表里应当有 lines")
            .1;
        assert_eq!(
            lines.description.as_deref(),
            Some(expected_lines_description)
        );
    }

    #[test]
    fn anti_afk_command_has_only_on_and_off_choices() {
        let commands = declared_commands();
        let command = commands
            .iter()
            .find(|command| command.name == "anti-afk")
            .expect("anti-afk must be registered");
        assert_eq!(command.parameters.len(), 1);
        let option = &command.parameters[0];
        assert_eq!(option.name, "command");
        assert!(option.required);
        let choices: Vec<&str> = option
            .choices
            .iter()
            .map(|choice| choice.name.as_ref())
            .collect();
        assert_eq!(choices, ["on", "off"]);
    }

    /// 一条指令展开后的说明：`(指令路径, 指令说明, [(选项名, 选项说明)])`。
    type FlatDescription = (String, Option<String>, Vec<(String, Option<String>)>);

    /// 整棵指令树展开后的说明列表。
    type FlatDescriptions = Vec<FlatDescription>;

    /// 将指令树展开为说明列表，用于比较宏声明文案与本地化结果。
    fn descriptions(
        commands: &[poise::Command<Data, Error>],
        parent_path: &str,
    ) -> FlatDescriptions {
        let mut collected = Vec::new();

        for command in commands {
            let name_path = if parent_path.is_empty() {
                command.name.to_string()
            } else {
                format!("{parent_path} {}", command.name)
            };
            let options = command
                .parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.to_string(),
                        parameter.description.as_ref().map(|text| text.to_string()),
                    )
                })
                .collect();
            collected.push((
                name_path.clone(),
                command.description.as_ref().map(|text| text.to_string()),
                options,
            ));

            collected.extend(descriptions(&command.subcommands, &name_path));
        }

        collected
    }

    #[test]
    fn both_languages_populate_the_entire_command_tree() {
        for (_, description, options) in descriptions(&declared_commands(), "") {
            assert!(description.is_none(), "宏中不应保留命令说明");
            assert!(options.iter().all(|(_, description)| description.is_none()));
        }

        for i18n_text in text::all_languages() {
            let mut commands = declared_commands();
            localize_command_texts(&mut commands, i18n_text.command_texts(), *i18n_text)
                .expect("文案表应完整且有效");
            for (path, description, options) in descriptions(&commands, "") {
                let expected = i18n_text
                    .command_texts()
                    .iter()
                    .find(|entry| entry.name_path == path)
                    .expect("必须有命令文案");
                assert_eq!(description.as_deref(), Some(expected.description), "{path}");
                for (name, description) in options {
                    let expected_option = expected.options.iter().find(|(key, _)| *key == name);
                    assert_eq!(
                        description.as_deref(),
                        expected_option.map(|(_, description)| *description),
                        "{path} {name}"
                    );
                }
            }
            fn assert_registered_descriptions(value: &serde_json::Value) {
                let description = value["description"]
                    .as_str()
                    .expect("注册的指令和选项必须有说明");
                assert_ne!(description, "A slash command");
                assert_ne!(description, "A slash command parameter");
                if let Some(options) = value.get("options").and_then(|value| value.as_array()) {
                    for option in options {
                        assert_registered_descriptions(option);
                    }
                }
            }
            for command in commands {
                let builder = command
                    .create_as_slash_command()
                    .expect("声明的指令应可注册");
                let payload = serde_json::to_value(builder).expect("指令应可序列化");
                assert_registered_descriptions(&payload);
            }
        }
    }

    #[test]
    fn invalid_command_texts_are_rejected() {
        let i18n_text = text::all_languages()[1];
        let original = i18n_text.command_texts();
        let start = original
            .iter()
            .position(|entry| entry.name_path == "start")
            .unwrap();

        let check = |texts: &[CommandText], path: &str, problem: CommandTextProblem| {
            let error = localize_command_texts(&mut declared_commands(), texts, i18n_text)
                .expect_err("无效文案必须被拒绝");
            assert_eq!(error, i18n_text.command_text_invalid(path, problem));
        };

        let mut texts = original.to_vec();
        texts.remove(start);
        check(&texts, "start", CommandTextProblem::Missing);

        let mut texts = original.to_vec();
        texts[start].options = &[];
        check(&texts, "start name", CommandTextProblem::Missing);

        let mut texts = original.to_vec();
        texts.push(original[start]);
        check(&texts, "start", CommandTextProblem::Duplicate);

        let mut texts = original.to_vec();
        texts[start].options = &[("name", "bot name"), ("name", "duplicate")];
        check(&texts, "start name", CommandTextProblem::Duplicate);

        let mut texts = original.to_vec();
        texts[start].options = &[("name", "bot name"), ("unknown", "unknown")];
        check(&texts, "start unknown", CommandTextProblem::Unknown);

        let mut texts = original.to_vec();
        texts.push(CommandText {
            name_path: "unknown",
            description: "unknown",
            options: &[],
        });
        check(&texts, "unknown", CommandTextProblem::Unknown);

        let mut texts = original.to_vec();
        texts[start].description = "  ";
        check(
            &texts,
            "start",
            CommandTextProblem::InvalidLength { length: 2 },
        );

        let mut texts = original.to_vec();
        texts[start].options = &[("name", "")];
        check(
            &texts,
            "start name",
            CommandTextProblem::InvalidLength { length: 0 },
        );
    }

    #[test]
    fn duplicate_command_definitions_are_rejected() {
        let i18n_text = text::all_languages()[1];
        let mut commands = declared_commands();
        commands.push(bot_control::start());
        let error = localize_command_texts(&mut commands, i18n_text.command_texts(), i18n_text)
            .expect_err("重复命令必须被拒绝");
        assert_eq!(
            error,
            i18n_text.command_text_invalid("start", CommandTextProblem::Duplicate)
        );

        let mut commands = declared_commands();
        let duplicate_parameter = bot_control::start().parameters.pop().unwrap();
        let start = commands
            .iter_mut()
            .find(|command| command.name == "start")
            .unwrap();
        start.parameters.push(duplicate_parameter);
        let error = localize_command_texts(&mut commands, i18n_text.command_texts(), i18n_text)
            .expect_err("重复选项必须被拒绝");
        assert_eq!(
            error,
            i18n_text.command_text_invalid("start name", CommandTextProblem::Duplicate)
        );
    }

    #[test]
    fn description_length_uses_utf16_units() {
        let i18n_text = text::all_languages()[1];
        assert!(validate_description("start", &"🐟".repeat(50), i18n_text).is_ok());
        assert_eq!(
            validate_description("start", &"🐟".repeat(51), i18n_text).unwrap_err(),
            i18n_text
                .command_text_invalid("start", CommandTextProblem::InvalidLength { length: 102 })
        );
    }
}
