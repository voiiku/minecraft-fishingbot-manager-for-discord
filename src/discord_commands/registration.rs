//! 手动同步配置的指令范围，并清理旧范围。

use poise::serenity_prelude as serenity;
use tokio::sync::Mutex;

use minecraft_fishingbot_manager_for_discord::{
    check_sync_commands_permission, config, text, utils::Report,
};

use super::{Context, Data, Error};

const SYNC_COMMAND_NAME: &str = "sync-commands";
static COMMAND_SYNC_LOCK: Mutex<()> = Mutex::const_new(());

/// 只在同步入口不存在时创建它，不覆盖同范围内的其他指令。
pub(crate) async fn try_register_missing_sync_commands(
    http: &serenity::Http,
    commands: &[poise::Command<Data, Error>],
    guild_ids: &[u64],
) -> Result<(), Error> {
    let sync_command = commands
        .iter()
        .find(|command| command.name == SYNC_COMMAND_NAME)
        .and_then(poise::Command::create_as_slash_command)
        .ok_or_else(|| std::io::Error::other("sync-commands is not a slash command"))?;

    if guild_ids.is_empty() {
        let registered_commands = serenity::Command::get_global_commands(http).await?;
        if sync_command_is_missing(&registered_commands) {
            serenity::Command::create_global_command(http, sync_command).await?;
        }
    } else {
        for configured_guild_id in guild_ids {
            let guild_id = serenity::GuildId::new(*configured_guild_id);
            let registration_result = async {
                let registered_commands = guild_id.get_commands(http).await?;
                if sync_command_is_missing(&registered_commands) {
                    guild_id.create_command(http, sync_command.clone()).await?;
                }
                Ok::<(), serenity::Error>(())
            }
            .await;
            if let Err(error) = registration_result {
                log::warn!(
                    "{}",
                    text::get().command_sync_bootstrap_failed(
                        &configured_guild_id.to_string(),
                        &error.to_string()
                    )
                );
            }
        }
    }
    Ok(())
}

fn sync_command_is_missing(registered_commands: &[serenity::Command]) -> bool {
    !registered_commands.iter().any(|command| {
        command.name == SYNC_COMMAND_NAME && command.kind == serenity::CommandType::ChatInput
    })
}

#[poise::command(slash_command, rename = "sync-commands")]
pub(crate) async fn sync_commands(ctx: Context<'_>) -> Result<(), Error> {
    let caller_id = ctx.author().id.get();
    if let Err(reply) = check_sync_commands_permission(caller_id) {
        return super::say_and_log(ctx, reply).await;
    }

    let _guard = match COMMAND_SYNC_LOCK.try_lock() {
        Ok(guard) => guard,
        Err(_) => return super::say_and_log(ctx, text::get().command_sync_in_progress()).await,
    };
    ctx.defer().await?;
    super::say_and_log(ctx, text::get().command_started_notice("/sync-commands")).await?;

    let serenity_ctx = ctx.serenity_context();
    let mut joined_guild_ids = serenity_ctx.cache.guilds();
    joined_guild_ids.sort_unstable();
    joined_guild_ids.dedup();
    let joined_guilds = joined_guild_ids
        .into_iter()
        .map(|guild_id| {
            let guild_name = serenity_ctx
                .cache
                .guild(guild_id)
                .map(|guild| guild.name.clone())
                .unwrap_or_else(|| guild_id.to_string());
            (guild_id, guild_name)
        })
        .collect();
    let reply = match sync_commands_and_clear_old_scopes(
        ctx.http(),
        &ctx.framework().options.commands,
        &config::Config::get().discord.guild_ids,
        joined_guilds,
    )
    .await
    {
        Ok(report) => report.render(),
        Err(reason) => text::get().command_sync_failed(&reason),
    };
    super::say_final_report(ctx, &reply).await
}

