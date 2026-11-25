//! 日志文件操作的运行时文案适配。

use std::fs::File;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use crate::config;
use crate::utils::fs::{self, CreateLogError, ReadLogError};

/// 创建时间戳日志文件并写入首行。
pub fn create_timestamped_log_file(
    dir: &Path,
    first_line: &str,
) -> Result<(PathBuf, File), String> {
    fs::create_log_dir(dir).map_err(localize_create_error)?;
    let format = config::Config::get().constants.log_filename_time_format;
    fs::create_timestamped_log_file(dir, first_line, format).map_err(localize_create_error)
}

/// 读取文件末尾的指定行数。
pub fn read_last_lines(file_path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    read_last_lines_after(file_path, 0, max_lines)
}

/// 读取指定字节偏移之后的末尾行。
pub(crate) fn read_last_lines_after(
    file_path: &Path,
    start: u64,
    max_lines: usize,
) -> Result<Vec<String>, String> {
    let max_lines = NonZeroUsize::new(max_lines).ok_or_else(|| {
        crate::config::Config::get()
            .text()
            .bot_log_zero_lines()
            .to_string()
    })?;
    let reader = fs::LogReader::open(file_path, start).map_err(localize_read_error)?;
    if reader.is_empty() {
        return Ok(Vec::new());
    }
    let block_bytes = config::Config::get().constants.log_read_block_bytes;
    reader
        .read_last_lines(max_lines, block_bytes)
        .map_err(localize_read_error)
}

fn localize_create_error(error: CreateLogError) -> String {
    let text = crate::bootstrap_error_text();
    match error {
        CreateLogError::CreateDir(path, error) => {
            text.create_log_dir_failed(&path.display().to_string(), &error.to_string())
        }
        CreateLogError::CreateFile(path, error) => {
            text.create_log_file_failed(&path.display().to_string(), &error.to_string())
        }
        CreateLogError::WriteFirstLine(path, error) => {
            text.write_log_first_line_failed(&path.display().to_string(), &error.to_string())
        }
    }
}

fn localize_read_error(error: ReadLogError) -> String {
    let text = crate::config::Config::get().text();
    match error {
        ReadLogError::Open(error) => text.open_log_file_failed(&error.to_string()),
        ReadLogError::Metadata(error) => text.read_log_metadata_failed(&error.to_string()),
        ReadLogError::Truncated => {
            text.read_log_content_failed("log truncated before the requested offset")
        }
        ReadLogError::Seek(error) => text.seek_log_file_failed(&error.to_string()),
        ReadLogError::Read(error) => text.read_log_content_failed(&error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn file_errors_keep_their_localized_context() {
        crate::config::init_for_test();
        let dir = crate::test_support::temp_test_dir("log-file-errors");
        fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.log");
        let io_error = File::open(&missing).unwrap_err();
        assert_eq!(
            read_last_lines(&missing, 1).unwrap_err(),
            crate::config::Config::get()
                .text()
                .open_log_file_failed(&io_error.to_string())
        );

        let file = dir.join("not-a-directory");
        fs::write(&file, "line\n").unwrap();
        assert_eq!(
            read_last_lines_after(&file, u64::MAX, 1).unwrap_err(),
            crate::config::Config::get()
                .text()
                .read_log_content_failed("log truncated before the requested offset")
        );
        let io_error = fs::create_dir_all(&file).unwrap_err();
        assert_eq!(
            create_timestamped_log_file(&file, "first").unwrap_err(),
            crate::bootstrap_error_text()
                .create_log_dir_failed(&file.display().to_string(), &io_error.to_string())
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
