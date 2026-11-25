//! 文件和目录操作，不处理本地化文案。

use chrono::Local;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) enum CreateLogError {
    CreateDir(PathBuf, io::Error),
    CreateFile(PathBuf, io::Error),
    WriteFirstLine(PathBuf, io::Error),
}

#[derive(Debug)]
pub(crate) enum ReadLogError {
    Open(io::Error),
    Metadata(io::Error),
    Truncated,
    Seek(io::Error),
    Read(io::Error),
}

/// 创建日志目录。
pub(crate) fn create_log_dir(dir: &Path) -> Result<(), CreateLogError> {
    fs::create_dir_all(dir).map_err(|error| CreateLogError::CreateDir(dir.to_path_buf(), error))
}

/// 在已创建的目录下创建时间戳日志文件并写入首行。
pub(crate) fn create_timestamped_log_file(
    dir: &Path,
    first_line: &str,
    format: &str,
) -> Result<(PathBuf, File), CreateLogError> {
    let log_path = dir.join(format!("{}.log", Local::now().format(format)));
    let mut file =
        File::create(&log_path).map_err(|e| CreateLogError::CreateFile(log_path.clone(), e))?;
    writeln!(file, "{first_line}")
        .map_err(|e| CreateLogError::WriteFirstLine(log_path.clone(), e))?;

    Ok((log_path, file))
}

/// 持有已打开的日志及已验证的起始偏移。
pub(crate) struct LogReader {
    file: File,
    start: u64,
    file_len: u64,
}

