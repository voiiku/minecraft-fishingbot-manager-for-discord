//! `/whoami`：返回调用者自己的 Discord 用户 ID。

use crate::text;

/// 返回调用者的 Discord 用户 ID。
pub(crate) fn whoami(caller_id: u64) -> String {
    text::get().command_whoami(caller_id)
}
