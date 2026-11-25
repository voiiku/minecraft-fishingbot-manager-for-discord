use super::*;
use crate::operations;
use crate::text;
use crate::text::report::IntoReplyText;

#[test]
fn control_routes_preserve_arguments_and_permissions() {
    const CHILD: &str = "FISHBOT_DISPATCH_TEST_CHILD";
    const ADMIN: u64 = 123_456_789_012_345_678;
    const OWNER: u64 = 987_654_321;
    const OTHER: u64 = 987_654_322;

    if std::env::var_os(CHILD).is_some() {
        crate::config::Config::load().expect("load test configuration");
        let i18n = crate::config::Config::get().text();
        let counter = std::env::var_os("FISHBOT_DISPATCH_COUNTER").unwrap();
        let payload = std::env::var_os("FISHBOT_DISPATCH_PAYLOAD").unwrap();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            tokio::task::spawn_blocking(move || {
                let stop = operations::single_bot_control::prepare_stop_single_bot;
                let prepared = stop("duck", OWNER).unwrap();
                assert_eq!(prepared.notice, i18n.command_started_notice("/stop duck"));
                assert!(stop("duck", ADMIN).is_ok());
                assert_eq!(
                    stop("duck", OTHER).err(),
                    Some(i18n.bot_dir_not_found("duck"))
                );
                assert_eq!(
                    stop("missing", OWNER).err(),
                    Some(i18n.bot_dir_not_found("missing"))
                );
                assert_eq!(
                    operations::all_bots_control::prepare_start_all_bots(OTHER).err(),
                    Some(i18n.admin_required(OTHER))
                );
                let bots_dir = &crate::config::Config::get().paths.bots_dir;
                let original = bots_dir.join(OWNER.to_string()).join("duck");
                let moved = bots_dir.join(OTHER.to_string()).join("duck");
                std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
                std::fs::rename(&original, &moved).unwrap();
                assert_eq!(prepared.execute(), Err(i18n.bot_dir_not_found("duck")));
                std::fs::rename(&moved, &original).unwrap();
                assert_eq!(
                    operations::single_bot_control::stop_single_bot("duck", OTHER).to_reply_text(),
                    i18n.bot_dir_not_found("duck")
                );
                assert_eq!(
                    operations::all_bots_control::send_all_bots_command(&["/help"], OTHER)
                        .to_reply_text(),
                    i18n.admin_required(OTHER)
                );
                for action in [
                    operations::all_bots_control::pause_all_bots
                        as fn(u64) -> Result<String, String>,
                    operations::all_bots_control::unpause_all_bots
                        as fn(u64) -> Result<String, String>,
                ] {
                    assert_eq!(action(OTHER).to_reply_text(), i18n.admin_required(OTHER));
                }
                assert!(!std::path::Path::new(&counter).exists());

                let other_bot = bots_dir.join(OTHER.to_string()).join("goose");
                std::fs::create_dir_all(&other_bot).unwrap();
                std::fs::write(other_bot.join("refreshToken"), "test token").unwrap();
                // duck 正在运行，goose 可启动。准备只发回估时，不应入队或拉起 goose。
                let start_all =
                    operations::all_bots_control::prepare_start_all_bots(ADMIN).unwrap();
                assert_eq!(
                    start_all.notice,
                    i18n.command_startall_queued_notice(1, 0, 2, 5)
                );
                drop(start_all);
                assert!(!other_bot.join("logs").exists());
                let start_all =
                    operations::all_bots_control::prepare_start_all_bots(ADMIN).unwrap();
                std::fs::remove_file(other_bot.join("refreshToken")).unwrap();
                assert_eq!(start_all.execute().unwrap(), i18n.startall_all_running());
                assert!(!other_bot.join("logs").exists());
                std::fs::write(other_bot.join("refreshToken"), "test token").unwrap();
                assert_eq!(
                    operations::queries::list_bot_candidates(OWNER).unwrap(),
                    vec![("duck".to_string(), true, false)]
                );
                assert_eq!(
                    operations::queries::list_bot_candidates(OTHER).unwrap(),
                    vec![("goose".to_string(), false, false)]
                );
                assert_eq!(
                    operations::queries::list_bot_candidates(ADMIN).unwrap(),
                    vec![
                        ("duck".to_string(), true, false),
                        ("goose".to_string(), false, false)
                    ]
                );

                let fragments = [" /say", "hello  world  "];
                let command = " /say hello  world  ";
                assert_eq!(
                    operations::single_bot_control::send_single_bot_command(
                        "duck", &fragments, OWNER,
                    )
                    .to_reply_text(),
                    i18n.bot_command_sent("duck", command)
                );
                assert_eq!(
                    std::fs::read_to_string(&payload).unwrap(),
                    format!("{command}\n")
                );

                let report = operations::all_bots_control::send_all_bots_command(&fragments, ADMIN)
                    .to_reply_text();
                assert!(report.contains(command), "{report}");
                assert_eq!(
                    std::fs::read_to_string(&payload).unwrap(),
                    format!("{command}\n{command}\n")
                );
                for reply in [
                    operations::single_bot_control::send_single_bot_command("duck", &[], OWNER)
                        .to_reply_text(),
                    operations::all_bots_control::send_all_bots_command(&[], ADMIN).to_reply_text(),
                ] {
                    assert_eq!(reply, i18n.bot_command_empty());
                }

                assert_eq!(
                    operations::accounts::add_bot("swan", OWNER).to_reply_text(),
                    i18n.bot_add_succeeded("swan")
                );
                let bots_dir = &crate::config::Config::get().paths.bots_dir;
                assert!(bots_dir.join(OWNER.to_string()).join("swan").is_dir());
                assert_eq!(
                    operations::single_bot_control::stop_single_bot("duck", OWNER).to_reply_text(),
                    i18n.bot_stop_succeeded("duck")
                );
            })
            .await
            .unwrap();
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
        "../../minecraft-fishingbot-manager-for-discord.conf.example.toml"
    ))
    .unwrap();
    config["paths"]["base_dir"] = toml::Value::String(data.to_str().unwrap().into());
    config["runtime"]["screen_prefix"] = toml::Value::String("lookup_".into());
    config["startup_queue"]["max_batch_size"] = toml::Value::Integer(1);
    config["startup_queue"]["max_pending_requests"] = toml::Value::Integer(2);
    config["startup_queue"]["batch_login_interval"] = toml::Value::Integer(61);
    config["timing"]["check_grace"] = toml::Value::Integer(1);
    config["timing"]["stop_grace"] = toml::Value::Float(0.001);
    config["timing"]["stop_settle"] = toml::Value::Float(0.001);
    std::fs::write(data.join(config["paths"]["jar"].as_str().unwrap()), b"jar").unwrap();
    std::fs::write(
        root.join(crate::test_support::TEST_CONFIG_FILE_NAME),
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
            "discord_commands::tests::control_routes_preserve_arguments_and_permissions",
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

/// 测试需要文案时显式加载示例配置。
fn init_test_config() {
    crate::config::init_for_test();
}

#[test]
fn command_descriptions_come_from_the_language_table() {
    config::init_for_test();
    init_test_config();
    let commands = all_commands().expect("指令文案应完整");
    let i18n_text = crate::config::Config::get().text();
    let constants = &config::Config::get().constants;
    let mut expected = declared_commands();
    localize_command_texts(
        &mut expected,
        i18n_text.command_texts(),
        i18n_text,
        constants.max_command_description_utf16,
    )
    .expect("指令文案应完整");
    localize_log_lines_description(
        &mut expected,
        i18n_text,
        i18n_text,
        constants.default_log_lines,
        constants.max_command_description_utf16,
    )
    .expect("日志行数说明应有效");
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
fn descriptions(commands: &[poise::Command<Data, Error>], parent_path: &str) -> FlatDescriptions {
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
        localize_command_texts(&mut commands, i18n_text.command_texts(), *i18n_text, 100)
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
        let error = localize_command_texts(&mut declared_commands(), texts, i18n_text, 100)
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
        CommandTextProblem::InvalidLength {
            length: 2,
            max: 100,
        },
    );

    let mut texts = original.to_vec();
    texts[start].options = &[("name", "")];
    check(
        &texts,
        "start name",
        CommandTextProblem::InvalidLength {
            length: 0,
            max: 100,
        },
    );
}

#[test]
fn duplicate_command_definitions_are_rejected() {
    let i18n_text = text::all_languages()[1];
    let mut commands = declared_commands();
    commands.push(bot_control::start());
    let error = localize_command_texts(&mut commands, i18n_text.command_texts(), i18n_text, 100)
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
    let error = localize_command_texts(&mut commands, i18n_text.command_texts(), i18n_text, 100)
        .expect_err("重复选项必须被拒绝");
    assert_eq!(
        error,
        i18n_text.command_text_invalid("start name", CommandTextProblem::Duplicate)
    );
}

#[test]
fn description_length_uses_utf16_units() {
    let i18n_text = text::all_languages()[1];
    assert!(validate_description("start", &"🐟".repeat(50), i18n_text, 100).is_ok());
    assert_eq!(
        validate_description("start", &"🐟".repeat(51), i18n_text, 100).unwrap_err(),
        i18n_text.command_text_invalid(
            "start",
            CommandTextProblem::InvalidLength {
                length: 102,
                max: 100
            }
        )
    );
}

#[test]
fn log_lines_description_uses_the_supplied_default() {
    for language in text::all_languages() {
        let mut commands = declared_commands();
        localize_log_lines_description(&mut commands, *language, *language, 73, 100).unwrap();
        let log = commands
            .iter()
            .find(|command| command.name == "log")
            .unwrap();
        let lines = log
            .parameters
            .iter()
            .find(|parameter| parameter.name == "lines")
            .unwrap();
        assert!(lines.description.as_deref().unwrap().contains("73"));
        assert!(
            localize_log_lines_description(&mut commands, *language, *language, 73, 1).is_err()
        );
    }
}