async fn sync_commands_and_clear_old_scopes(
    http: &serenity::Http,
    commands: &[poise::Command<Data, Error>],
    configured_guild_ids: &[u64],
    joined_guilds: Vec<(serenity::GuildId, String)>,
) -> Result<Report, String> {
    let mut report = Report::new();
    report.set_title(text::get().command_sync_title());

    if configured_guild_ids.is_empty() {
        poise::builtins::register_globally(http, commands)
            .await
            .map_err(|error| error.to_string())?;
        report
            .basics
            .push(text::get().command_sync_registered().to_string());
        clear_guild_commands_now(http, joined_guilds, &mut report).await;
        return Ok(report);
    }

    let mut updated_guild_count = 0;
    for configured_guild_id in configured_guild_ids {
        let guild_id = serenity::GuildId::new(*configured_guild_id);
        let guild_name = joined_guilds
            .iter()
            .find(|(candidate, _)| *candidate == guild_id)
            .map_or_else(|| configured_guild_id.to_string(), |(_, name)| name.clone());
        match poise::builtins::register_in_guild(http, commands, guild_id).await {
            Ok(()) => {
                updated_guild_count += 1;
                let message = text::get().command_sync_guild_updated(&guild_name);
                log::info!("{message}");
                report.succeeded.push(message);
            }
            Err(error) => {
                let message =
                    text::get().command_sync_guild_failed(&guild_name, &error.to_string());
                log::warn!("{message}");
                report.failed.push(message);
            }
        }
    }

    let failed_updates = configured_guild_ids.len() - updated_guild_count;
    if failed_updates > 0 {
        report
            .basics
            .push(text::get().command_sync_cleanup_skipped().to_string());
    } else if let Err(error) = clear_global_commands_now(http).await {
        let message = text::get().command_sync_global_clear_failed(&error);
        log::warn!("{message}");
        report.failed.push(message);
        report
            .basics
            .push(text::get().command_sync_cleanup_skipped().to_string());
    } else {
        let stale_guilds = joined_guilds
            .into_iter()
            .filter(|(guild_id, _)| !configured_guild_ids.contains(&guild_id.get()))
            .collect();
        clear_guild_commands_now(http, stale_guilds, &mut report).await;
    }
    report.basics.insert(
        0,
        text::get().command_sync_guild_summary(
            configured_guild_ids.len(),
            updated_guild_count,
            failed_updates,
        ),
    );
    Ok(report)
}

async fn clear_global_commands_now(http: &serenity::Http) -> Result<(), String> {
    let registered_global_commands = serenity::Command::get_global_commands(http)
        .await
        .map_err(|error| error.to_string())?;
    if !registered_global_commands.is_empty() {
        serenity::Command::set_global_commands(http, Vec::new())
            .await
            .map_err(|error| error.to_string())?;
        let message = text::get().command_sync_global_cleared(registered_global_commands.len());
        log::info!("{message}");
    }
    Ok(())
}

async fn clear_guild_commands_now(
    http: &serenity::Http,
    guilds: Vec<(serenity::GuildId, String)>,
    report: &mut Report,
) {
    let guild_count = guilds.len();
    let initial_success_count = report.succeeded.len();
    let initial_failure_count = report.failed.len();
    for (guild_id, guild_name) in guilds {
        match guild_id.get_commands(http).await {
            Ok(registered_guild_commands) if registered_guild_commands.is_empty() => {}
            Ok(registered_guild_commands) => match guild_id.set_commands(http, Vec::new()).await {
                Ok(_) => {
                    let message = text::get().log_text(text::LogText::GuildCommandsCleared {
                        guild_name: &guild_name,
                        count: registered_guild_commands.len(),
                    });
                    log::info!("{message}");
                    report.succeeded.push(message);
                }
                Err(error) => record_guild_clear_failure(report, &guild_name, &error.to_string()),
            },
            Err(error) => record_guild_clear_failure(report, &guild_name, &error.to_string()),
        }
    }

    if guild_count > 0 {
        report.basics.push(text::get().command_sync_summary(
            guild_count,
            report.succeeded.len() - initial_success_count,
            report.failed.len() - initial_failure_count,
        ));
    }
}

