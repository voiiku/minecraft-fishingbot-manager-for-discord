//! 当遇到永久错误时（如 token 配置错误 / 机器人没被邀请加入服务器），会显式报错、记录进日志并以 1 退出，不再自动重启。
//! 当遇到可重试的错误时（网络错误），进程会停止全部钓鱼机器人，等待并尝试重建连接，而不会自行结束进程。
//!
//! 启动顺序：
//!
//! 1. 取配置：`config::Config::get_or_init()` 会尝试读取可执行文件同级目录的 `minecraft-fishingbot-manager-for-discord.conf.toml`，
//!    解析、校验与安装语言都在里面做；配置文件不存在时它写好模板并打印提示，进程正常退出（退出码 0）；
//!    当某项配置不合法，它就带着字段名报错、以退出码 1 结束进程；
//!    当配置解析成功后，语言会被立刻安装，此后的报错与日志都跟随配置语言（在语言成功安装前，默认使用英语报错/提示）
//! 2. `ProcessLog::get_or_init()`：创建本次进程日志，并向终端与 `<部署目录>/logs/时间戳.log` 实时写入日志信息，日志创建完成后立刻写入进程信息；
//! 3. `WATCHDOG.get_or_init()`：显式获取并自动初始化进程内唯一的看门狗
//! 4. `regenerate_fishingbot_config()`：生成钓鱼机器人的配置，失败是永久错误，以退出码 1 退出；
//! 5. `wait_for_attempt()` 等这一次尝试结束（正常运行时它一直挂着、不返回），三个结束方式：
//!    `client.start()` 返回（客户端被关停，token / Intent 非法等致命错误；普通掉线不会返回）
//!    看门狗判定已经掉线。
//!    收到系统退出信号。只有收到退出信号时才彻底结束（退出码 0）。
//! 6. 掉线处理：看门狗检测到掉线-停止全部机器人-关闭旧连接-等一个判定窗口后尝试重连 → 回到第 5 步；
//!    重连成功后向本机器人所在的所有服务器通报一次"何时掉线、现在恢复、机器人已停机"的通知。

mod commands;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use poise::serenity_prelude as serenity;

