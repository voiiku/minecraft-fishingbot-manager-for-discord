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
    // 尝试打开文件
    let mut file = File::open(file_path).map_err(|e| {
        text::get().log_text(text::LogText::OpenLogFileFailed {
            reason: &e.to_string(),
        })
    })?;

    // 文件长度就是文件末尾的位置
    let file_len = file
        .metadata()
        .map_err(|e| {
            text::get().log_text(text::LogText::ReadLogMetadataFailed {
                reason: &e.to_string(),
            })
        })?
        .len();
    // 开始向前扫描的起点位置，会逐块向前推进。
    let mut position = file_len;
    // 已扫到的行（由新到旧）的收集器，最后会被反转成文件顺序返回。
    let mut scanned_lines: Vec<String> = Vec::new();
    // 预定义要用的闭包函数
    // 1. 获取一块字节：position - read_size = 下次要读取字节块的起点。
    let mut read_previous_chunk = |position: &mut u64| -> Result<Vec<u8>, String> {
        // 取实际最小的读取范围值，因为文件可能小于 BACKWARD_READ_BLOCK_BYTES。
        let read_size = std::cmp::min(BACKWARD_READ_BLOCK_BYTES as u64, *position) as usize;
        // 一次读取的起点（position创建时对应文件末尾，此时减去read_size就表示对应要读取字节块的起点）
        *position -= read_size as u64;

        file.seek(SeekFrom::Start(*position)).map_err(|e| {
            text::get().log_text(text::LogText::SeekLogFileFailed {
                reason: &e.to_string(),
            })
        })?;
        // 临时存储字节块数据的缓冲区，大小为 read_size
        let mut chunk = vec![0u8; read_size];
        file.read_exact(&mut chunk).map_err(|e| {
            text::get().log_text(text::LogText::ReadLogContentFailed {
                reason: &e.to_string(),
            })
        })?;

        Ok(chunk)
    };

    // 2. 攒行器：获取一块字节（由新到旧）以及"是否已经到文件开头了"的判断，
    // 它返回这一块里全部的完整行（由新到旧）；第二个返回值是判断是否该结束整个扫描。
    // 一行可能横跨两块（读取是按字节块的），"正在攒的那一行"只能由它自己保管。
    let mut take_lines = {
        // 需要积攒的行收集器，按倒序扫描每一行（遇到\n就认为这一行已经收集完毕）。
        let mut reversed_line_bytes: Vec<u8> = Vec::new();

        move |chunk: &[u8], at_file_start: bool| -> (Vec<String>, bool) {
            // 这一块攒出的行（由新到旧）的收集数组。
            let mut lines: Vec<String> = Vec::new();
            // 定义要用的单行攒行器
            // 收集正在攒的这一行：翻正 → 解码 → 去掉 CRLF 的 `\r` → 问 `when_to_stop` 要不要停。
            // 返回true就结束扫描（本行也收集）。
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

            // 预定义结束状态标志bool
            let mut should_stop = false;
            // 一块字节出现 `\n` 就说明"现在正在攒的那一行"完毕了
            for &byte in chunk.iter().rev() {
                if byte == b'\n' {
                    if take_line(&mut reversed_line_bytes) {
                        // 判断为true则停止，并把这一行收集到 lines 里（take_line 内部实现）。
                        should_stop = true;
                        break;
                    }
                } else {
                    reversed_line_bytes.push(byte);
                }
            }

            // 若已经扫到文件开头（开头那一行前面没有 `\n`），存入lines。
            if at_file_start && !should_stop {
                take_line(&mut reversed_line_bytes);
            }

            (lines, should_stop)
        }
    };

    // 第一步：若空文件直接返回空数组
    if file_len == 0 {
        return Ok(Vec::new());
    }

    // 第二步：文件末尾里的最后一个字节如果是 `\n`，去掉后，就认为"`\n` = 上一行的结束标志"。
    let mut chunk = read_previous_chunk(&mut position)?;
    if chunk.last() == Some(&b'\n') {
        // 丢弃最后一个字节\n，但是保留这个块，因为之后还要用
        chunk.pop();
    }

    // 第三步：按字节块循环往前读取并倒序存储在scanned_lines数组中，直到走到文件开头。
    'scan: loop {
        // 把这一块交给攒行器：它产出该块内可攒出的行（由新到旧），并返回判定是否命中。
        // `position == 0` 表示这一块已经到文件开头 —— 那时它会连"开头那一行"（可能为空行）一起保存。
        let (lines, should_stop) = take_lines(&chunk, position == 0);
        scanned_lines.extend(lines);

        // 第四步：判定true或已经扫描到文件开头就结束。
        if should_stop || position == 0 {
            break 'scan;
        }

        // 第五步：若一次扫描还没到文件开头，再向前取一块字节，回到第三步重复。
        chunk = read_previous_chunk(&mut position)?;
    }

    // 上面是"由新到旧"存储的，翻转成文件顺序（由旧到新）返回。
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

    /// 每个测试用一个独立的临时目录（带进程号，避免并发跑测试时互相踩）。
    use crate::test_support::temp_test_dir;

    /// 按行往回读：返回的顺序是**文件顺序**（旧→新），末尾换行不算空行。
    #[test]
    fn backward_reader_returns_lines_in_file_order() {
        let dir = temp_test_dir("read-backwards");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("2026-09-28-01-47-01.log");
        fs::write(&log_path, "一\n二\n三\n").expect("写日志");

        let all_lines = read_lines_backwards_until(&log_path, |_| false).expect("读日志");
        assert_eq!(all_lines, vec!["一", "二", "三"]);

        // 判定命中就停：只往回扫到"三"为止，更旧的两行不必读。
        let first_line =
            read_lines_backwards_until(&log_path, |line| line == "三").expect("读日志");
        assert_eq!(first_line, vec!["三"]);

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 边界：末尾没有换行的最后一行要读出来；空文件返回空（调用方据此区分"空日志"与"没有日志"）；
    /// 只有一个换行的文件是一行空行（与 `str::lines()` 的口径一致）。
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

    /// 空行（开头、中间、连续多个）要和 `str::lines()` 的口径一致 —— 往回读只是个实现手段，
    /// 不该在这里偏一寸：整条规则只允许有"文件末尾那个换行不算空行"这一个特例。
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

    /// 行数为 0 直接报错，不静默按 1 行处理（`/log` 会把这句话原样回给用户）。
    #[test]
    fn read_last_lines_rejects_zero_lines() {
        let dir = temp_test_dir("read-zero-lines");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("2026-09-28-01-47-01.log");
        fs::write(&log_path, "一行\n").expect("写日志");

        let message = read_last_lines(&log_path, 0).expect_err("0 行应当报错");
        // 这句话取自语言表（`/log` 会原样回给用户）：期望值也从那里取，不写死中文或英文。
        assert_eq!(message, text::get().bot_log_zero_lines());

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// CRLF、跨读块、多字节字符：`\r` 去掉；行不会被读块边界切开（每行末尾的 emoji 都完整）；
    /// `read_last_lines` 只取尾部 N 行。
    #[test]
    fn backward_reader_handles_crlf_and_crosses_read_blocks() {
        let dir = temp_test_dir("read-backwards-blocks");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("big.log");

        // 每行都带 CRLF，且总长度明显超过一个读块（`BACKWARD_READ_BLOCK_BYTES` = 16 KiB）。
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

        // 往回读全文：行数要对得上（跨块拼接既不丢行也不多行）。
        let all_lines = read_lines_backwards_until(&log_path, |_| false).expect("读全文");
        assert_eq!(all_lines.len(), 2000);

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    /// 子目录名列表：只要目录，普通文件跳过（顺序由文件系统给，所以这里按集合比较）。
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

    /// 列子目录失败要报错（调用方据此决定是回复用户还是记日志）；删目录则把"不存在"当成成功。
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
