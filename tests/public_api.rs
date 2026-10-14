//! 集成测试：测试公开的接口

use std::time::{Duration, Instant};

use minecraft_fishingbot_manager_for_discord::text::{self, Language};
use minecraft_fishingbot_manager_for_discord::{Watchdog, handle_whoami};

/// `/whoami` ：任何调用者都能拿到自己的用户 ID（权限要求是 `Everyone`）。
#[test]
fn whoami_reply_contains_the_caller_id() {
    let caller_id = 123_456_789_012_345_678u64;

    let reply = handle_whoami(caller_id);

    assert!(reply.contains(&caller_id.to_string()), "{reply}");
}

/// 语言取值：两种支持的语言都能解析，别的报错。
#[test]
fn language_names_are_parsed() {
    let supported_names = Language::supported_names();
    assert!(supported_names.contains(&"ENGLISH"));
    assert!(supported_names.contains(&"CHINESE_SIMPLIFIED"));

    assert!(Language::from_name("ENGLISH").is_ok());
    assert!(Language::from_name("CHINESE_SIMPLIFIED").is_ok());
    // jar 认识但本项目没实现的取值（上游枚举里还有 11 个）。
    assert!(Language::from_name("GERMAN").is_err());
    // 旧配置里那种写法不再接受（没有映射表了，取值就是 jar 的常量名）。
    assert!(Language::from_name("zh-CN").is_err());
}

/// 每种语言都要能把指令说明表交给外部，且同一语言里 `name_path` 不重复。
#[test]
fn every_language_exposes_its_command_texts() {
    for language in text::all_languages() {
        let command_texts = language.command_texts();
        assert!(!command_texts.is_empty());

        let mut name_paths: Vec<&str> = command_texts.iter().map(|text| text.name_path).collect();
        let total = name_paths.len();
        name_paths.sort_unstable();
        name_paths.dedup();
        assert_eq!(name_paths.len(), total, "name_path 有重复");
    }
}

/// 装配失败归类：三种原因与永久错误。
#[test]
fn setup_failures_are_classified_from_outside() {
    assert!(matches!(
        text::setup_failure_log_text("Missing Access"),
        text::LogText::DiscordInitFailedNotInvited { .. }
    ));
    assert!(matches!(
        text::setup_failure_log_text("Invalid Token"),
        text::LogText::DiscordInitFailedBadToken { .. }
    ));
    assert!(matches!(
        text::setup_failure_log_text("connection refused"),
        text::LogText::DiscordInitFailed { .. }
    ));

    assert!(text::failure_is_permanent(
        &text::LogText::DiscordInitFailedNotInvited {
            error: "Missing Access"
        }
    ));
    assert!(!text::failure_is_permanent(
        &text::LogText::DiscordInitFailed {
            error: "connection refused"
        }
    ));
}

/// 看门狗的公开入口：时序由调用方给（不必读配置），记账走它的方法。
/// 判定结果（`Alive`/`Waiting`/`NeverReady`/`Unreachable`）是看门狗内部的事，不在公开面上。
#[test]
fn watchdog_bookkeeping_can_be_driven_from_outside() {
    let watchdog = Watchdog::new(
        Instant::now(),
        Duration::from_secs(600),
        Duration::from_secs(200),
        Duration::from_secs(1200),
    );

    // 新一轮尝试：握手与探活的计时都从此刻重来。
    watchdog.begin_connection_attempt(Instant::now());
    // 记录本次握手；是不是进程内第一次由 `is_first_handshake` 单独回答（入口据此只打印一次就绪日志）。
    watchdog.record_handshake_success(Instant::now());
    assert!(watchdog.is_first_handshake());
    assert!(!watchdog.is_first_handshake());
    // 探活结果只是记账：成功清掉失败计时，失败记下首次失败时刻。
    watchdog.clear_probe_failures();
    watchdog.record_probe_failure_start(Instant::now());
}
