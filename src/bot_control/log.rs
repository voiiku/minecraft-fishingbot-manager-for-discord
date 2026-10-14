//! `/log`：读取最近一次运行的过滤后日志，返回文件名与行内容供命令层作为附件发送。

use crate::config;
use crate::text;
use crate::utils::{BotDirEntry, MAX_ATTACHMENT_BYTES, latest_log_path, read_last_lines};

/// `/log` 附件数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotLogView {
    /// 日志文件名（附件的文件名就是它）。
    pub file_name: String,
    /// 最后 N 行，已按附件大小裁剪。
    pub lines: Vec<String>,
    /// 被裁掉的最早行数。
    pub dropped_line_count: usize,
}

/// 读取最新日志的最后 N 行；权限由路由层检查。
pub(crate) fn bot_log_view(
    bot_dir_entry: &BotDirEntry,
    line_count: usize,
) -> Result<BotLogView, String> {
    let bot_name: &str = &bot_dir_entry.bot_name;

    if line_count == 0 {
        return Err(text::get().bot_log_zero_lines().to_string());
    }
    // 限制读取行数。
    let target_line_count = line_count.min(config::Config::get().runtime.max_log_lines);

    // 只读最新日志。
    let Some(target_log_path) = latest_log_path(&bot_dir_entry.bot_dir_path) else {
        return Err(text::get().bot_log_missing(bot_name));
    };

    // 只保留最后 N 行。
    let log_lines = read_last_lines(&target_log_path, target_line_count)?;

    // 附件名使用日志文件名。
    let Some(file_name) = target_log_path.file_name() else {
        return Err(text::get().bot_log_missing(bot_name));
    };
    let file_name = file_name.to_string_lossy().into_owned();

    Ok(cap_attachment_size(
        file_name,
        log_lines,
        MAX_ATTACHMENT_BYTES,
    ))
}

/// 按附件大小上限从最早处裁剪日志行。
fn cap_attachment_size(file_name: String, lines: Vec<String>, max_bytes: usize) -> BotLogView {
    let total_bytes: usize = lines.iter().map(|line| line.len() + 1).sum();
    if total_bytes <= max_bytes {
        return BotLogView {
            file_name,
            lines,
            dropped_line_count: 0,
        };
    }

    let mut kept_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut kept_bytes = 0usize;
    for line in lines.iter().rev() {
        let line_bytes = line.len() + 1;
        if kept_bytes + line_bytes > max_bytes {
            break;
        }
        kept_bytes += line_bytes;
        kept_lines.push(line.clone());
    }
    kept_lines.reverse();

    let dropped_line_count = lines.len() - kept_lines.len();
    BotLogView {
        file_name,
        lines: kept_lines,
        dropped_line_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 未超上限时保留全部行。
    #[test]
    fn small_log_is_kept_as_is() {
        let lines = vec!["第一行".to_string(), "第二行".to_string()];

        let log_view =
            cap_attachment_size("2026-01-01-00-00-00.log".to_string(), lines.clone(), 1024);

        assert_eq!(log_view.file_name, "2026-01-01-00-00-00.log");
        assert_eq!(log_view.lines, lines);
        assert_eq!(log_view.dropped_line_count, 0);
    }

    /// 超上限时从最早的行开始裁剪。
    #[test]
    fn oversized_log_drops_the_oldest_lines() {
        let lines = vec!["1111".to_string(), "2222".to_string(), "3333".to_string()];

        let log_view = cap_attachment_size("big.log".to_string(), lines, 11);

        assert_eq!(log_view.lines, vec!["2222".to_string(), "3333".to_string()]);
        assert_eq!(log_view.dropped_line_count, 1);
    }

    /// 一行也放不下时返回空内容并记录省略行数。
    #[test]
    fn nothing_fits_reports_every_line_as_dropped() {
        let lines = vec!["1111".to_string(), "2222".to_string()];

        let log_view = cap_attachment_size("big.log".to_string(), lines, 3);

        assert!(log_view.lines.is_empty());
        assert_eq!(log_view.dropped_line_count, 2);
    }
}
