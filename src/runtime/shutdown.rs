//! 管理器进程退出策略；机器人停止与复核由 bot_instances::stop 负责。
use super::{bot_instances::stop, discord_notices::notify_guilds};
use crate::config::Config;
use poise::serenity_prelude as serenity;

/// 报告永久错误并以退出码 1 结束；配置未加载时不能安全地识别机器人。
pub fn exit_on_permanent_failure(reason: &str) -> ! {
    report_permanent_error(reason);
    if Config::is_loaded() {
        match stop::shutdown_all_bots() {
            Ok(report) if log::log_enabled!(log::Level::Info) => log::info!("{report}"),
            Ok(report) => eprintln!("{report}"),
            Err(error) => report_permanent_error(&error),
        }
    }
    std::process::exit(1);
}

fn report_permanent_error(reason: &str) {
    if log::log_enabled!(log::Level::Error) {
        log::error!("{reason}");
    } else {
        eprintln!("{reason}");
    }
}

/// 收到退出信号时停止机器人，有通知上下文时限时发送报告，然后以 0 退出。
pub async fn exit_on_shutdown_signal(notice_ctx: Option<serenity::Context>) -> ! {
    let config = Config::get();
    let text = config.text();
    log::info!("{}", text.shutdown_signal());
    let report = match stop::stop_all_bot_sessions() {
        Ok(outcome) => {
            let mut report = outcome.into_report();
            report.set_title(text.shutdown_signal_report_title());
            report.render()
        }
        Err(message) => {
            log::warn!("{message}");
            message
        }
    };
    log::info!("{report}");

    if let Some(ctx) = notice_ctx {
        let timeout = config.constants.shutdown_report_timeout;
        let send = notify_guilds(&ctx, &report, |guild, reason| {
            log::warn!(
                "{}",
                crate::bootstrap_error_text().shutdown_report_notice_failed(guild, reason)
            );
        });
        match tokio::time::timeout(timeout, send).await {
            Ok((notified, failed)) => {
                log::info!("{}", text.shutdown_report_notified(notified, failed))
            }
            Err(_) => log::warn!(
                "{}",
                crate::bootstrap_error_text().shutdown_report_timeout(timeout.as_secs())
            ),
        }
    } else {
        log::warn!(
            "{}",
            crate::bootstrap_error_text().shutdown_report_no_context()
        );
    }
    std::process::exit(0);
}
