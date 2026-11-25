//! 机器人目录名的基础解析。

pub(crate) fn owner_id_from_dir_name(dir_name: &str) -> Option<u64> {
    if dir_name.starts_with('0') || !dir_name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    dir_name.parse::<u64>().ok()
}
