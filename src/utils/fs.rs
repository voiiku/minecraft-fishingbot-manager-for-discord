//! 通用的处理文件和路径的函数都在这里，如何处理是调用方的逻辑。
//! 具体功能专用的文件查询/筛选由各自模块内部定义，不在此处

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::text;

/// 从文件末尾往回读时每次读多少字节。
const BACKWARD_READ_BLOCK_BYTES: usize = 16 * 1024;

/// 从文件末尾往回逐行读，由 `when_to_stop` 判断何时停止（包括判断成功那一行）；
/// 返回扫到的行（文件顺序：由旧到新）。
/// 例如，状态判定只关心最新的关键词（[`BotDirEntry::connection_status`]）。
/// 文件为空时返回空数组，调用方据此区分"文件不存在"与"文件是空的"。
pub fn read_lines_backwards_until(
    file_path: &Path,
    mut when_to_stop: impl FnMut(&str) -> bool,
) -> Result<Vec<String>, String> {
    let mut file = File::open(file_path).map_err(|e| {
        text::get().log_text(text::LogText::OpenLogFileFailed {
            reason: &e.to_string(),
        })
    })?;

    let file_len = file
        .metadata()
        .map_err(|e| {
            text::get().log_text(text::LogText::ReadLogMetadataFailed {
                reason: &e.to_string(),
            })
        })?
        .len();
    let mut position = file_len;
    let mut scanned_lines: Vec<String> = Vec::new();
    let mut read_previous_chunk = |position: &mut u64| -> Result<Vec<u8>, String> {
        let read_size = std::cmp::min(BACKWARD_READ_BLOCK_BYTES as u64, *position) as usize;
        *position -= read_size as u64;

        file.seek(SeekFrom::Start(*position)).map_err(|e| {
            text::get().log_text(text::LogText::SeekLogFileFailed {
                reason: &e.to_string(),
            })
        })?;
        let mut chunk = vec![0u8; read_size];
        file.read_exact(&mut chunk).map_err(|e| {
            text::get().log_text(text::LogText::ReadLogContentFailed {
                reason: &e.to_string(),
            })
        })?;

        Ok(chunk)
    };

    // 倒序读取时，一行可能跨越多个字节块。
    let mut take_lines = {
        let mut reversed_line_bytes: Vec<u8> = Vec::new();

        move |chunk: &[u8], at_file_start: bool| -> (Vec<String>, bool) {
            let mut lines: Vec<String> = Vec::new();
            let mut take_line = |reversed_line_bytes: &mut Vec<u8>| -> bool {
                reversed_line_bytes.reverse();
                let mut line = String::from_utf8_lossy(reversed_line_bytes).into_owned();
                reversed_line_bytes.clear();
                if line.ends_with('\r') {
                    line.pop();
                }

                let should_stop = when_to_stop(&line);
                lines.push(line);
                should_stop
            };

            let mut should_stop = false;
            for &byte in chunk.iter().rev() {
                if byte == b'\n' {
                    if take_line(&mut reversed_line_bytes) {
                        should_stop = true;
                        break;
                    }
                } else {
                    reversed_line_bytes.push(byte);
                }
            }

            if at_file_start && !should_stop {
                take_line(&mut reversed_line_bytes);
            }

            (lines, should_stop)
        }
    };

    if file_len == 0 {
        return Ok(Vec::new());
    }

    // 末尾换行是上一行的结束符，不是额外的空行。
    let mut chunk = read_previous_chunk(&mut position)?;
    if chunk.last() == Some(&b'\n') {
        chunk.pop();
    }

    loop {
        let (lines, should_stop) = take_lines(&chunk, position == 0);
        scanned_lines.extend(lines);

        if should_stop || position == 0 {
            break;
        }

        chunk = read_previous_chunk(&mut position)?;
    }

    scanned_lines.reverse();
    Ok(scanned_lines)
}

/// 读取文件的最后 `max_lines` 行。
/// `max_lines` 为 0 直接报错返回给调用者。
pub fn read_last_lines(file_path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    if max_lines == 0 {
        return Err(text::get().bot_log_zero_lines().to_string());
    }

    let mut counted_lines = 0usize;
    read_lines_backwards_until(file_path, |_| {
        counted_lines += 1;
        counted_lines >= max_lines
    })
}

/// 列出 `dir` 下可读取的子目录名字（只要名字的 UTF-8 文本），跳过单条读取错误，顺序由系统决定。
pub fn list_subdirectory_names(dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut subdirectory_names = Vec::new();
    for dir_entry in fs::read_dir(dir)?.flatten() {
        if !dir_entry.path().is_dir() {
            continue;
        }
        if let Some(dir_name) = dir_entry.file_name().to_str() {
            subdirectory_names.push(dir_name.to_string());
        }
    }

    Ok(subdirectory_names)
}

