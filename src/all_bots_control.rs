//! 全服批量指令。权限由路由层检查，流程复用 `utils`。

mod commandall;
mod restartall;
mod startall;
mod stopall;

// 本模块对外的暴露的命令与进程收尾入口。
pub(crate) use commandall::send_command_to_all_bots;
pub(crate) use restartall::restart_all_bots;
pub(crate) use startall::{planned_startall_notice, start_all_bots};
pub(crate) use stopall::stop_all_bots;
