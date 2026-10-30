//! 持续向匹配前缀的 screen 会话发送反 AFK 输入。

use std::sync::Mutex;

use input_pulse::InputPulse;

use crate::config::Config;
use crate::text;

static ANTI_AFK: Mutex<Option<InputPulse>> = Mutex::new(None);

/// 启动反 AFK；返回是否新启动。线程启动不保证存在匹配会话或输入已送达。
pub(crate) async fn start_anti_afk() -> Result<bool, String> {
    let i18n_text = text::get();
    tokio::task::spawn_blocking(move || {
        let mut task = ANTI_AFK
            .lock()
            .map_err(|error| i18n_text.anti_afk_lock_failed(&error.to_string()))?;
        if task.as_ref().is_some_and(InputPulse::is_running) {
            return Ok(false);
        }
        if let Some(mut pulse) = task.take() {
            pulse
                .wait()
                .map_err(|error| i18n_text.anti_afk_worker_failed(&error))?;
        }

        let config = Config::get();
        let mut pulse = config
            .anti_afk
            .input_pulse_config(&config.runtime.screen_prefix)
            .and_then(InputPulse::new)
            .map_err(|error| i18n_text.anti_afk_config_failed(&error))?;
        pulse
            .start(|_| {})
            .map_err(|error| i18n_text.anti_afk_start_failed(&error))?;
        *task = Some(pulse);
        Ok(true)
    })
    .await
    .map_err(|error| i18n_text.anti_afk_start_failed(&error.to_string()))?
}

/// 请求停止并等待线程退出；未运行时返回错误。
/// 停止可能等待当前输入或外部系统调用结束，没有固定时间上限。
pub(crate) async fn stop_anti_afk() -> Result<(), String> {
    let i18n_text = text::get();
    tokio::task::spawn_blocking(move || {
        let mut task = ANTI_AFK
            .lock()
            .map_err(|error| i18n_text.anti_afk_lock_failed(&error.to_string()))?;
        let Some(mut pulse) = task.take() else {
            return Err(i18n_text.anti_afk_stop_not_running().to_string());
        };
        let running = pulse.is_running();
        // 等待退出期间保持持锁，避免旧线程停止前又启动新线程。
        pulse
            .stop()
            .map_err(|error| i18n_text.anti_afk_worker_failed(&error))?;
        if !running {
            return Err(i18n_text.anti_afk_stop_not_running().to_string());
        }
        Ok(())
    })
    .await
    .map_err(|error| i18n_text.anti_afk_stop_failed(&error.to_string()))?
}

/// 只检查线程是否存活，不回收线程或读取运行错误。
pub(crate) fn is_anti_afk_running() -> Result<bool, String> {
    let i18n_text = text::get();
    let task = ANTI_AFK
        .lock()
        .map_err(|error| i18n_text.anti_afk_lock_failed(&error.to_string()))?;
    Ok(task.as_ref().is_some_and(InputPulse::is_running))
}

#[cfg(test)]
mod tests {
    use super::*;
    use input_pulse::{InputPulseConfig, TargetMode};

    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn stopping_an_inactive_task_returns_error() {
        let _guard = TEST_LOCK.lock().await;
        assert!(!is_anti_afk_running().expect("query inactive task"));
        assert_eq!(
            stop_anti_afk().await.unwrap_err(),
            text::get().anti_afk_stop_not_running()
        );
        let config =
            InputPulseConfig::new(TargetMode::ScreenSession, "fb_", "!ENTER!", 180, 0.03).unwrap();
        *ANTI_AFK.lock().unwrap() = Some(InputPulse::new(config).unwrap());
        assert!(!is_anti_afk_running().expect("query inactive worker"));
        assert!(ANTI_AFK.lock().unwrap().is_some(), "query must not collect");

        assert_eq!(
            stop_anti_afk().await.unwrap_err(),
            text::get().anti_afk_stop_not_running()
        );
        assert!(ANTI_AFK.lock().unwrap().is_none());
        assert_eq!(
            stop_anti_afk().await.unwrap_err(),
            text::get().anti_afk_stop_not_running()
        );
        assert!(!is_anti_afk_running().expect("query stopped task"));
    }

    #[tokio::test]
    #[ignore = "requires GNU screen"]
    async fn running_worker_is_reported_and_stopped() {
        let _guard = TEST_LOCK.lock().await;
        let prefix = format!(
            "__anti_afk_test_{}_{:?}__",
            std::process::id(),
            std::time::SystemTime::now()
        );
        let config = InputPulseConfig::new(
            TargetMode::ScreenSession,
            prefix,
            "!/!!/!!ENTER!",
            180,
            0.03,
        )
        .unwrap();
        for _ in 0..2 {
            let mut pulse = InputPulse::new(config.clone()).unwrap();
            pulse.start(|_| {}).expect("start worker");
            *ANTI_AFK.lock().unwrap() = Some(pulse);

            let running = is_anti_afk_running();
            let duplicate = start_anti_afk().await;
            let stopped = stop_anti_afk().await;

            assert!(running.expect("query running worker"));
            assert!(!duplicate.expect("duplicate start"));
            stopped.expect("stop and join worker");
            assert!(!is_anti_afk_running().expect("query stopped worker"));
            assert_eq!(
                stop_anti_afk().await.unwrap_err(),
                text::get().anti_afk_stop_not_running()
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires GNU screen"]
    async fn stopped_worker_error_is_not_hidden() {
        let _guard = TEST_LOCK.lock().await;
        let config = InputPulseConfig::new(
            TargetMode::ScreenSession,
            "test-anti-afk",
            "!ENTER!",
            180,
            0.03,
        )
        .unwrap();
        let mut pulse = InputPulse::new(config).unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let mut sender = Some(sender);
        pulse
            .start(move |event| {
                if matches!(event, input_pulse::RunEvent::Started) {
                    if let Some(sender) = sender.take() {
                        let _ = sender.send(());
                    }
                    panic!("test worker failure");
                }
            })
            .expect("start worker");
        *ANTI_AFK.lock().unwrap() = Some(pulse);
        receiver.await.expect("worker initialized");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while is_anti_afk_running().expect("query worker") {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker exits");

        let error = stop_anti_afk()
            .await
            .expect_err("worker failure must be returned");
        assert!(error.contains("test worker failure"), "{error}");
        assert!(ANTI_AFK.lock().unwrap().is_none());
    }
}
