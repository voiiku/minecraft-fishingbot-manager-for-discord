//! 初始化配置与运行组件，再持续管理 Discord 连接。
//! 运行时的永久连接错误会停止机器人并退出；可重试错误会停止机器人并等待重连。

use minecraft_fishingbot_manager_for_discord::discord_commands;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use poise::serenity_prelude as serenity;

use discord_commands::{Data, Error, try_register_missing_sync_commands};
use minecraft_fishingbot_manager_for_discord::config;
use minecraft_fishingbot_manager_for_discord::{
    ProcessLog, Watchdog, bootstrap_error_text, complete_startup, regenerate_fishingbot_config,
    runtime::{bot_startup::StartupQueue, shutdown},
    text,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 日志、看门狗与启动队列均依赖已加载的配置。
    config::Config::load().unwrap_or_else(|message| shutdown::exit_on_permanent_failure(&message));
    let settings = config::Config::get();
    let _ = ProcessLog::get_or_init();
    let watchdog = Watchdog::get_or_init();
    let _startup_queue = StartupQueue::get_or_init();

    if let Err(message) = regenerate_fishingbot_config() {
        shutdown::exit_on_permanent_failure(&message);
    }

    let mut attempt_count: u32 = 0;
    let discord_bot_token = &settings.discord.token;
    // 网关可能仍保持连接但无法通信，另用 REST 请求探活。
    let probe_http = Arc::new(serenity::Http::new(discord_bot_token));
    let intents = serenity::GatewayIntents::GUILDS;
    loop {
        attempt_count += 1;
        watchdog.begin_connection_attempt(Instant::now());
        let framework = build_framework()
            .unwrap_or_else(|message| shutdown::exit_on_permanent_failure(&message));

        let stop_probe = Arc::new(AtomicBool::new(false));
        watchdog.probe(Arc::clone(&probe_http), Arc::clone(&stop_probe));

        let mut client = match serenity::ClientBuilder::new(discord_bot_token, intents)
            .framework(framework)
            .await
        {
            Ok(client) => Some(client),
            Err(e) => {
                stop_probe.store(true, Ordering::Relaxed);
                let reason = setup_failure_reason(&e.to_string());
                log::warn!(
                    "{}",
                    bootstrap_error_text().reconnect_failed(
                        attempt_count,
                        watchdog.window(),
                        &reason
                    )
                );
                None
            }
        };
        if let Some(client) = client.as_mut() {
            watchdog.wait_for_attempt(client).await;
            stop_probe.store(true, Ordering::Relaxed);

            // 关闭旧连接，避免下一轮重复连接。
            client.shard_manager.shutdown_all().await;
            log::warn!(
                "{}",
                bootstrap_error_text().reconnect_scheduled(attempt_count + 1, watchdog.window())
            );
        }

        watchdog.wait_before_reconnect().await;
    }
}

/// 永久错误触发退出，其余错误返回重试原因。
fn setup_failure_reason(error: &str) -> String {
    let (message, is_permanent) = setup_failure_details(error, bootstrap_error_text());
    if is_permanent {
        shutdown::exit_on_permanent_failure(&message);
    }
    message
}

/// 根据 Discord 错误分类并生成日志文案。
fn setup_failure_details(error: &str, language: &dyn text::Text) -> (String, bool) {
    if ["Missing Access", "Unknown Guild"]
        .iter()
        .any(|phrase| error.contains(phrase))
    {
        (language.discord_init_failed_not_invited(error), true)
    } else if ["Unauthorized", "Invalid Token", "401"]
        .iter()
        .any(|phrase| error.contains(phrase))
    {
        (language.discord_init_failed_bad_token(error), true)
    } else {
        (language.discord_init_failed(error), false)
    }
}

