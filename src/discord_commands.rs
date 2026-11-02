//! Discord 命令层：解析参数、检查权限、调度业务并发送回复。
//! 同步业务操作使用 `spawn_blocking`，指令说明在注册前按配置语言填充。

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
use minecraft_fishingbot_manager_for_discord::{
    AllBotsControlAction, BotControlAction, BotManageAction, ShowAction, handle_all_bots_control,
    handle_bot_control, handle_bot_manage, handle_show,
};

pub(crate) use registration::try_register_missing_sync_commands;

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

/// 单机器人控制：发送进度回执，并在阻塞线程池执行路由。
pub(crate) async fn single_bot_control(
    ctx: Context<'_>,
    action: BotControlAction<'_>,
    name: String,
    caller_id: u64,
    started_notice: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    say_and_log(ctx, started_notice).await?;
    let reply = dispatch_bot_control(action, name, caller_id).await?;
    say_final_report(ctx, &reply).await
}

/// 单机器人账号管理；`/token` 的 Modal 回复单独处理。
pub(crate) async fn single_bot_manage(
    ctx: Context<'_>,
    action: BotManageAction,
    name: String,
    caller_id: u64,
) -> Result<(), Error> {
    ctx.defer().await?;
    let reply = dispatch_bot_manage(action, name, caller_id).await?;
    say_and_log(ctx, reply).await
}

/// 全服批量管理：发送进度回执，并在阻塞线程池执行路由。
pub(crate) async fn all_bots_manage(
    ctx: Context<'_>,
    action: AllBotsControlAction<'_>,
    caller_id: u64,
    started_notice: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    say_and_log(ctx, started_notice).await?;
    let reply = dispatch_all_bots_control(action, caller_id).await?;
    say_final_report(ctx, &reply).await
}

/// 只读查询：在线程池执行路由并发送结果。
pub(crate) async fn read_only_query(
    ctx: Context<'_>,
    action: ShowAction,
    caller_id: u64,
) -> Result<(), Error> {
    ctx.defer().await?;
    let reply = tokio::task::spawn_blocking(move || handle_show(action, caller_id)).await?;
    say_and_log(ctx, reply).await
}

async fn dispatch_bot_control(
    action: BotControlAction<'_>,
    name: String,
    caller_id: u64,
) -> Result<String, tokio::task::JoinError> {
    let action = match action {
        BotControlAction::Start => BotControlAction::Start,
        BotControlAction::Stop => BotControlAction::Stop,
        BotControlAction::Restart => BotControlAction::Restart,
        BotControlAction::Pause => BotControlAction::Pause,
        BotControlAction::Unpause => BotControlAction::Unpause,
        BotControlAction::Command(args) => {
            let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
            return tokio::task::spawn_blocking(move || {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                handle_bot_control(BotControlAction::Command(&args), &name, caller_id)
            })
            .await;
        }
    };

    tokio::task::spawn_blocking(move || handle_bot_control(action, &name, caller_id)).await
}

async fn dispatch_bot_manage(
    action: BotManageAction,
    name: String,
    caller_id: u64,
) -> Result<String, tokio::task::JoinError> {
    tokio::task::spawn_blocking(move || handle_bot_manage(action, &name, caller_id)).await
}

