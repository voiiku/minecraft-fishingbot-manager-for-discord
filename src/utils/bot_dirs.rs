//! 机器人目录相关的工具函数：路径拼接、查找、创建与遍历。
//!
//! 目录布局固定为 `base_dir/bots/Discord用户ID/机器人名`：归属由 Discord 用户 ID 决定，
//! 机器人目录名由 [`ToCleanBotName::to_clean_bot_name`] 转换后的机器人名决定。
//! 所有查询机器人路径都调用这里的工具函数；而机器人实际运行状态由 [`super::screen`] 判断。

use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::text;

use super::bot_name::ToCleanBotName;
use super::fs::list_subdirectory_names;

/// 机器人目录下 refreshToken 文件的状态，并提供相应的检测与打印方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotTokenStatus {
    /// Token 文件已经存在（目前不检验真实性），可以启动登录。
    Ready,
    /// 尚无 Token 文件。
    Missing,
}

/// Token 文件路径：`<机器人目录>/<runtime.token_file>`。
/// 文件名只在此处从配置读取一次，存在性判断与 Token 写入均使用该文件名。
pub fn token_file_path(bot_dir: &Path) -> PathBuf {
    bot_dir.join(&config::Config::get().runtime.token_file)
}

impl BotTokenStatus {
    /// 检查一个目录下是否存在 Token 文件（文件名来自配置的 `runtime.token_file`）。
    pub fn from_dir(bot_dir: &Path) -> Self {
        if token_file_path(bot_dir).exists() {
            Self::Ready
        } else {
            Self::Missing
        }
    }

    /// 是否已经可以启动。
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl Display for BotTokenStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "{}", text::get().token_status_ready()),
            Self::Missing => write!(f, "{}", text::get().token_status_missing()),
        }
    }
}

/// 单个机器人的目录条目：归属 Discord 用户 ID、机器人名、文件夹路径，以及构造时的 Token 状态。
/// 文件夹路径按 `base_dir/bots/Discord用户ID/机器人名` 拼接，目录不一定真实存在。
/// 该条目同时充当数据对象：在构造时完成机器人名校验与清洗，再向启动、停止、状态、列表各条管道传递，下游无需重复拼接路径或再次清洗。
#[derive(Debug, Clone)]
pub struct BotDirEntry {
    /// 归属 Discord 用户 ID：机器人挂在哪个 Discord 用户 ID 目录下。
    pub owner_id: u64,
    /// 机器人文件夹名。
    pub bot_name: String,
    /// 机器人文件夹路径。
    pub bot_dir_path: PathBuf,
    /// Token 文件状态：只在构造条目时取一次。
    pub bot_token_status: BotTokenStatus,
}
/// 三条构造入口：`new`（已知归属 Discord 用户 ID）、`find_by_name`（按名字找）、`from_dir_name`（磁盘列表）。
impl BotDirEntry {
    fn from_dir_name(target_bot_name: String, owner_id: u64) -> Result<Self, String> {
        // 校验名字合法性。
        target_bot_name.to_clean_bot_name()?;

        let bot_dir_path = bot_dir_path(&target_bot_name, owner_id);
        let bot_token_status = BotTokenStatus::from_dir(&bot_dir_path);

        Ok(Self {
            owner_id,
            bot_name: target_bot_name,
            bot_dir_path,
            bot_token_status,
        })
    }

    /// 原始机器人名与归属 Discord 用户 ID，返回条目或异常名字的报错信息。
    pub fn new(raw_bot_name: &str, owner_id: u64) -> Result<Self, String> {
        Self::from_dir_name(raw_bot_name.to_clean_bot_name()?, owner_id)
    }

    /// 原始机器人名：开销较高（需遍历全服所有用户目录按名字查找），仅在无现成信息时调用。
    pub fn find_by_name(raw_bot_name: &str) -> Result<Self, String> {
        let clean_bot_name = raw_bot_name.to_clean_bot_name()?;

        all_bot_dir_entries()?
            .into_iter()
            .find(|bot_dir_entry| bot_dir_entry.bot_name == clean_bot_name)
            .ok_or_else(|| text::get().bot_dir_not_found(&clean_bot_name))
    }
}

/// 由已清洗的机器人名与归属 Discord 用户 ID 拼接出机器人目录的理论路径（`base_dir/bots/Discord用户ID/机器人名`）。
fn bot_dir_path(clean_bot_name: &str, owner_id: u64) -> PathBuf {
    config::Config::get()
        .paths
        .bots_dir
        .join(owner_id.to_string())
        .join(clean_bot_name)
}