/// 删除目录，目录本来不存在时也算成功。
/// 调用方关心的是"保证目录不存在"这个结果，调用方不必再校验目录的存在性。
/// （`/bot del` 删数据目录、拉起会话前清 `raw/`）
pub fn remove_dir_all_if_exists(dir: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(dir) {
        // 目录本来就不在 = 达成执行目标。
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试使用独立临时目录，避免并发测试互相干扰。
    use crate::test_support::temp_test_dir;

    /// 从文件末尾读取时，返回结果按文件顺序排列；末尾换行不产生空行。
    #[test]
    fn backward_reader_returns_lines_in_file_order() {
        let dir = temp_test_dir("read-backwards");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("2026-09-28-01-47-01.log");
        fs::write(&log_path, "一\n二\n三\n").expect("写日志");

        let all_lines = read_lines_backwards_until(&log_path, |_| false).expect("读日志");
        assert_eq!(all_lines, vec!["一", "二", "三"]);

        // 遇到匹配行后停止扫描，更旧的行不再读取。
        let first_line =
            read_lines_backwards_until(&log_path, |line| line == "三").expect("读日志");
        assert_eq!(first_line, vec!["三"]);

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 末尾无换行时仍返回最后一行；空文件返回空集合；仅含一个换行的文件返回一个空行。
    #[test]
    fn backward_reader_handles_missing_trailing_newline_and_empty_file() {
        let dir = temp_test_dir("read-backwards-edge");
        fs::create_dir_all(&dir).expect("建临时目录");

        let without_newline = dir.join("no-newline.log");
        fs::write(&without_newline, "只有一行").expect("写日志");
        assert_eq!(
            read_lines_backwards_until(&without_newline, |_| false).expect("读日志"),
            vec!["只有一行"]
        );

        let empty = dir.join("empty.log");
        fs::write(&empty, "").expect("写空日志");
        assert!(
            read_lines_backwards_until(&empty, |_| false)
                .expect("读空日志")
                .is_empty()
        );

        let only_newline = dir.join("only-newline.log");
        fs::write(&only_newline, "\n").expect("写日志");
        assert_eq!(
            read_lines_backwards_until(&only_newline, |_| false).expect("读日志"),
            vec![""]
        );

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 空行处理与 `str::lines()` 一致；唯一特例是文件末尾的换行不计为额外空行。
    #[test]
    fn backward_reader_matches_str_lines_on_empty_lines() {
        let dir = temp_test_dir("read-backwards-empty-lines");
        fs::create_dir_all(&dir).expect("建临时目录");

        let payloads = [
            "\n",
            "\n\n",
            "\n一\n",
            "一\n\n二\n",
            "一\n二\n\n",
            "\n\n一\n\n",
            "没有换行的最后一行",
        ];

        for (index, payload) in payloads.iter().enumerate() {
            let log_path = dir.join(format!("case-{index}.log"));
            fs::write(&log_path, payload).expect("写日志");

            let read_lines = read_lines_backwards_until(&log_path, |_| false).expect("读日志");
            assert_eq!(
                read_lines,
                payload.lines().collect::<Vec<&str>>(),
                "{payload:?} 的行应当与 `str::lines()` 一致"
            );
        }

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 处理 CRLF、跨读块与多字节字符；`read_last_lines` 只返回尾部 N 行。
    #[test]
    fn backward_reader_handles_crlf_and_crosses_read_blocks() {
        let dir = temp_test_dir("read-backwards-blocks");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("big.log");

        // 总长度明显超过单个读块（`BACKWARD_READ_BLOCK_BYTES` = 16 KiB）。
        let mut content = String::new();
        for index in 0..2000 {
            content.push_str(&format!("第{index}行-钓鱼中🐟\r\n"));
        }
        fs::write(&log_path, content).expect("写大日志");

        let last_three_lines = read_last_lines(&log_path, 3).expect("读最后 3 行");
        assert_eq!(
            last_three_lines,
            vec![
                "第1997行-钓鱼中🐟",
                "第1998行-钓鱼中🐟",
                "第1999行-钓鱼中🐟"
            ]
        );

        // 全文读取用于验证跨块拼接既不丢行，也不重复行。
        let all_lines = read_lines_backwards_until(&log_path, |_| false).expect("读全文");
        assert_eq!(all_lines.len(), 2000);

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 子目录名列表只包含目录，普通文件被跳过；比较前排序以消除文件系统顺序差异。
    #[test]
    fn subdirectory_names_skip_plain_files() {
        let dir = temp_test_dir("list-subdirs");
        fs::create_dir_all(dir.join("一段")).expect("建子目录");
        fs::create_dir_all(dir.join("二段")).expect("建子目录");
        fs::write(dir.join("一份日志.log"), "").expect("建普通文件");

        let mut names = list_subdirectory_names(&dir).expect("列子目录");
        names.sort();
        assert_eq!(names, vec!["一段".to_string(), "二段".to_string()]);

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 列出不存在的目录会报错；删除不存在的目录则视为成功。
    #[test]
    fn missing_dir_is_error_when_listing_and_success_when_removing() {
        let dir = temp_test_dir("missing-dir");
        assert!(list_subdirectory_names(&dir).is_err());
        remove_dir_all_if_exists(&dir).expect("删不存在的目录也算成功");

        fs::create_dir_all(&dir).expect("建目录");
        fs::write(dir.join("里面.log"), "").expect("建文件");
        remove_dir_all_if_exists(&dir).expect("删掉真的目录");
        assert!(!dir.exists());
    }
}
