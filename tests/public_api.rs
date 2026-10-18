//! 公开 API 的集成测试。

mod common;

use std::time::{Duration, Instant};

use minecraft_fishingbot_manager_for_discord::{Watchdog, handle_whoami};

/// `/whoami` 对任何调用者返回其自己的用户 ID。
#[test]
fn whoami_reply_contains_the_caller_id() {
    common::install_english();
    let caller_id = 123_456_789_012_345_678u64;

    let reply = handle_whoami(caller_id);

    assert!(reply.contains(&caller_id.to_string()), "{reply}");
}

/// 看门狗的公开方法允许外部推进计时状态；内部判定类型不对外暴露。
#[test]
fn watchdog_bookkeeping_can_be_driven_from_outside() {
    let watchdog = Watchdog::new(
        Instant::now(),
        Duration::from_secs(600),
        Duration::from_secs(200),
        Duration::from_secs(1200),
    );

    watchdog.begin_connection_attempt(Instant::now());
    watchdog.record_handshake_success(Instant::now());
    assert!(watchdog.is_first_handshake());
    assert!(!watchdog.is_first_handshake());
    watchdog.clear_probe_failures();
    watchdog.record_probe_failure_start(Instant::now());
}