/// 获取一个目录名对应的归属 Discord 用户 ID；该名字不是归属目录（不是雪花 ID 形状）时返回 `None`。
///
/// 是否为归属目录由 `is_some()` 判定，具体归属用户即此 id：
/// [`all_bot_dir_entries`] 需要 id 建立条目，[`is_bot_name_taken`] 只判是否归属。
/// 只接受纯数字且不以 `0` 开头的名字：`parse` 本身会接受 `+1`、`007` 这类写法，
/// 若照此解析，管理员手工建出的 `007` 目录会被当成用户 7 的归属目录。
/// 名字来自 [`list_subdirectory_names`]，解析失败只返回 `None`。
fn from_dir_name_to_owner_id(dir_name: &str) -> Option<u64> {
    if dir_name.starts_with('0') || !dir_name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    dir_name.parse::<u64>().ok()
}

/// 获取一个用户目录下的全部机器人条目。
fn bot_dir_entries_in_owner_dir(
    owner_id: u64,
    owner_dir: &Path,
) -> Result<Vec<BotDirEntry>, String> {
    let bot_dir_names = list_subdirectory_names(owner_dir)
        .map_err(|e| text::get().read_bot_list_failed(&e.to_string()))?;

    Ok(bot_dir_names
        .into_iter()
        // 非法名（例如管理员手工建的中文目录名）交由 `from_dir_name` 校验，未通过则跳过。
        .filter_map(|dir_bot_name| BotDirEntry::from_dir_name(dir_bot_name, owner_id).ok())
        .collect())
}

/// 全服所有机器人目录条目，按机器人名排序。
/// 目录不存在视为正常空结果，读取失败则显式报错。
pub fn all_bot_dir_entries() -> Result<Vec<BotDirEntry>, String> {
    let bots_dir = &config::Config::get().paths.bots_dir;

    // `NotFound` 表示尚无任何机器人；其余读取失败才是错误。
    let owner_dir_names = match list_subdirectory_names(bots_dir) {
        Ok(owner_dir_names) => owner_dir_names,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(text::get().read_bots_root_failed(&error.to_string())),
    };

    let mut bot_dir_entries = Vec::new();
    for owner_dir_name in owner_dir_names {
        // 非 Discord 用户 ID 的目录（手工放入的其他目录）跳过。
        let Some(owner_id) = from_dir_name_to_owner_id(&owner_dir_name) else {
            continue;
        };

        // 单个用户目录读取失败时跳过，不影响其他账号。
        let owner_dir_path = bots_dir.join(&owner_dir_name);
        if let Ok(owner_bot_dir_entries) = bot_dir_entries_in_owner_dir(owner_id, &owner_dir_path) {
            bot_dir_entries.extend(owner_bot_dir_entries);
        }
    }

    sort_by_bot_name(&mut bot_dir_entries);
    Ok(bot_dir_entries)
}

/// 检查磁盘上是否已存在同名的机器人目录，用于 `/bot add` 与 `/bot rename` 的重名检查。
/// 传入参数必须是 [`ToCleanBotName::to_clean_bot_name`] 转过的名字。
/// 根目录不存在表示未占用；读取失败返回错误。
pub fn is_bot_name_taken(clean_bot_name: &str) -> Result<bool, String> {
    bot_name_taken_in(&config::Config::get().paths.bots_dir, clean_bot_name)
}

fn bot_name_taken_in(bots_dir: &Path, clean_bot_name: &str) -> Result<bool, String> {
    let entries = match fs::read_dir(bots_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(text::get().read_bots_root_failed(&error.to_string())),
    };

    for entry in entries {
        let entry = entry.map_err(|error| text::get().read_bots_root_failed(&error.to_string()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if from_dir_name_to_owner_id(name).is_none() {
            continue;
        }
        match fs::metadata(entry.path()) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(text::get().read_bots_root_failed(&error.to_string())),
        }
        let candidate = entry.path().join(clean_bot_name);
        match fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(text::get().read_bot_list_failed(&error.to_string())),
        }
    }
    Ok(false)
}

