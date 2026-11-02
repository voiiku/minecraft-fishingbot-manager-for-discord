//! 读取 Linux 进程运行时长，并格式化为天、时、分。

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// 根据 PID 计算进程已运行秒数；读不到 `/proc` 信息时返回 `None`。
pub fn process_elapsed_seconds(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;

    // comm 可含空格和括号；最后一个 ')' 之后索引 19（从零开始）为 starttime。
    let after_comm = stat.rsplit(')').next()?;
    let starttime: u64 = after_comm.split_whitespace().nth(19)?.parse().ok()?;

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

    /// Linux 的 `/proc/stat` 必须能提供启动时间。
    #[cfg(target_os = "linux")]
    #[test]
    fn boot_time_is_readable_on_linux() {
        assert!(system_boot_time().is_some());
    }
}
