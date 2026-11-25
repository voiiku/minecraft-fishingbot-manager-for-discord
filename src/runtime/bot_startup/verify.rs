//! 启动后检查会话；宽限期后再次检查会话并查询 FishingBot。

use std::{thread, time::Duration};

use super::{LaunchEntries, StartIssue, VerifiedStarts};
use crate::runtime::bot_instances::bot_status::query_bot_status;
use crate::runtime::bot_instances::screen::{screen_session_snapshot, session_name};

pub(super) fn verify_start_results(
    mut launches: LaunchEntries,
    check_grace: Duration,
) -> VerifiedStarts {
    for delay in [Duration::ZERO, check_grace] {
        let mut pending = launches
            .iter_mut()
            .filter(|(_, result)| result.is_ok())
            .peekable();
        if pending.peek().is_none() {
            break;
        }
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        let snapshot = screen_session_snapshot();
        for (bot, result) in pending {
            let name = &bot.bot_name;
            match &snapshot {
                Err(error) => {
                    *result = Err(verification_failure(
                        name,
                        crate::config::Config::get().text().bot_line(name, error),
                    ))
                }
                Ok(sessions) => {
                    let alive = session_name(name)
                        .ok()
                        .and_then(|key| sessions.get(&key))
                        .is_some_and(|session| !session.is_dead);
                    if !alive {
                        *result = Err(verification_failure(
                            name,
                            crate::config::Config::get().text().start_failed(name),
                        ));
                    }
                }
            }
        }
    }

    launches
        .into_iter()
        .map(|(bot, launch)| {
            let result = launch.and_then(|()| {
                query_bot_status(&bot, None)
                    .and_then(|status| {
                        status.ok_or_else(|| {
                            crate::config::Config::get().text().bot_line(
                                &bot.bot_name,
                                crate::config::Config::get().text().bot_status_unavailable(),
                            )
                        })
                    })
                    .map_err(|error| verification_failure(&bot.bot_name, error))
            });
            (bot.bot_name, result)
        })
        .collect()
}

fn verification_failure(bot_name: &str, reason: String) -> StartIssue {
    log::warn!(
        "{}",
        crate::config::Config::get()
            .text()
            .start_verification_failed(bot_name, &reason)
    );
    StartIssue::Failure(reason)
}