use commands::{Data, Error, register_commands};
use minecraft_fishingbot_manager_for_discord::config;
use minecraft_fishingbot_manager_for_discord::{
    ProcessLog, WATCHDOG, Watchdog, regenerate_fishingbot_config, text,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 获取配置
    // 没有配置文件时将生成模板、打印提示后以 0 退出。
    // 配置不合法时报错并以退出码 1 退出。
    let settings = config::Config::get_or_init();
    // 2. 初始化本次运行的程序日志（进程内唯一，首次调用建日志文件并装上 logger）。
    // 失败时不影响程序运行，输出终端警告；级别由配置文件中的 `[log] debug` 决定。
    let _ = ProcessLog::get_or_init();

    // 3. 初始化看门狗，进程内唯一，不存在就创建。
    // 所有关于程序状态、信息获取与对不同生命周期的处理逻辑都通过看门狗进行。
    let watchdog = WATCHDOG.get_or_init();

    // 4. 根据配置中的[fishingbot]表，重新生成钓鱼机器人的配置，会覆盖旧文件。
    // 失败是永久错误，以退出码 1 退出。
    if let Err(message) = regenerate_fishingbot_config() {
        watchdog.exit_on_permanent_failure(&message);
    }

    // 5. 进入连接循环，每一轮就是一次尝试连接到Discord的过程，并阻塞等待不同失败结果以执行对应处理逻辑
    // 正常运行时会阻塞在第 8 步，直到客户端返回、看门狗判定掉线或收到退出信号。
    // 尝试连接的次数
    let mut attempt_times: u32 = 0;
    // Discord 机器人的 token，用于连接 Discord API
    let discord_bot_token = &settings.discord.token;
    // 探活使用的 REST 句柄：可能保持连接但故障收不到内容。
    // 看门狗需要定期主动检查网络，让每轮探活任务各拿一份句柄，同时避免冲突和消耗所有权。
    let probe_http = Arc::new(serenity::Http::new(discord_bot_token));
    // Intents 决定 Discord 推送哪些事件给本机器人。
    // 必须订阅 GUILDS：它提供服务器的结构信息（频道、成员列表等），其余可以不订阅。
    let intents = serenity::GatewayIntents::GUILDS;
    loop {
        attempt_times += 1;
        // 每次进入循环重新计时等待时长，旧的握手与探活都被重置为 None。
        watchdog.begin_connection_attempt(Instant::now());

        // 6. 启动一个新的探活任务
        // 创建停止探活标志，每轮重置，用于判断是否应该提前结束。
        let stop_probe = Arc::new(AtomicBool::new(false));
        watchdog.probe(Arc::clone(&probe_http), Arc::clone(&stop_probe));

        // 7. 装配 client，与discord通信的通道
        // 当clint.start()返回时，说明客户端本身出现严重错误
        let mut client = match serenity::ClientBuilder::new(discord_bot_token, intents)
            .framework(build_framework())
            .await
        {
            // 如果装配客户端成功，就获取客户端
            Ok(client) => Some(client),
            Err(e) => {
                // 如果装配客户端失败，就设置停止探活标志为 true
                stop_probe.store(true, Ordering::Relaxed);
                // 失败原因记录到日志，如果是永久错误就在分流里（setup_failure_reason）执行停机并以退出码 1 结束本进程。
                let reason = setup_failure_reason(watchdog, &e.to_string());
                log::warn!(
                    "{}",
                    text::get().log_text(text::LogText::ReconnectFailed {
                        attempt: attempt_times,
                        seconds: watchdog.window().as_secs(),
                        reason: &reason,
                    })
                );
                None
            }
        };
        // 8. 成功构造 client 后，阻塞等待这一次尝试结束，再按结束方式收尾。
        // 尝试结束的概念：表示出现意外，正常运行时相当不会出现尝试结束，会在这一步一直等待
        // [`wait_for_attempt`]会一直等待三个结束标志中的一个出现，谁先出现就先进入对应的逻辑
        // 三种结束分别是:
        // 客户端返回：客户端只有遇到严重错误时才会返回，掉线不会返回，而是静默尝试重连。
        // 看门狗认定掉线：由于客户端掉线时不返回，所以需要让看门狗定时主动探测网络是否正常，超时就认定掉线，停止全部机器人
        // 收到退出信号：当进程收到退出信号时，执行永久退出，不再自动重启。
        if let Some(client) = client.as_mut() {
            // 阻塞等待这一次尝试结束，并按结束方式收尾。
            watchdog.wait_for_attempt(client).await;
            // 尝试结束时，停止本轮的探活任务。
            stop_probe.store(true, Ordering::Relaxed);

            // 9. 上轮可能有残留，关掉全部连接，防止重复连接造成冲突。
            client.shard_manager.shutdown_all().await;
            // 尝试结束就说明下线了，记录为警告
            log::warn!(
                "{}",
                text::get().log_text(text::LogText::ReconnectScheduled {
                    attempt: attempt_times + 1,
                    seconds: watchdog.window().as_secs(),
                })
            );
        }

        // 10. 等待一个判定窗口再重建连接；期间收到退出信号就直接退出进程。
        watchdog.wait_before_reconnect().await;
    }
}

/// 根据错误，获取错误文本，并判断是否是永久性错误，写入日志
/// 如果是永久性错误（如 token 错误、机器人没被邀请进服务器），就执行永久退出流程
/// 否则返回渲染好的失败文案，交给调用方当重试原因
fn setup_failure_reason(watchdog: &Watchdog, error: &str) -> String {
    let failure = text::setup_failure_log_text(error);

    if text::failure_is_permanent(&failure) {
        watchdog.exit_on_permanent_failure(&text::get().log_text(failure));
    }

    text::get().log_text(failure)
}