/// 装配 poise 框架、确保手动同步入口存在并打印邀请链接。
/// 每次重建连接都要重新装配。
fn build_framework() -> Result<poise::Framework<Data, Error>, String> {
    async fn on_event(
        framework_ctx: poise::FrameworkContext<'_, Data, Error>,
        event: &serenity::FullEvent,
    ) -> Result<(), Error> {
        if matches!(
            event,
            serenity::FullEvent::Ready { .. } | serenity::FullEvent::Resume { .. }
        ) {
            let watchdog = Watchdog::get_or_init();
            if watchdog.record_handshake_success(Instant::now()) {
                log::info!(
                    "{}",
                    config::Config::get().text().watchdog_ready(
                        watchdog.window(),
                        watchdog.probe_interval(),
                        watchdog.first_handshake_timeout()
                    )
                );
            }

            watchdog.remember_notice_context(framework_ctx.serenity_context);
            watchdog
                .notify_reconnect_success(framework_ctx.serenity_context)
                .await;
        }

        Ok(())
    }

    async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
        match error {
            poise::FrameworkError::Setup { error, .. } => {
                let watchdog = Watchdog::get_or_init();
                watchdog.request_reconnect(setup_failure_reason(&error.to_string()));
            }
            poise::FrameworkError::Command { error, ctx, .. } => {
                log::error!(
                    "{}",
                    bootstrap_error_text().command_run_failed(&error.to_string())
                );
                discord_commands::reply_command_failure(ctx).await;
            }
            poise::FrameworkError::CommandPanic { payload, ctx, .. } => {
                log::error!(
                    "{}",
                    bootstrap_error_text()
                        .command_run_failed(payload.as_deref().unwrap_or("command panicked"))
                );
                discord_commands::reply_command_failure(ctx).await;
            }
            other => {
                log::warn!(
                    "{}",
                    bootstrap_error_text().framework_error(&other.to_string())
                );
            }
        }
    }

    Ok(poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: discord_commands::all_commands()?,
            event_handler: |framework_ctx, event| Box::pin(on_event(framework_ctx, event)),
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                let application_id = ready.user.id;
                log::info!(
                    "{}",
                    config::Config::get()
                        .text()
                        .discord_connected(application_id.get())
                );
                log::info!(
                    "{}",
                    config::Config::get().text().invite_url(&invite_url(
                        application_id,
                        config::Config::get().constants.discord_invite_permissions
                    ))
                );

                try_register_missing_sync_commands(
                    &ctx.http,
                    &framework.options().commands,
                    &config::Config::get().discord.guild_ids,
                )
                .await?;

                complete_startup();
                Ok(Data)
            })
        })
        .build())
}

/// 用应用 ID 拼出邀请链接。
/// 权限位写在链接中（可在机器人页面查看）：
/// View Channels `0x400` | Send Messages `0x800` | Embed Links `0x2000` | Attach Files `0x4000` | Read Message History `0x8000` = 60416。
fn invite_url(application_id: serenity::UserId, permissions: u64) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={}&scope=bot+applications.commands&permissions={permissions}",
        application_id.get(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 邀请链接应申请的最小权限集合。
    const EXPECTED_PERMISSIONS: u64 = 0x400 | 0x800 | 0x2000 | 0x4000 | 0x8000;
    /// Discord 权限位：Administrator。
    const ADMINISTRATOR: u64 = 0x8;
    /// Discord 权限位：Manage Server。
    const MANAGE_GUILD: u64 = 0x20;
    /// Discord 权限位：Mention Everyone。
    const MENTION_EVERYONE: u64 = 0x1_0000;

    #[test]
    fn setup_failures_keep_their_category_and_retry_policy() {
        for language in text::all_languages() {
            for error in ["Missing Access", "Unknown Guild", "Missing Access: 401"] {
                assert_eq!(
                    setup_failure_details(error, *language),
                    (language.discord_init_failed_not_invited(error), true)
                );
            }
            for error in ["Unauthorized", "Invalid Token", "HTTP 401"] {
                assert_eq!(
                    setup_failure_details(error, *language),
                    (language.discord_init_failed_bad_token(error), true)
                );
            }
            let error = "connection refused";
            assert_eq!(
                setup_failure_details(error, *language),
                (language.discord_init_failed(error), false)
            );
        }
    }

    /// 邀请链接的权限位必须与最小权限集合一致，且不得包含高权限位。
    #[test]
    fn invite_url_carries_the_documented_permissions() {
        let permissions = permissions_in(&invite_url(
            serenity::UserId::new(123),
            EXPECTED_PERMISSIONS,
        ));

        assert_eq!(permissions, EXPECTED_PERMISSIONS);
        assert_eq!(
            permissions & ADMINISTRATOR,
            0,
            "不得申请 Administrator 权限"
        );
        assert_eq!(permissions & MANAGE_GUILD, 0, "不得申请 Manage Server 权限");
        assert_eq!(
            permissions & MENTION_EVERYONE,
            0,
            "不得申请 Mention Everyone 权限"
        );
    }

    /// 解析邀请链接中 `permissions` 查询参数的十进制值。
    fn permissions_in(url: &str) -> u64 {
        let (_, after_permissions) = url
            .split_once("permissions=")
            .expect("邀请链接必须包含 permissions 参数");
        let raw_value = after_permissions
            .split('&')
            .next()
            .expect("permissions 参数必须有值");

        raw_value.parse().expect("permissions 参数必须是十进制整数")
    }

    /// 邀请链接必须同时包含 `bot` 与 `applications.commands` 作用域。
    /// 缺少 `applications.commands` 时，斜杠命令注册会返回 `50001 Missing Access`。
    #[test]
    fn invite_url_carries_both_scopes() {
        let url = invite_url(serenity::UserId::new(123), EXPECTED_PERMISSIONS);

        assert!(url.contains("scope=bot+applications.commands"), "{url}");
        assert_eq!(url.matches("permissions=").count(), 1, "{url}");
    }
}
