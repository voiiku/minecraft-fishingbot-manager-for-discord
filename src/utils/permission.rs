//! 权限判定。
//! 每条指令都必须在路由层明确声明自己的要求（[`RequiredPermission`]）
//! 全部权限判定一律走 [`has_permission`]
//! 配置文件中的 `[admins]`（Discord 用户 ID = 名字，见 [`config::Config::admins`]）。

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredPermission {
    /// 只有管理员允许使用。
    MustBeAdmin,
    /// 本人或管理员。
    OwnerOrAdmin(u64),
    /// 任何人。
    Everyone,
}

/// 检查用户是否满足指令的权限要求
/// 每条指令都必须在路由层（`lib.rs` 的 `handle_*`）判断一次，底层函数一律不再重复校验。
pub fn has_permission(caller_id: u64, required: RequiredPermission) -> bool {
    match required {
        RequiredPermission::Everyone => true,
        // 管理员名单唯一，所以先放行管理员。
        _ if config::Config::admins().is_admin(caller_id) => true,
        RequiredPermission::MustBeAdmin => false,
        // 随后检查归属 Discord 用户 ID 是否匹配（是否是本人）。
        RequiredPermission::OwnerOrAdmin(owner_id) => owner_id == caller_id,
    }
}