async fn dispatch_all_bots_control(
    action: AllBotsControlAction<'_>,
    caller_id: u64,
) -> Result<String, tokio::task::JoinError> {
    let action = match action {
        AllBotsControlAction::StartAll => AllBotsControlAction::StartAll,
        AllBotsControlAction::StopAll => AllBotsControlAction::StopAll,
        AllBotsControlAction::RestartAll => AllBotsControlAction::RestartAll,
        AllBotsControlAction::PauseAll => AllBotsControlAction::PauseAll,
        AllBotsControlAction::UnpauseAll => AllBotsControlAction::UnpauseAll,
        AllBotsControlAction::CommandAll(args) => {
            let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
            return tokio::task::spawn_blocking(move || {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                handle_all_bots_control(AllBotsControlAction::CommandAll(&args), caller_id)
            })
            .await;
        }
    };

    tokio::task::spawn_blocking(move || handle_all_bots_control(action, caller_id)).await
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

/// 发送最终报告；交互回复失败时改发频道消息，以应对 15 分钟的令牌有效期。
async fn say_final_report(ctx: Context<'_>, report: &str) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    let command_path = ctx.command().qualified_name.clone();

    if let Err(error) = say_and_log(ctx, report).await {
        // 使用绝对路径区分 `log` crate 与同名指令模块。
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

    #[test]
    fn dispatch_preserves_actions_arguments_and_permissions() {
        const CHILD: &str = "FISHBOT_DISPATCH_TEST_CHILD";
        const ADMIN: u64 = 123_456_789_012_345_678;
        const OWNER: u64 = 987_654_321;
        const OTHER: u64 = 987_654_322;

        if std::env::var_os(CHILD).is_some() {
            minecraft_fishingbot_manager_for_discord::config::Config::get_or_init();
            let i18n = text::get();
            let counter = std::env::var_os("FISHBOT_DISPATCH_COUNTER").unwrap();
            let payload = std::env::var_os("FISHBOT_DISPATCH_PAYLOAD").unwrap();
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                assert_eq!(
                    dispatch_bot_control(BotControlAction::Stop, "duck".into(), OTHER)
                        .await
                        .unwrap(),
                    i18n.bot_dir_not_found("duck")
                );
                assert_eq!(
                    dispatch_all_bots_control(AllBotsControlAction::CommandAll(&["/help"]), OTHER)
                        .await
                        .unwrap(),
                    i18n.admin_required(OTHER)
                );
                for action in [
                    AllBotsControlAction::PauseAll,
                    AllBotsControlAction::UnpauseAll,
                ] {
                    assert_eq!(
                        dispatch_all_bots_control(action, OTHER).await.unwrap(),
                        i18n.admin_required(OTHER)
                    );
                }
                assert!(!std::path::Path::new(&counter).exists());

                let fragments = [" /say", "hello  world  "];
                let command = " /say hello  world  ";
                assert_eq!(
                    dispatch_bot_control(
                        BotControlAction::Command(&fragments),
                        "duck".into(),
                        OWNER,
                    )
                    .await
                    .unwrap(),
                    i18n.bot_command_sent("duck", command)
                );
                assert_eq!(
                    std::fs::read_to_string(&payload).unwrap(),
                    format!("{command}\n")
                );

                let report =
                    dispatch_all_bots_control(AllBotsControlAction::CommandAll(&fragments), ADMIN)
                        .await
                        .unwrap();
                assert!(report.contains(command), "{report}");
                assert_eq!(
                    std::fs::read_to_string(&payload).unwrap(),
                    format!("{command}\n{command}\n")
                );
                for reply in [
                    dispatch_bot_control(BotControlAction::Command(&[]), "duck".into(), OWNER)
                        .await
                        .unwrap(),
                    dispatch_all_bots_control(AllBotsControlAction::CommandAll(&[]), ADMIN)
                        .await
                        .unwrap(),
                ] {
                    assert_eq!(reply, i18n.bot_command_empty());
                }

                assert_eq!(
                    dispatch_bot_manage(BotManageAction::Add, "swan".into(), OWNER)
                        .await
                        .unwrap(),
                    i18n.bot_add_succeeded("swan")
                );
                let bots_dir = &minecraft_fishingbot_manager_for_discord::config::Config::get()
                    .paths
                    .bots_dir;
                assert!(bots_dir.join(OWNER.to_string()).join("swan").is_dir());
                assert_eq!(
                    dispatch_bot_control(BotControlAction::Stop, "duck".into(), OWNER)
                        .await
                        .unwrap(),
                    i18n.bot_stop_succeeded("duck")
                );
            });
            return;
        }

        use std::os::unix::fs::PermissionsExt;
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "fishingbot-dispatch-{}-{unique}",
            std::process::id()
        ));
        let data = root.join("data");
        let bot = data.join("bots").join(OWNER.to_string()).join("duck");
        let bin = root.join("bin");
        std::fs::create_dir_all(&bot).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bot.join("refreshToken"), "test token").unwrap();
        let mut config: toml::Value = toml::from_str(include_str!(
            "../minecraft-fishingbot-manager-for-discord.conf.example.toml"
        ))
        .unwrap();
        config["paths"]["base_dir"] = toml::Value::String(data.to_str().unwrap().into());
        config["runtime"]["screen_prefix"] = toml::Value::String("lookup_".into());
        config["timing"]["stop_grace"] = toml::Value::Float(0.001);
        config["timing"]["stop_settle"] = toml::Value::Float(0.001);
        std::fs::write(data.join(config["paths"]["jar"].as_str().unwrap()), b"jar").unwrap();
        std::fs::write(
            root.join(minecraft_fishingbot_manager_for_discord::config::CONFIG_FILE_NAME),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        let screen = bin.join("screen");
        std::fs::write(
            &screen,
            r#"#!/bin/sh
case "$1" in
  -list)
    printf x >> "$FISHBOT_DISPATCH_COUNTER"
    if [ -f "$FISHBOT_DISPATCH_COUNTER.stopped" ]; then
      printf 'No Sockets found in /tmp/test.\n'
      exit 1
    fi
    printf 'There is a screen on:\n\t123.lookup_duck\t(Detached)\n'
    ;;
  -S)
    if [ "$2" != "lookup_duck" ] || [ "$3" != "-X" ] || [ "$4" != "stuff" ]; then exit 2; fi
    printf '%s' "$5" >> "$FISHBOT_DISPATCH_PAYLOAD"
    ;;
  -X)
    if [ "$2" != "-S" ] || [ "$3" != "lookup_duck" ] || [ "$4" != "stuff" ]; then exit 2; fi
    : > "$FISHBOT_DISPATCH_COUNTER.stopped"
    ;;
  *) exit 2 ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&screen, std::fs::Permissions::from_mode(0o700)).unwrap();
        let executable = root.join("dispatch-test");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let output = std::process::Command::new(executable)
            .args([
                "--exact",
                "discord_commands::tests::dispatch_preserves_actions_arguments_and_permissions",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("FISHBOT_DISPATCH_COUNTER", root.join("counter"))
            .env("FISHBOT_DISPATCH_PAYLOAD", root.join("payload"))
            .env("PATH", &bin)
            .output()
            .expect("run isolated dispatch test");
        std::fs::remove_dir_all(&root).expect("remove dispatch test directory");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// 测试进程的语言实现是进程级单例；需要文案的测试先安装英语。
    fn install_english_text() {
        minecraft_fishingbot_manager_for_discord::text::install(
            minecraft_fishingbot_manager_for_discord::text::Language::English,
        );
    }

    #[test]
    fn command_descriptions_come_from_the_language_table() {
        install_english_text();
        let commands = all_commands().expect("指令文案应完整");
        let i18n_text = text::get();
        let mut expected = declared_commands();
        localize_command_texts(&mut expected, i18n_text.command_texts(), i18n_text)
            .expect("指令文案应完整");
        assert_eq!(descriptions(&commands, ""), descriptions(&expected, ""));
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

    #[test]
    fn registered_command_names_and_subcommands_are_stable() {
        let commands = declared_commands();
        let names: Vec<_> = commands
            .iter()
            .map(|command| command.name.as_ref())
            .collect();
        assert_eq!(
            names,
            [
                "help",
                "whoami",
                "ping",
                "checkserver",
                "sync-commands",
                "show",
                "bot",
                "token",
                "log",
                "start",
                "stop",
                "restart",
                "pause",
                "unpause",
                "command",
                "startall",
                "stopall",
                "restartall",
                "pauseall",
                "unpauseall",
                "commandall",
                "anti-afk",
            ]
        );
        for (name, expected) in [
            (
                "show",
                &["status", "botconfig", "mybots", "allbots", "admins"][..],
            ),
            ("bot", &["add", "del", "rename", "login"][..]),
        ] {
            let command = commands
                .iter()
                .find(|command| command.name == name)
                .unwrap();
            let subcommands: Vec<_> = command
                .subcommands
                .iter()
                .map(|subcommand| subcommand.name.as_ref())
                .collect();
            assert_eq!(subcommands, expected);
        }
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
