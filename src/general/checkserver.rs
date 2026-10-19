//! `/checkserver`：查询钓鱼服务器自身的资源占用。

use std::time::Duration;

use chrono::Local;
use sysinfo::{Disks, System};

use crate::text;
use crate::utils::{Report, format_uptime, process_elapsed_seconds};

/// 采集本机 CPU、内存、磁盘、运行时长与系统时间，并返回格式化报告。
pub(crate) async fn checkserver() -> Result<String, String> {
    // 仅需要 CPU 与内存数据，避免加载进程列表。
    let mut sys = System::new();

    // 两次采样计算 CPU 使用率。
    sys.refresh_cpu_all();
    tokio::time::sleep(Duration::from_millis(200)).await;
    sys.refresh_cpu_all();

    let cpu_usage = sys.global_cpu_usage();

    sys.refresh_memory();
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let total_mem_mb = total_mem / (1024 * 1024);
    let used_mem_mb = used_mem / (1024 * 1024);
    let mem_usage_pct = if total_mem > 0 {
        (used_mem as f64 / total_mem as f64) * 100.0
    } else {
        0.0
    };

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

    let system_time = Local::now()
        .format("%Y-%m-%d %H:%M:%S %Z (UTC%:z)")
        .to_string();

    let i18n_text = text::get();
    let mut report = Report::new();
    report.set_title(i18n_text.checkserver_title());
    report
        .basics
        .push(i18n_text.checkserver_cpu_usage(cpu_usage));
    report
        .basics
        .push(i18n_text.checkserver_memory_used_total(used_mem_mb, total_mem_mb));
    report
        .basics
        .push(i18n_text.checkserver_memory_usage(mem_usage_pct));
    report
        .basics
        .push(i18n_text.checkserver_disk_usage(disk_usage_pct));
    report
        .basics
        .push(i18n_text.checkserver_uptime(&format_uptime(System::uptime())));
    if let Some(process_elapsed_secs) = process_elapsed_seconds(std::process::id()) {
        report
            .basics
            .push(i18n_text.checkserver_process_uptime(&format_uptime(process_elapsed_secs)));
    }
    report
        .basics
        .push(i18n_text.checkserver_system_time(&system_time));

    Ok(report.render())
}