/// 装配 poise 框架，注册指令并打印邀请链接，事件与错误交给内部定义的两个处理接口。
/// 每次重建连接都要重新装配。
fn build_framework() -> poise::Framework<Data, Error> {
    /// 1. 定义框架的事件处理接口：gateway 的事件转给看门狗。
    async fn on_event(
        framework_ctx: poise::FrameworkContext<'_, Data, Error>,
        event: &serenity::FullEvent,
    ) -> Result<(), Error> {
        // 检查握手状态，`Ready` / `Resume` 都算成功握手，第一次握手时输出看门狗已就绪。
        if matches!(
            event,
            serenity::FullEvent::Ready { .. } | serenity::FullEvent::Resume { .. }
        ) {
            // 是不是进程内第一次成功握手由 `is_first_handshake` 判断
            let watchdog = WATCHDOG.get_or_init();
            // 先记录成功（清除失败记录）再判断是不是进程内第一次成功握手。
            watchdog.record_handshake_success(Instant::now());
            if watchdog.is_first_handshake() {
                log::info!(
                    "{}",
                    text::get().log_text(text::LogText::WatchdogReady {
                        window_secs: watchdog.window().as_secs(),
                        probe_interval_secs: watchdog.probe_interval().as_secs(),
                        first_handshake_timeout_secs: watchdog.first_handshake_timeout().as_secs(),
                    })
                );
            }

            // 记下这次握手的上下文：收到退出信号时的收尾要用它把结果发到各服务器。
            watchdog.remember_notice_context(framework_ctx.serenity_context);

            // 掉线又重连后发送这条通报。
            watchdog
                .notify_reconnect_success(framework_ctx.serenity_context)
                .await;
        }

        Ok(())
    }

    /// 2. 定义错误处理接口：命令层与装配阶段的错误出口。
    async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
        match error {
            poise::FrameworkError::Setup { error, .. } => {
                let watchdog = WATCHDOG.get_or_init();
                // 装配失败按成因分流，永久错误就退出进程
                // 其余的当掉线处理，把原因交给进程入口，由它停机并重建连接。
                watchdog.request_reconnect(setup_failure_reason(watchdog, &error.to_string()));
            }
            poise::FrameworkError::Command { error, ctx, .. } => {
                log::error!(
                    "{}",
                    text::get().log_text(text::LogText::CommandRunFailed {
                        error: &error.to_string(),
                    })
                );
                commands::reply_command_failure(ctx).await;
            }
            poise::FrameworkError::CommandPanic { payload, ctx, .. } => {
                log::error!(
                    "{}",
                    text::get().log_text(text::LogText::CommandRunFailed {
                        error: payload.as_deref().unwrap_or("command panicked"),
                    })
                );
                commands::reply_command_failure(ctx).await;
            }
            other => {
                log::warn!(
                    "{}",
                    text::get().log_text(text::LogText::FrameworkError {
                        detail: &other.to_string(),
                    })
                );
            }
        }
    }

    poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all_commands(),
            event_handler: |framework_ctx, event| Box::pin(on_event(framework_ctx, event)),
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                let application_id = ready.user.id;
                log::info!(
                    "{}",
                    text::get().log_text(text::LogText::DiscordConnected {
                        application_id: application_id.get(),
                    })
                );
                // 装配成功就输出邀请链接。
                log::info!(
                    "{}",
                    text::get().log_text(text::LogText::InviteUrl {
                        url: &invite_url(application_id),
                    })
                );

                // 指令全局注册，确认当前全局指令已经在 Discord 稳定存在后，尝试清理各服务器上遗留的 guild 指令。
                register_commands(ctx, &framework.options().commands, &ready.guilds).await?;

                Ok(Data)
            })
        })
        .build()
}

/// 用应用 ID 拼出邀请链接。
/// 权限位写在链接中（可在机器人页面查看）：
/// View Channels `0x400` | Send Messages `0x800` | Embed Links `0x2000` | Attach Files `0x4000` | Read Message History `0x8000` = 60416。
fn invite_url(application_id: serenity::UserId) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={}&scope=bot+applications.commands&permissions=60416",
        application_id.get()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 链接里那串权限位要和注释逐位一致：少了 Embed Links，带链接的回复不会展开；
    /// 多了 Mention Everyone / Administrator 就是白送权限。
    #[test]
    fn invite_url_carries_the_documented_permissions() {
        let permissions = permissions_in(&invite_url(serenity::UserId::new(123)));

        assert_eq!(permissions, 0x400 | 0x800 | 0x2000 | 0x4000 | 0x8000);
        assert_eq!(permissions & 0x8, 0, "不能要 Administrator");
        assert_eq!(permissions & 0x20, 0, "不能要 Manage Server");
        assert_eq!(permissions & 0x1_0000, 0, "不能要 Mention Everyone");
    }

    /// 从邀请链接里取出 `permissions=` 那串数字：断言的是链接里真正写着的东西，不是另一个副本。
    fn permissions_in(url: &str) -> u64 {
        url.split("permissions=")
            .nth(1)
            .expect("邀请链接要带 permissions 参数")
            .parse()
            .expect("permissions 参数应当是十进制数字")
    }

    /// 邀请链接必须同时带 `bot` 与 `applications.commands`：少了后者，机器人即使进了服，
    /// 装配阶段注册斜杠指令也会被 Discord 回 `50001 Missing Access`。
    #[test]
    fn invite_url_carries_both_scopes() {
        let url = invite_url(serenity::UserId::new(123));

        assert!(url.contains("scope=bot+applications.commands"), "{url}");
        assert_eq!(url.matches("permissions=").count(), 1, "{url}");
    }
}
