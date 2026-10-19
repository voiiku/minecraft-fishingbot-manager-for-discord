//! 看门狗互斥锁中毒时的内部不变量文案。
//!
//! 这些文案只在看门狗代码使用，但属于运行期信息，必须随 `[i18n] language` 切换。
//! 通过 [`crate::text::Text::watchdog_text`] 读取静态字符串，避免在锁操作附近分配临时 `String`。

/// 看门狗内部不变量消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogText {
    /// 开始新一轮连接尝试时，`started` 的互斥锁中毒。
    StartedLockReset,
    /// 清除上一次握手状态时，`last_handshake` 的互斥锁中毒。
    LastHandshakeLockClear,
    /// 记录握手状态时，`last_handshake` 的互斥锁中毒。
    LastHandshakeLockRecord,
    /// 判定网络状态时，`last_handshake` 的互斥锁中毒。
    LastHandshakeLockRead,
    /// 清除探活失败状态时，`unhealthy_since` 的互斥锁中毒。
    UnhealthySinceLockClear,
    /// 记录探活失败状态时，`unhealthy_since` 的互斥锁中毒。
    UnhealthySinceLockRecord,
    /// 判定网络状态时，`unhealthy_since` 的互斥锁中毒。
    UnhealthySinceLockRead,
    /// 判定网络状态时，`started` 的互斥锁中毒。
    StartedLockRead,
    /// 记录掉线状态时，`offline_since` 的互斥锁中毒。
    OfflineSinceLockRecord,
    /// 读取掉线状态时，`offline_since` 的互斥锁中毒。
    OfflineSinceLockRead,
    /// 记录掉线原因时，`pending_disconnect_reason` 的互斥锁中毒。
    PendingDisconnectReasonLockRecord,
    /// 读取掉线原因时，`pending_disconnect_reason` 的互斥锁中毒。
    PendingDisconnectReasonLockRead,
    /// 发送收尾报告时，`notice_ctx` 的互斥锁中毒。
    NoticeCtxLockSendReport,
    /// 记录 Discord 上下文时，`notice_ctx` 的互斥锁中毒。
    NoticeCtxLockRecord,
}

impl WatchdogText {
    /// 全部文案，供覆盖测试遍历。
    #[cfg(test)]
    pub(crate) const ALL: [Self; 14] = [
        Self::StartedLockReset,
        Self::LastHandshakeLockClear,
        Self::LastHandshakeLockRecord,
        Self::LastHandshakeLockRead,
        Self::UnhealthySinceLockClear,
        Self::UnhealthySinceLockRecord,
        Self::UnhealthySinceLockRead,
        Self::StartedLockRead,
        Self::OfflineSinceLockRecord,
        Self::OfflineSinceLockRead,
        Self::PendingDisconnectReasonLockRecord,
        Self::PendingDisconnectReasonLockRead,
        Self::NoticeCtxLockSendReport,
        Self::NoticeCtxLockRecord,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text;

    /// 每条看门狗内部不变量文案在两种语言下都必须存在。
    #[test]
    fn every_watchdog_text_is_present_in_every_language() {
        for language in text::all_languages() {
            for message in WatchdogText::ALL {
                assert!(
                    !language.watchdog_text(message).is_empty(),
                    "missing watchdog text: {message:?}"
                );
            }
        }
    }

    /// 看门狗文案确实随语言切换，而不是两种语言共用一套字面量。
    #[test]
    fn watchdog_texts_are_localized() {
        let languages = text::all_languages();
        let (zh_cn, en_us) = (languages[0], languages[1]);
        for message in WatchdogText::ALL {
            assert_ne!(
                zh_cn.watchdog_text(message),
                en_us.watchdog_text(message),
                "watchdog text was not localized: {message:?}"
            );
        }
    }
}