impl LogReader {
    pub(crate) fn open(file_path: &Path, start: u64) -> Result<Self, ReadLogError> {
        let file = File::open(file_path).map_err(ReadLogError::Open)?;
        let file_len = file.metadata().map_err(ReadLogError::Metadata)?.len();
        if start > file_len {
            return Err(ReadLogError::Truncated);
        }
        Ok(Self {
            file,
            start,
            file_len,
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.file_len == self.start
    }

    pub(crate) fn read_last_lines(
        self,
        max_lines: NonZeroUsize,
        block_bytes: NonZeroUsize,
    ) -> Result<Vec<String>, ReadLogError> {
        let mut counted_lines = 0usize;
        self.read_lines_backwards(block_bytes, |_| {
            counted_lines += 1;
            counted_lines >= max_lines.get()
        })
    }

    fn read_lines_backwards(
        self,
        block_bytes: NonZeroUsize,
        mut when_to_stop: impl FnMut(&str) -> bool,
    ) -> Result<Vec<String>, ReadLogError> {
        let Self {
            mut file,
            start,
            file_len,
        } = self;
        let mut position = file_len;
        let mut scanned_lines: Vec<String> = Vec::new();
        let mut read_previous_chunk = |position: &mut u64| -> Result<Vec<u8>, ReadLogError> {
            let read_size = std::cmp::min(block_bytes.get() as u64, *position - start) as usize;
            *position -= read_size as u64;

            file.seek(SeekFrom::Start(*position))
                .map_err(ReadLogError::Seek)?;
            let mut chunk = vec![0u8; read_size];
            file.read_exact(&mut chunk).map_err(ReadLogError::Read)?;

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

        if file_len == start {
            return Ok(Vec::new());
        }

        // 末尾换行是上一行的结束符，不是额外的空行。
        let mut chunk = read_previous_chunk(&mut position)?;
        if chunk.last() == Some(&b'\n') {
            chunk.pop();
        }

        loop {
            let (lines, should_stop) = take_lines(&chunk, position == start);
            scanned_lines.extend(lines);

            if should_stop || position == start {
                break;
            }

            chunk = read_previous_chunk(&mut position)?;
        }

        scanned_lines.reverse();
        Ok(scanned_lines)
    }
}

/// 列出 `dir` 下可读取的子目录名字（只要名字的 UTF-8 文本），跳过单条读取错误，顺序由系统决定。
pub(crate) fn list_subdirectory_names(dir: &Path) -> Result<Vec<String>, std::io::Error> {
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

/// 删除目录；目录不存在时视为成功。
pub fn remove_dir_all_if_exists(dir: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(name: &str) -> PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "minecraft-fishingbot-utils-{name}-{}-{timestamp}",
            std::process::id()
        ))
    }

    fn block_size() -> NonZeroUsize {
        NonZeroUsize::new(32).unwrap()
    }

    fn read_lines_backwards_from(
        file_path: &Path,
        start: u64,
        when_to_stop: impl FnMut(&str) -> bool,
    ) -> Result<Vec<String>, ReadLogError> {
        LogReader::open(file_path, start)?.read_lines_backwards(block_size(), when_to_stop)
    }

    fn read_last_lines_after(
        file_path: &Path,
        start: u64,
        max_lines: usize,
    ) -> Result<Vec<String>, ReadLogError> {
        LogReader::open(file_path, start)?
            .read_last_lines(NonZeroUsize::new(max_lines).unwrap(), block_size())
    }

    #[test]
    fn injected_read_sizes_preserve_multibyte_lines() {
        let dir = temp_test_dir("read-configurable-blocks");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bot.log");
        fs::write(&path, "first\r\n钓鱼🐟\r\nlast\n").unwrap();

        for size in [1, 2, 7] {
            let lines = LogReader::open(&path, 0)
                .unwrap()
                .read_last_lines(
                    NonZeroUsize::new(2).unwrap(),
                    NonZeroUsize::new(size).unwrap(),
                )
                .unwrap();
            assert_eq!(lines, ["钓鱼🐟", "last"]);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn injected_timestamp_format_names_the_created_file() {
        let dir = temp_test_dir("create-configurable-log");
        create_log_dir(&dir).unwrap();
        let (path, _) = create_timestamped_log_file(&dir, "header", "%Y%m%d").unwrap();
        let name = path.file_stem().unwrap().to_str().unwrap();
        assert_eq!(name.len(), 8);
        assert!(name.bytes().all(|byte| byte.is_ascii_digit()));
        assert_eq!(fs::read_to_string(path).unwrap(), "header\n");
        fs::remove_dir_all(dir).unwrap();
    }

    /// 从文件末尾读取时，返回结果按文件顺序排列；末尾换行不产生空行。
    #[test]
    fn backward_reader_returns_lines_in_file_order() {
        let dir = temp_test_dir("read-backwards");
        fs::create_dir_all(&dir).expect("建临时目录");
        let log_path = dir.join("2026-09-28-01-47-01.log");
        fs::write(&log_path, "一\n二\n三\n").expect("写日志");

        let all_lines = read_lines_backwards_from(&log_path, 0, |_| false).expect("读日志");
        assert_eq!(all_lines, vec!["一", "二", "三"]);

        // 遇到匹配行后停止扫描，更旧的行不再读取。
        let first_line =
            read_lines_backwards_from(&log_path, 0, |line| line == "三").expect("读日志");
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
            read_lines_backwards_from(&without_newline, 0, |_| false).expect("读日志"),
            vec!["只有一行"]
        );

        let empty = dir.join("empty.log");
        fs::write(&empty, "").expect("写空日志");
        assert!(
            read_lines_backwards_from(&empty, 0, |_| false)
                .expect("读空日志")
                .is_empty()
        );

        let only_newline = dir.join("only-newline.log");
        fs::write(&only_newline, "\n").expect("写日志");
        assert_eq!(
            read_lines_backwards_from(&only_newline, 0, |_| false).expect("读日志"),
            vec![""]
        );

        fs::remove_dir_all(&dir).expect("清理临时目录");
    }

    #[test]
    fn last_lines_after_offset_exclude_older_content() {
        let dir = temp_test_dir("read-after-offset");
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("bot.log");
        let old = "旧回显\n";
        fs::write(&log_path, old).unwrap();
        let offset = old.len() as u64;
        assert!(
            read_last_lines_after(&log_path, offset, 2)
                .unwrap()
                .is_empty()
        );

        fs::write(&log_path, format!("{old}第一行\n第二行\n第三行\n")).unwrap();
        assert_eq!(
            read_last_lines_after(&log_path, offset, 2).unwrap(),
            ["第二行", "第三行"]
        );
        assert!(read_last_lines_after(&log_path, u64::MAX, 2).is_err());
        fs::remove_dir_all(&dir).unwrap();
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

            let read_lines = read_lines_backwards_from(&log_path, 0, |_| false).expect("读日志");
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

        // 总长度明显超过测试使用的单个读块。
        let mut content = String::new();
        for index in 0..2000 {
            content.push_str(&format!("第{index}行-钓鱼中🐟\r\n"));
        }
        fs::write(&log_path, content).expect("写大日志");

        let last_three_lines = read_last_lines_after(&log_path, 0, 3).expect("读最后 3 行");
        assert_eq!(
            last_three_lines,
            vec![
                "第1997行-钓鱼中🐟",
                "第1998行-钓鱼中🐟",
                "第1999行-钓鱼中🐟"
            ]
        );

        // 全文读取用于验证跨块拼接既不丢行，也不重复行。
        let all_lines = read_lines_backwards_from(&log_path, 0, |_| false).expect("读全文");
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
