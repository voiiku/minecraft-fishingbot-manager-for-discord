//! 看门狗互斥锁中毒时的诊断文案。
//!
//! 文案随 `[i18n] language` 选择，由 [`crate::text::Text::watchdog_text`] 返回静态字符串。
//! 该接口不分配内存，适用于锁操作路径。

/// 看门狗互斥锁中毒的诊断消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogText {
    /// 重置连接尝试起始时刻时，`started` 互斥锁中毒。
    StartedLockReset,
    /// 清除上次握手状态时，`last_handshake` 互斥锁中毒。
    LastHandshakeLockClear,
    /// 记录握手时刻时，`last_handshake` 互斥锁中毒。
    LastHandshakeLockRecord,
    /// 读取握手状态时，`last_handshake` 互斥锁中毒。
    LastHandshakeLockRead,
    /// 清除探活失败起始时刻时，`unhealthy_since` 互斥锁中毒。
    UnhealthySinceLockClear,
    /// 记录探活失败起始时刻时，`unhealthy_since` 互斥锁中毒。
    UnhealthySinceLockRecord,
    /// 读取探活失败起始时刻时，`unhealthy_since` 互斥锁中毒。
    UnhealthySinceLockRead,
    /// 读取连接尝试起始时刻时，`started` 互斥锁中毒。
    StartedLockRead,
    /// 记录掉线原因与时刻时，`offline_since` 互斥锁中毒。
    OfflineSinceLockRecord,
    /// 读取掉线记录时，`offline_since` 互斥锁中毒。
    OfflineSinceLockRead,
    /// 记录待处理掉线原因时，`pending_disconnect_reason` 互斥锁中毒。
    PendingDisconnectReasonLockRecord,
    /// 读取待处理掉线原因时，`pending_disconnect_reason` 互斥锁中毒。
    PendingDisconnectReasonLockRead,
    /// 读取通知上下文以发送收尾报告时，`notice_ctx` 互斥锁中毒。
    NoticeCtxLockSendReport,
    /// 记录 Discord 通知上下文时，`notice_ctx` 互斥锁中毒。
    NoticeCtxLockRecord,
}

impl WatchdogText {
    /// 全部变体，供覆盖测试遍历。
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

    /// 每种语言都必须为全部 `WatchdogText` 变体提供非空文案。
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

    /// 同一 `WatchdogText` 变体在中文与英文实现中的文案必须不同。
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
