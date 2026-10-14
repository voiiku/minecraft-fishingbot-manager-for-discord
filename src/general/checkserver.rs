//! `/checkserver`：查询钓鱼服务器自身的资源占用。

use std::time::Duration;

use sysinfo::{Disks, System};

use crate::text;
use crate::utils::{Report, format_uptime};

/// 返回本机 CPU、内存、磁盘与运行时长。
pub(crate) async fn checkserver_report() -> Result<String, String> {
    let mut sys = System::new_all();

    // 两次采样计算 CPU 使用率。
    sys.refresh_cpu_all();
    tokio::time::sleep(Duration::from_millis(200)).await;
    sys.refresh_cpu_all();

    let cpu_usage = sys.global_cpu_usage();

    sys.refresh_memory();
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let available_mem = sys.available_memory();

    let mem_usage_pct = if total_mem > 0 {
        (used_mem as f64 / total_mem as f64) * 100.0
    } else {
        0.0
    };
    let available_mem_mb = available_mem / (1024 * 1024);

    // 汇总磁盘容量。
    let disks = Disks::new_with_refreshed_list();
    let total_disk: u64 = disks.iter().map(|d| d.total_space()).sum();
    let available_disk: u64 = disks.iter().map(|d| d.available_space()).sum();
    let used_disk = total_disk.saturating_sub(available_disk);

    let disk_usage_pct = if total_disk > 0 {
        (used_disk as f64 / total_disk as f64) * 100.0
    } else {
        0.0
    };

    let i18n_text = text::get();
    let mut report = Report::new();
    report.set_title(i18n_text.checkserver_title());
    report
        .basics
        .push(i18n_text.checkserver_cpu_usage(cpu_usage));
    report
        .basics
        .push(i18n_text.checkserver_memory_usage(mem_usage_pct));
    report
        .basics
        .push(i18n_text.checkserver_available_memory(available_mem_mb));
    report
        .basics
        .push(i18n_text.checkserver_disk_usage(disk_usage_pct));
    report
        .basics
        .push(i18n_text.checkserver_uptime(&format_uptime(System::uptime())));

    Ok(report.render())
}
