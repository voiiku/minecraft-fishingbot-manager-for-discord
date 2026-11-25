//! 机器人日志文件名的识别与排序。

use chrono::NaiveDateTime;

pub(crate) fn bot_log_time(file_name: &str, format: &str) -> Option<NaiveDateTime> {
    let time_part = file_name.strip_suffix(".log")?;
    let timestamp = NaiveDateTime::parse_from_str(time_part, format).ok()?;
    (timestamp.format(format).to_string() == time_part).then_some(timestamp)
}

pub(crate) fn latest_log_file_name<'a>(
    candidate_file_names: &'a [String],
    format: &str,
) -> Option<&'a str> {
    candidate_file_names
        .iter()
        .filter_map(|file_name| {
            bot_log_time(file_name, format).map(|time| (time, file_name.as_str()))
        })
        .max_by_key(|(time, _)| *time)
        .map(|(_, file_name)| file_name)
}
