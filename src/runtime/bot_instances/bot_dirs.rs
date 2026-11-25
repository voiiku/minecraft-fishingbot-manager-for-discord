//! 机器人目录、归属与凭据状态。
use crate::config;
use crate::runtime::utils::bot_dirs::owner_id_from_dir_name;
use crate::utils::bot_name::{BotNameError, ToCleanBotName};
use crate::utils::fs::list_subdirectory_names;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn bot_name_error(error: BotNameError<'_>) -> String {
    let language = config::Config::get().text();
    match error {
        BotNameError::Empty => language.bot_name_empty().to_string(),
        BotNameError::InvalidCharacter { name, character } => {
            language.bot_name_illegal_char(name, character)
        }
    }
}

/// 机器人目录中 Token 文件的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotTokenStatus {
    /// Token 文件存在，不保证其有效。
    Ready,
    /// Token 文件不存在。
    Missing,
}

impl BotTokenStatus {
    /// 是否已经可以启动。
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// 机器人目录条目及创建时的 Token 状态。
#[derive(Debug, Clone)]
pub struct BotDirEntry {
    /// 所属 Discord 用户 ID。
    pub owner_id: u64,
    /// 机器人目录名称。
    pub bot_name: String,
    /// 机器人目录路径。
    pub bot_dir_path: PathBuf,
    /// 创建条目时的 Token 状态。
    pub bot_token_status: BotTokenStatus,
}

/// Token 文件路径：`<机器人目录>/<runtime.token_file>`。
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
}

impl Display for BotTokenStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(
                f,
                "{}",
                crate::config::Config::get().text().token_status_ready()
            ),
            Self::Missing => write!(
                f,
                "{}",
                crate::config::Config::get().text().token_status_missing()
            ),
        }
    }
}

impl BotDirEntry {
    fn from_dir_name(target_bot_name: String, owner_id: u64) -> Result<Self, String> {
        target_bot_name
            .to_clean_bot_name()
            .map_err(crate::runtime::bot_instances::bot_dirs::bot_name_error)?;

        let bot_dir_path = bot_dir_path(&target_bot_name, owner_id);
        let bot_token_status = BotTokenStatus::from_dir(&bot_dir_path);

        Ok(Self {
            owner_id,
            bot_name: target_bot_name,
            bot_dir_path,
            bot_token_status,
        })
    }

    /// 根据原始名称和所属用户 ID 构造条目。
    pub fn new(raw_bot_name: &str, owner_id: u64) -> Result<Self, String> {
        Self::from_dir_name(
            raw_bot_name
                .to_clean_bot_name()
                .map_err(crate::runtime::bot_instances::bot_dirs::bot_name_error)?,
            owner_id,
        )
    }

    /// 按名称遍历全部用户目录，查找机器人。
    pub fn find_by_name(raw_bot_name: &str) -> Result<Self, String> {
        let clean_bot_name = raw_bot_name
            .to_clean_bot_name()
            .map_err(crate::runtime::bot_instances::bot_dirs::bot_name_error)?;

        all_bot_dir_entries()?
            .into_iter()
            .find(|bot_dir_entry| bot_dir_entry.bot_name == clean_bot_name)
            .ok_or_else(|| {
                crate::config::Config::get()
                    .text()
                    .bot_dir_not_found(&clean_bot_name)
            })
    }
}

/// 根据规范化名称和用户 ID 构造机器人目录路径。
fn bot_dir_path(clean_bot_name: &str, owner_id: u64) -> PathBuf {
    config::Config::get()
        .paths
        .bots_dir
        .join(owner_id.to_string())
        .join(clean_bot_name)
}

/// 获取一个用户目录下的全部机器人条目。
fn bot_dir_entries_in_owner_dir(
    owner_id: u64,
    owner_dir: &Path,
) -> Result<Vec<BotDirEntry>, String> {
    let bot_dir_names = list_subdirectory_names(owner_dir)
        .map_err(|e| crate::bootstrap_error_text().read_bot_list_failed(&e.to_string()))?;

    Ok(bot_dir_names
        .into_iter()
        // 忽略名称不符合规范的目录。
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
        Err(error) => {
            return Err(crate::bootstrap_error_text().read_bots_root_failed(&error.to_string()));
        }
    };

    let mut bot_dir_entries = Vec::new();
    for owner_dir_name in owner_dir_names {
        // 非 Discord 用户 ID 的目录（手工放入的其他目录）跳过。
        let Some(owner_id) = owner_id_from_dir_name(&owner_dir_name) else {
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
        Err(error) => {
            return Err(crate::config::Config::get()
                .text()
                .read_bots_root_failed(&error.to_string()));
        }
    };

    for entry in entries {
        let entry = entry.map_err(|error| {
            crate::config::Config::get()
                .text()
                .read_bots_root_failed(&error.to_string())
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if owner_id_from_dir_name(name).is_none() {
            continue;
        }
        match fs::metadata(entry.path()) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(crate::config::Config::get()
                    .text()
                    .read_bots_root_failed(&error.to_string()));
            }
        }
        let candidate = entry.path().join(clean_bot_name);
        match fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(crate::config::Config::get()
                    .text()
                    .read_bot_list_failed(&error.to_string()));
            }
        }
    }
    Ok(false)
}

/// 创建机器人数据目录，成功时返回新建的机器人条目。
pub fn create_bot_dir(raw_bot_name: &str, caller_id: u64) -> Result<BotDirEntry, String> {
    let bot_dir_entry = BotDirEntry::new(raw_bot_name, caller_id)?;

    if is_bot_name_taken(&bot_dir_entry.bot_name)? {
        return Err(crate::config::Config::get()
            .text()
            .bot_add_name_taken(&bot_dir_entry.bot_name));
    }

    fs::create_dir_all(&bot_dir_entry.bot_dir_path).map_err(|e| {
        crate::config::Config::get()
            .text()
            .create_bot_dir_failed(&e.to_string())
    })?;

    Ok(bot_dir_entry)
}

/// 获取全部 Token 已存在的机器人目录条目，按机器人名排序。
/// 停止和进程收尾应使用 [`all_bot_dir_entries`]，以包含运行期间 Token 被删除的会话。
pub(crate) fn token_ready_bot_dir_entries() -> Result<Vec<BotDirEntry>, String> {
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

/// 按机器人名排序；所有涉及机器人名的场景共用此排序。
fn sort_by_bot_name(bot_dir_entries: &mut [BotDirEntry]) {
    bot_dir_entries.sort_unstable_by(|left, right| left.bot_name.cmp(&right.bot_name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_check_distinguishes_absence_from_io_failure() {
        crate::config::init_for_test();
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
    /// 名字由 `utils::fs::list_subdirectory_names` 从磁盘读出来（非 UTF-8 的名字在那里已被跳过），
    /// 本函数只做解析、不碰磁盘也不读配置，所以能直接单测。
    #[test]
    fn owner_dir_names_are_parsed_as_user_ids() {
        assert_eq!(owner_id_from_dir_name("12345"), Some(12345));
        // 机器人的目录名（`duck`）不是用户 ID，所以拿机器人目录名来问会得到 `None`。
        assert_eq!(owner_id_from_dir_name("duck"), None);
        assert_eq!(owner_id_from_dir_name("abc"), None);
        // 空目录名同样算不出归属。
        assert_eq!(owner_id_from_dir_name(""), None);
        // `parse` 会接受带 `+` 与前导 `0` 的写法，但那不是雪花 ID 的形状，一律不算归属目录。
        assert_eq!(owner_id_from_dir_name("+12345"), None);
        assert_eq!(owner_id_from_dir_name("007"), None);
    }
}
