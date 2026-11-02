//! 指令权限要求。

/// 指令所需的权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredPermission {
    /// 仅管理员。
    MustBeAdmin,
    /// 机器人所有者或管理员。
    OwnerOrAdmin(u64),
    /// 任何用户。
    Everyone,
}