/// 创建机器人数据目录，成功时返回新建的机器人条目。
pub fn create_bot_dir(raw_bot_name: &str, caller_id: u64) -> Result<BotDirEntry, String> {
    let bot_dir_entry = BotDirEntry::new(raw_bot_name, caller_id)?;

    if is_bot_name_taken(&bot_dir_entry.bot_name)? {
        return Err(text::get().bot_add_name_taken(&bot_dir_entry.bot_name));
    }

    fs::create_dir_all(&bot_dir_entry.bot_dir_path)
        .map_err(|e| text::get().create_bot_dir_failed(&e.to_string()))?;

    Ok(bot_dir_entry)
}

/// 获取全部 Token 已存在的机器人目录条目，按机器人名排序。
/// 兜底收尾类操作（`/stopall`、进程退出）应改用 [`all_bot_dir_entries`]：这些场景需要把孤儿进程计入（即运行期间 Token 被删除）。
pub fn token_ready_bot_dir_entries() -> Result<Vec<BotDirEntry>, String> {
    let mut bot_dir_entries = all_bot_dir_entries()?;
    bot_dir_entries.retain(|bot_dir_entry| bot_dir_entry.bot_token_status.is_ready());

    Ok(bot_dir_entries)
}

/// 获取某个 Discord 用户 ID 下的全部机器人目录条目，按机器人名排序；该用户目录不存在时返回空列表。
pub fn bot_dir_entries_of_owner(query_owner_id: u64) -> Result<Vec<BotDirEntry>, String> {
    let query_dir = config::Config::get()
        .paths
        .bots_dir
        .join(query_owner_id.to_string());
    if !query_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut bot_dir_entries = bot_dir_entries_in_owner_dir(query_owner_id, &query_dir)?;
    sort_by_bot_name(&mut bot_dir_entries);

    Ok(bot_dir_entries)
}

/// 按 Token 状态把一批机器人目录分组：有 Token 与无 Token 两组均交回调用方。
pub fn split_bot_entries_by_token_status(
    bot_entries: &[BotDirEntry],
) -> (Vec<BotDirEntry>, Vec<BotDirEntry>) {
    bot_entries
        .iter()
        .cloned()
        .partition(|bot_dir_entry| bot_dir_entry.bot_token_status.is_ready())
}

/// 按机器人名排序；所有涉及机器人名的场景共用此排序。
fn sort_by_bot_name(bot_dir_entries: &mut [BotDirEntry]) {
    bot_dir_entries.sort_unstable_by(|left, right| left.bot_name.cmp(&right.bot_name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_check_distinguishes_absence_from_io_failure() {
        let dir = crate::test_support::temp_test_dir("name-lookup");
        fs::create_dir_all(dir.join("123")).unwrap();
        fs::write(dir.join("456"), b"not a directory").unwrap();
        assert!(!bot_name_taken_in(&dir.join("missing"), "duck").unwrap());
        assert!(!bot_name_taken_in(&dir, "duck").unwrap());

        fs::create_dir(dir.join("123").join("duck")).unwrap();
        assert!(bot_name_taken_in(&dir, "duck").unwrap());
        assert!(bot_name_taken_in(&dir.join("456"), "duck").is_err());

        std::os::unix::fs::symlink("goose", dir.join("123").join("goose")).unwrap();
        assert!(bot_name_taken_in(&dir, "goose").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// 归属目录名必须是纯数字的 Discord 用户 ID；其他名字（例如管理员手工放入的目录）均不成立。
    ///
    /// 名字由 `super::fs::list_subdirectory_names` 从磁盘读出来（非 UTF-8 的名字在那里已被跳过），
    /// 本函数只做解析、不碰磁盘也不读配置，所以能直接单测。
    #[test]
    fn owner_dir_names_are_parsed_as_user_ids() {
        assert_eq!(from_dir_name_to_owner_id("12345"), Some(12345));
        // 机器人的目录名（`duck`）不是用户 ID，所以拿机器人目录名来问会得到 `None`。
        assert_eq!(from_dir_name_to_owner_id("duck"), None);
        assert_eq!(from_dir_name_to_owner_id("abc"), None);
        // 空目录名同样算不出归属。
        assert_eq!(from_dir_name_to_owner_id(""), None);
        // `parse` 会接受带 `+` 与前导 `0` 的写法，但那不是雪花 ID 的形状，一律不算归属目录。
        assert_eq!(from_dir_name_to_owner_id("+12345"), None);
        assert_eq!(from_dir_name_to_owner_id("007"), None);
    }
}