fn record_guild_clear_failure(report: &mut Report, guild_name: &str, reason: &str) {
    let message =
        text::get().log_text(text::LogText::GuildCommandsClearFailed { guild_name, reason });
    log::warn!("{message}");
    report.failed.push(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct ExpectedRequest {
        method: &'static str,
        path: &'static str,
        status: &'static str,
        response: String,
        body: Option<serde_json::Value>,
    }

    impl ExpectedRequest {
        fn new(
            method: &'static str,
            path: &'static str,
            status: &'static str,
            response: impl Into<String>,
        ) -> Self {
            Self {
                method,
                path,
                status,
                response: response.into(),
                body: None,
            }
        }

        fn with_body(mut self, body: serde_json::Value) -> Self {
            self.body = Some(body);
            self
        }
    }

    async fn mock_discord(
        expected: Vec<ExpectedRequest>,
    ) -> (serenity::Http, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for expected in expected {
                let (mut stream, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(3), listener.accept())
                        .await
                        .expect("expected Discord request")
                        .unwrap();
                let mut request = Vec::new();
                let (header_end, content_length) = loop {
                    let mut buffer = [0; 4096];
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0, "incomplete HTTP headers");
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(position) =
                        request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                    {
                        let header_end = position + 4;
                        let headers = std::str::from_utf8(&request[..position]).unwrap();
                        let first_line = headers.lines().next().unwrap();
                        assert!(
                            first_line.starts_with(&format!("{} ", expected.method))
                                && first_line.contains(expected.path),
                            "{first_line}"
                        );
                        let content_length = headers
                            .lines()
                            .filter_map(|line| line.split_once(':'))
                            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                            .unwrap_or(0);
                        break (header_end, content_length);
                    }
                };
                while request.len() - header_end < content_length {
                    let mut buffer = [0; 4096];
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0, "incomplete HTTP body");
                    request.extend_from_slice(&buffer[..count]);
                }
                if let Some(body) = expected.body {
                    let actual: serde_json::Value =
                        serde_json::from_slice(&request[header_end..header_end + content_length])
                            .unwrap();
                    assert_eq!(actual, body);
                }
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    expected.status,
                    expected.response.len(),
                    expected.response
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
                "unexpected extra Discord request"
            );
        });
        let http = serenity::http::HttpBuilder::new("test-token")
            .proxy(format!("http://{address}"))
            .application_id(serenity::ApplicationId::new(1))
            .ratelimiter_disabled(true)
            .build();
        (http, server)
    }

    fn command_json(name: &str, kind: u8) -> String {
        format!(
            r#"{{
                "id": "1", "application_id": "1", "guild_id": null,
                "name": "{name}", "name_localized": null, "name_localizations": null,
                "description": "d", "description_localized": null,
                "description_localizations": null, "default_member_permissions": null,
                "contexts": null, "version": "1", "handler": null, "type": {kind}
            }}"#
        )
    }

    #[test]
    fn startup_only_bootstraps_a_missing_slash_command() {
        fn registered(name: &str, kind: u8) -> serenity::Command {
            serde_json::from_str(&command_json(name, kind)).unwrap()
        }

        assert!(sync_command_is_missing(&[]));
        assert!(sync_command_is_missing(&[registered("help", 1)]));
        assert!(sync_command_is_missing(&[registered(SYNC_COMMAND_NAME, 2)]));
        assert!(!sync_command_is_missing(&[
            registered("help", 1),
            registered(SYNC_COMMAND_NAME, 1),
        ]));
        text::install(text::Language::English);
        let command = super::super::all_commands()
            .unwrap()
            .into_iter()
            .find(|command| command.name == SYNC_COMMAND_NAME)
            .unwrap()
            .create_as_slash_command()
            .unwrap();
        let payload = serde_json::to_value(command).unwrap();
        assert_eq!(payload["name"], SYNC_COMMAND_NAME);
    }

    #[tokio::test]
    async fn startup_creates_only_the_missing_entry_point() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let builder = commands
            .iter()
            .find(|command| command.name == SYNC_COMMAND_NAME)
            .unwrap()
            .create_as_slash_command()
            .unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new(
                "GET",
                "/applications/1/commands",
                "200 OK",
                format!("[{}]", command_json("help", 1)),
            ),
            ExpectedRequest::new(
                "POST",
                "/applications/1/commands",
                "200 OK",
                command_json(SYNC_COMMAND_NAME, 1),
            )
            .with_body(serde_json::to_value(builder).unwrap()),
        ])
        .await;
        try_register_missing_sync_commands(&http, &commands, &[])
            .await
            .unwrap();
        server.await.unwrap();

        let (http, server) = mock_discord(vec![ExpectedRequest::new(
            "GET",
            "/applications/1/commands",
            "200 OK",
            format!("[{}]", command_json(SYNC_COMMAND_NAME, 1)),
        )])
        .await;
        try_register_missing_sync_commands(&http, &commands, &[])
            .await
            .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn manual_sync_clears_old_commands_and_reports_partial_failures() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new("PUT", "/applications/1/commands", "200 OK", "[]").with_body(
                serde_json::from_slice(
                    &serde_json::to_vec(&poise::builtins::create_application_commands(&commands))
                        .unwrap(),
                )
                .unwrap(),
            ),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/11/commands",
                "200 OK",
                format!("[{}]", command_json("old", 1)),
            ),
            ExpectedRequest::new("PUT", "/applications/1/guilds/11/commands", "200 OK", "[]")
                .with_body(serde_json::json!([])),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/12/commands",
                "200 OK",
                format!("[{}]", command_json("old", 1)),
            ),
            ExpectedRequest::new(
                "PUT",
                "/applications/1/guilds/12/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            )
            .with_body(serde_json::json!([])),
            ExpectedRequest::new("GET", "/applications/1/guilds/13/commands", "200 OK", "[]"),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/14/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            ),
        ])
        .await;
        let guilds = [(11, "first"), (12, "second"), (13, "empty"), (14, "fourth")]
            .into_iter()
            .map(|(id, name)| (serenity::GuildId::new(id), name.to_string()))
            .collect();
        let report = sync_commands_and_clear_old_scopes(&http, &commands, &[], guilds)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(report.succeeded.len(), 1);
        assert!(report.succeeded[0].contains("first"));
        assert_eq!(report.failed.len(), 2);
        assert!(report.failed[0].contains("second"));
        assert!(report.failed[0].contains("/sync-commands"));
        assert!(report.failed[1].contains("fourth"));
        assert_eq!(report.basics[1], text::get().command_sync_summary(4, 1, 2));
    }

    #[tokio::test]
    async fn failed_global_update_does_not_clear_guild_commands() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let (http, server) = mock_discord(vec![ExpectedRequest::new(
            "PUT",
            "/applications/1/commands",
            "403 Forbidden",
            r#"{"message":"Missing Access","code":50001}"#,
        )])
        .await;
        let result = sync_commands_and_clear_old_scopes(
            &http,
            &commands,
            &[],
            vec![(serenity::GuildId::new(11), "first".into())],
        )
        .await;
        assert!(result.is_err());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn guild_bootstrap_continues_after_an_unavailable_server() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let builder = commands
            .iter()
            .find(|command| command.name == SYNC_COMMAND_NAME)
            .unwrap()
            .create_as_slash_command()
            .unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/11/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            ),
            ExpectedRequest::new("GET", "/applications/1/guilds/12/commands", "200 OK", "[]"),
            ExpectedRequest::new(
                "POST",
                "/applications/1/guilds/12/commands",
                "200 OK",
                command_json(SYNC_COMMAND_NAME, 1),
            )
            .with_body(serde_json::to_value(builder).unwrap()),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/13/commands",
                "200 OK",
                format!("[{}]", command_json(SYNC_COMMAND_NAME, 1)),
            ),
        ])
        .await;
        try_register_missing_sync_commands(&http, &commands, &[11, 12, 13])
            .await
            .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn guild_sync_updates_all_targets_before_removing_old_scopes() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let payload = serde_json::from_slice(
            &serde_json::to_vec(&poise::builtins::create_application_commands(&commands)).unwrap(),
        )
        .unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new("PUT", "/applications/1/guilds/11/commands", "200 OK", "[]")
                .with_body(payload),
            ExpectedRequest::new("PUT", "/applications/1/guilds/12/commands", "200 OK", "[]"),
            ExpectedRequest::new(
                "GET",
                "/applications/1/commands",
                "200 OK",
                format!("[{}]", command_json("old", 1)),
            ),
            ExpectedRequest::new("PUT", "/applications/1/commands", "200 OK", "[]")
                .with_body(serde_json::json!([])),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/13/commands",
                "200 OK",
                format!("[{}]", command_json("old", 1)),
            ),
            ExpectedRequest::new("PUT", "/applications/1/guilds/13/commands", "200 OK", "[]")
                .with_body(serde_json::json!([])),
            ExpectedRequest::new(
                "GET",
                "/applications/1/guilds/14/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            ),
        ])
        .await;
        let guilds = [
            (11, "first"),
            (12, "second"),
            (13, "old"),
            (14, "unavailable"),
        ]
        .into_iter()
        .map(|(id, name)| (serenity::GuildId::new(id), name.to_string()))
        .collect();
        let report = sync_commands_and_clear_old_scopes(&http, &commands, &[11, 12], guilds)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].contains("unavailable"));
        assert_eq!(report.succeeded.len(), 3);
        assert_eq!(
            report.basics[0],
            text::get().command_sync_guild_summary(2, 2, 0)
        );
        assert_eq!(report.basics[1], text::get().command_sync_summary(2, 1, 1));
        assert!(
            !report
                .succeeded
                .contains(&text::get().command_sync_global_cleared(1))
        );
    }

    #[tokio::test]
    async fn failed_guild_update_retains_global_and_unconfigured_commands() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new("PUT", "/applications/1/guilds/11/commands", "200 OK", "[]"),
            ExpectedRequest::new(
                "PUT",
                "/applications/1/guilds/12/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            ),
        ])
        .await;
        let guilds = [(11, "first"), (12, "second"), (13, "old")]
            .into_iter()
            .map(|(id, name)| (serenity::GuildId::new(id), name.to_string()))
            .collect();
        let report = sync_commands_and_clear_old_scopes(&http, &commands, &[11, 12], guilds)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(report.succeeded.len(), 1);
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].contains("second"));
        assert!(
            report
                .basics
                .contains(&text::get().command_sync_cleanup_skipped().to_string())
        );
    }

    #[tokio::test]
    async fn failed_global_cleanup_retains_unconfigured_guild_commands() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new("PUT", "/applications/1/guilds/11/commands", "200 OK", "[]"),
            ExpectedRequest::new(
                "GET",
                "/applications/1/commands",
                "200 OK",
                format!("[{}]", command_json("old", 1)),
            ),
            ExpectedRequest::new(
                "PUT",
                "/applications/1/commands",
                "403 Forbidden",
                r#"{"message":"Missing Access","code":50001}"#,
            )
            .with_body(serde_json::json!([])),
        ])
        .await;
        let guilds = [(11, "first"), (13, "old")]
            .into_iter()
            .map(|(id, name)| (serenity::GuildId::new(id), name.to_string()))
            .collect();
        let report = sync_commands_and_clear_old_scopes(&http, &commands, &[11], guilds)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].contains("global"));
        assert_eq!(
            report.basics[0],
            text::get().command_sync_guild_summary(1, 1, 0)
        );
        assert!(
            report
                .basics
                .contains(&text::get().command_sync_cleanup_skipped().to_string())
        );
    }

    #[tokio::test]
    async fn guild_sync_skips_deleting_an_empty_global_scope() {
        text::install(text::Language::English);
        let commands = super::super::all_commands().unwrap();
        let (http, server) = mock_discord(vec![
            ExpectedRequest::new("PUT", "/applications/1/guilds/11/commands", "200 OK", "[]"),
            ExpectedRequest::new("GET", "/applications/1/commands", "200 OK", "[]"),
        ])
        .await;
        let report = sync_commands_and_clear_old_scopes(
            &http,
            &commands,
            &[11],
            vec![(serenity::GuildId::new(11), "first".into())],
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(report.succeeded.len(), 1);
        assert!(report.failed.is_empty());
        assert_eq!(
            report.basics,
            [text::get().command_sync_guild_summary(1, 1, 0)]
        );
    }

    #[test]
    fn guild_sync_report_omits_empty_cleanup_and_global_detail() {
        const CHILD: &str = "FISHBOT_SYNC_REPORT_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            minecraft_fishingbot_manager_for_discord::config::Config::get_or_init();
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let commands = super::super::all_commands().unwrap();
                let old_commands = (0..20)
                    .map(|index| command_json(&format!("old_{index}"), 1))
                    .collect::<Vec<_>>()
                    .join(",");
                let (http, server) = mock_discord(vec![
                    ExpectedRequest::new(
                        "PUT",
                        "/applications/1/guilds/11/commands",
                        "200 OK",
                        "[]",
                    ),
                    ExpectedRequest::new(
                        "GET",
                        "/applications/1/commands",
                        "200 OK",
                        format!("[{old_commands}]"),
                    ),
                    ExpectedRequest::new("PUT", "/applications/1/commands", "200 OK", "[]")
                        .with_body(serde_json::json!([])),
                ])
                .await;
                let report = sync_commands_and_clear_old_scopes(
                    &http,
                    &commands,
                    &[11],
                    vec![(serenity::GuildId::new(11), "示例服务器".into())],
                )
                .await
                .unwrap();
                server.await.unwrap();
                assert_eq!(
                    report.render(),
                    "Discord 指令同步结果:\n=======================\n\
                     - 目标服务器 1 个；更新成功 1 个，更新失败 0 个\n\n\
                     [完成明细]:\n- 服务器 示例服务器 的指令已更新"
                );
            });
            return;
        }

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "fishingbot-sync-report-{}-{unique}",
            std::process::id()
        ));
        let data = root.join("data");
        std::fs::create_dir_all(&data).unwrap();
        let mut config: toml::Value = toml::from_str(include_str!(
            "../../minecraft-fishingbot-manager-for-discord.conf.example.toml"
        ))
        .unwrap();
        config["paths"]["base_dir"] = toml::Value::String(data.to_str().unwrap().into());
        config["i18n"]["language"] = toml::Value::String("CHINESE_SIMPLIFIED".into());
        std::fs::write(data.join(config["paths"]["jar"].as_str().unwrap()), b"jar").unwrap();
        std::fs::write(
            root.join(minecraft_fishingbot_manager_for_discord::config::CONFIG_FILE_NAME),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        let executable = root.join("sync-report-test");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let output = std::process::Command::new(executable)
            .args([
                "--exact",
                "discord_commands::registration::tests::guild_sync_report_omits_empty_cleanup_and_global_detail",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("run isolated sync report test");
        std::fs::remove_dir_all(&root).expect("remove sync report test directory");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
