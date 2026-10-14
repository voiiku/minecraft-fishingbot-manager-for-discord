//! 权限判定：用户能不能执行某个指令，全项目只从这里出结论。
//! 每条指令都必须在路由层明确声明自己的要求（[`RequiredPermission`]）
//! 全部权限判定一律走 [`has_permission`]
//! 管理员名单唯一：`minecraft-fishingbot-manager-for-discord.conf.toml` 的 `[admins]`（Discord 用户 ID = 名字，见 [`config::admins`]）。

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredPermission {
    /// 只有管理员允许使用。
    MustBeAdmin,
    /// 这个归属（Discord 用户 ID）名下的机器人：本人或管理员。
    OwnerOrAdmin(u64),
    /// 任何人，如 /show botconfig，/show allbots 等
    Everyone,
}

/// 检查用户是否满足指令的权限要求；
/// 每条指令都必须在路由层（`lib.rs` 的 `handle_*`）判断一次，
/// 底层被调用的函数一律不再重复校验权限。
/// [`RequiredPermission::OwnerOrAdmin`] 用于单个机器人控制与管理的指令
/// 路由层先用`super::bot_dirs::BotDirEntry::find_by_name` 把目标条目解析出来，再拿它的归属判断。
/// [`RequiredPermission::MustBeAdmin`] 用于仅限管理员的指令：[`crate::handle_all_bots_control`] 判一次。
pub fn has_permission(caller_id: u64, required: RequiredPermission) -> bool {
    match required {
        RequiredPermission::Everyone => true,
        // 管理员名单唯一，所以先放行管理员。
        _ if config::admins().is_admin(caller_id) => true,
        // 执行到这一步说明不是管理员，不满足MustBeAdmin。
        RequiredPermission::MustBeAdmin => false,
        // 随后检查归属 Discord 用户 ID 是否匹配（是否是本人）。
        RequiredPermission::OwnerOrAdmin(owner_id) => owner_id == caller_id,
    }
}
