use super::permitted_bot_dir_entry;
use crate::config::Config;
use crate::runtime::bot_instances::bot_dirs::{BotDirEntry, BotTokenStatus};

/// 已鉴权的单次控制操作。发送进度回执后调用 `execute`；丢弃它不会执行操作。
#[must_use]
pub(crate) struct PreparedControl {
    pub(crate) notice: String,
    run: Box<dyn FnOnce() -> Result<String, String> + Send>,
}

impl PreparedControl {
    pub(super) fn new(
        notice: String,
        run: impl FnOnce() -> Result<String, String> + Send + 'static,
    ) -> Self {
        Self {
            notice,
            run: Box::new(run),
        }
    }

    pub(super) fn for_bot(
        name: &str,
        caller_id: u64,
        notice: impl FnOnce(&dyn crate::text::Text) -> String,
        run: impl FnOnce(BotDirEntry) -> Result<String, String> + Send + 'static,
    ) -> Result<Self, String> {
        let mut bot = permitted_bot_dir_entry(name, caller_id)?;
        Ok(Self::new(notice(Config::get().text()), move || {
            // 回执期间目标可能已被移动；不再按名字查找并操作另一个所有者的同名会话。
            if !bot.bot_dir_path.is_dir() {
                return Err(Config::get().text().bot_dir_not_found(&bot.bot_name));
            }
            bot.bot_token_status = BotTokenStatus::from_dir(&bot.bot_dir_path);
            run(bot)
        }))
    }

    pub(crate) fn execute(self) -> Result<String, String> {
        (self.run)()
    }
}
