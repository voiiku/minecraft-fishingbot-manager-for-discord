//! 命令层：定义斜杠指令，取参数、校验数字、defer，并把结果交给 `lib::handle_*`。
//! 权限与业务逻辑在下层；同步业务调用统一放进 `spawn_blocking`。
//!
//! 命令说明和选项说明写在宏里的英文是兜底文案，注册前会按配置语言覆盖。
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
use std::future::Future;

use minecraft_fishingbot_manager_for_discord::text;

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

/// 返回待注册的指令列表（顺序就是 Discord 中的显示顺序）。
///
/// 宏里的说明是英文兜底；注册前按 `[i18n] language` 覆盖指令与选项说明。
pub(crate) fn all_commands() -> Vec<poise::Command<Data, Error>> {
    let mut commands = declared_commands();
    localize_command_texts(&mut commands, "");

    commands
}

/// 创建宏生成的英文兜底指令树，供本地化与测试使用。
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
    ]
}

/// 用配置语言的说明覆盖宏里的说明；`parent_path` 是父指令路径（顶层传空串，子指令拼成
/// `"bot add"` 这种以空格分隔的路径）。
///
/// 只覆盖表里找得到的项：表里没有的保留宏的文档注释（不会静默出错，
/// 下面的 `every_command_and_option_has_text_in_every_language` 测试会把缺口报出来）。
fn localize_command_texts(commands: &mut [poise::Command<Data, Error>], parent_path: &str) {
    let i18n_text = text::get();

    for command in commands.iter_mut() {
        let name_path = if parent_path.is_empty() {
            command.name.to_string()
        } else {
            format!("{parent_path} {}", command.name)
        };

        if let Some(command_text) = i18n_text
            .command_texts()
            .iter()
            .find(|command_text| command_text.name_path == name_path)
        {
            command.description = Some(Cow::Borrowed(command_text.description));

            for (option_name, option_description) in command_text.options {
                if let Some(parameter) = command
                    .parameters
                    .iter_mut()
                    .find(|parameter| parameter.name.as_ref() == *option_name)
                {
                    parameter.description = Some(Cow::Borrowed(option_description));
                }
            }
        }

        localize_command_texts(&mut command.subcommands, &name_path);
    }
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

/// 异步执行服务器探测并发送结果。
pub(crate) async fn server_probe(
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
            "Failed to send the fallback response for /{command_path} to Discord user {caller_id}: {error}"
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

    /// 将指令树展开为 `(指令路径, 选项名列表)`，路径示例：`"start"`、`"bot add"`。
    fn command_paths(
        commands: &[poise::Command<Data, Error>],
        parent_path: &str,
    ) -> Vec<(String, Vec<String>)> {
        let mut paths = Vec::new();

        for command in commands {
            let name_path = if parent_path.is_empty() {
                command.name.to_string()
            } else {
                format!("{parent_path} {}", command.name)
            };

            let option_names = command
                .parameters
                .iter()
                .map(|parameter| parameter.name.to_string())
                .collect();
            paths.push((name_path.clone(), option_names));

            paths.extend(command_paths(&command.subcommands, &name_path));
        }

        paths
    }

    /// 注册前，指令说明必须已替换为当前语言表中的文案。
    #[test]
    fn command_descriptions_come_from_the_language_table() {
        install_english_text();
        let commands = all_commands();
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

    /// 宏声明的文案副本必须与 `en_us.rs` 中的英文表逐字一致。
    ///
    /// poise 宏会在编译期将 `///` 首段与 `#[description]` 写入指令定义；
    /// 语言表缺项时，这些副本会被直接注册到 Discord。该测试同时验证：
    /// 宏副本与英文表一致，且本地化后的指令树与宏声明结构一致。
    #[test]
    fn macro_texts_match_the_english_table() {
        install_english_text();
        let declared = descriptions(&declared_commands(), "");
        let localized = descriptions(&all_commands(), "");

        assert_eq!(declared.len(), localized.len(), "指令树结构必须一致");
        for (declared_command, localized_command) in declared.iter().zip(&localized) {
            assert_eq!(declared_command.0, localized_command.0, "指令路径必须一致");
            assert_eq!(
                declared_command.1, localized_command.1,
                "指令 {} 的说明必须与英文表一致",
                declared_command.0
            );
            assert_eq!(
                declared_command.2, localized_command.2,
                "指令 {} 的选项说明必须与英文表一致",
                declared_command.0
            );
        }
    }

    /// 指令与选项说明必须落在 Discord 的 100 字符上限内。
    ///
    /// Discord 按 UTF-16 码元计数，与 `utils/reply.rs` 的消息长度口径一致。
    /// 命令说明超限时 poise 会在编译期报错；选项说明没有编译期检查，需要此测试覆盖。
    #[test]
    fn every_description_fits_discords_100_character_limit() {
        for text_impl in minecraft_fishingbot_manager_for_discord::text::all_languages() {
            for command_text in text_impl.command_texts() {
                let length = command_text.description.encode_utf16().count();
                assert!(
                    (1..=100).contains(&length),
                    "指令 {} 的说明 {length} 字符（上限 100）",
                    command_text.name_path
                );

                for (option_name, option_description) in command_text.options {
                    let length = option_description.encode_utf16().count();
                    assert!(
                        (1..=100).contains(&length),
                        "指令 {} 的选项 {option_name} 说明 {length} 字符（上限 100）",
                        command_text.name_path
                    );
                }
            }
        }
    }

    /// 每种语言的说明表必须覆盖全部指令与选项。
    ///
    /// 该测试发现“新增指令或选项但未补充文案”的遗漏，并列出缺失项。
    /// 直接读取语言表，不调用 `install`，避免污染并发测试的全局语言状态。
    #[test]
    fn every_command_and_option_has_text_in_every_language() {
        install_english_text();
        let paths = command_paths(&all_commands(), "");

        let mut missing: Vec<String> = Vec::new();
        for text_impl in minecraft_fishingbot_manager_for_discord::text::all_languages() {
            let command_texts = text_impl.command_texts();

            for (name_path, option_names) in &paths {
                let Some(command_text) = command_texts
                    .iter()
                    .find(|command_text| command_text.name_path == *name_path)
                else {
                    missing.push(format!("缺少指令说明: {name_path}"));
                    continue;
                };

                for option_name in option_names {
                    if !command_text
                        .options
                        .iter()
                        .any(|(name, _)| name == option_name)
                    {
                        missing.push(format!("缺少选项说明: {name_path} {option_name}"));
                    }
                }
            }
        }

        assert!(
            missing.is_empty(),
            "指令说明表有缺口:\n{}",
            missing.join("\n")
        );
    }
}
