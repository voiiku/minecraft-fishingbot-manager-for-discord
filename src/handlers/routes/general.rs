//! 通用指令路由。

use super::permission_refusal;
use crate::handlers::{checkserver, help, ping, whoami};
use crate::utils::{IntoReplyText, RequiredPermission, has_permission};

/// 返回 `/help` 帮助文本。
pub fn handle_help(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    help()
}

/// 返回调用者的 Discord 用户 ID。
pub fn handle_whoami(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    whoami(caller_id)
}

/// 校验手动同步 Discord 指令的管理员权限。
pub fn check_sync_commands_permission(caller_id: u64) -> Result<(), String> {
    if !has_permission(caller_id, RequiredPermission::MustBeAdmin) {
        return Err(permission_refusal(caller_id));
    }
    Ok(())
}

/// 探测游戏服务器。
pub async fn handle_ping(caller_id: u64, host: Option<&str>, port: Option<u16>) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    ping(host, port).await.to_reply_text()
}

/// 返回钓鱼服务器的资源信息。
pub async fn handle_checkserver(caller_id: u64) -> String {
    if !has_permission(caller_id, RequiredPermission::Everyone) {
        return permission_refusal(caller_id);
    }

    checkserver().await.to_reply_text()
}
