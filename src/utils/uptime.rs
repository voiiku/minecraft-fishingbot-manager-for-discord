//! 运行时长：读出「一个进程已经跑了多久」，并把它格式化成「天-时-分」。
//!
//! * 读取（[`process_elapsed_seconds`]）是 Linux 专有信息（本项目只面向 Linux 部署）：
//!   `/proc/<pid>/stat` 给出进程启动时刻相对系统开机时刻的滴答数，再结合 `/proc/stat` 里的
//!   btime 才能换算成"已运行秒数"；
//! * 格式化（[`format_uptime`]）供机器人运行时长与服务器运行时长共用（后者由 `sysinfo` 给出秒数）。

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// 根据 PID 计算进程已运行秒数；读不到 `/proc` 信息时返回 `None`。
pub fn process_elapsed_seconds(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;

    // /proc/<pid>/stat 的第 2 个字段 comm 可能带空格和括号，
    // 先定位到最后一个 ')'，从第 3 个字段开始数；starttime 是第 22 个字段，
    // 所以相对 after_comm 的下标是 22 - 3 = 19。
    let after_comm = stat.rsplit(')').next()?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let starttime: u64 = fields.get(19)?.parse().ok()?;

    // Linux 的 USER_HZ 固定是 100，starttime 要除以 100 才是秒。
    let start_epoch = system_boot_time()?.checked_add(starttime / 100)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    Some(now.saturating_sub(start_epoch))
}

/// 读取系统启动时间 btime（Unix 纪元秒数）。
fn system_boot_time() -> Option<u64> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    stat.lines().find_map(|line| {
        line.strip_prefix("btime ")
            .and_then(|rest| rest.trim().parse().ok())
    })
}

/// 把秒数格式化成「天-时-分」，例如 90_000 秒 → `1天-1时-0分`。
/// 机器人运行时长与服务器运行时长统一使用此格式。
pub fn format_uptime(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    format!("{days}天-{hours}时-{minutes}分")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 天/时/分的进位边界。
    #[test]
    fn formats_days_hours_minutes() {
        assert_eq!(format_uptime(86_400 + 3_600 + 5 * 60), "1天-1时-5分");
    }

    /// 不足一分钟时全是 0，秒不进位到分钟。
    #[test]
    fn drops_seconds() {
        assert_eq!(format_uptime(59), "0天-0时-0分");
        assert_eq!(format_uptime(61), "0天-0时-1分");
    }

    /// `btime` 是稳定的系统事实：能读出来就说明 `/proc/stat` 的解析没问题。
    #[cfg(target_os = "linux")]
    #[test]
    fn boot_time_is_readable_on_linux() {
        assert!(system_boot_time().is_some());
    }

    /// 当前进程自己一定在跑，所以运行时长必须算得出来。
    ///
    /// 只断言「算得出来」：测试进程刚启动，整数秒会被截断成 0，
    /// 断言「大于 0」就成了一条看机器快慢、随时可能变红的脆弱测试。
    #[cfg(target_os = "linux")]
    #[test]
    fn current_process_has_elapsed_seconds() {
        assert!(process_elapsed_seconds(std::process::id()).is_some());
    }
}
