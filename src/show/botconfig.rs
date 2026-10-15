//! `/show botconfig`：返回所有机器人共用的游戏侧配置（以附件发出）。
//!
//! 返回配置文件名与内容，由命令层作为附件发送。

use crate::config;
use crate::text;
use crate::utils::MAX_ATTACHMENT_BYTES;

/// 游戏侧配置附件数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishingbotConfigView {
    /// 配置文件名（附件的文件名就是它，通常是 `conf.json`）。
    pub file_name: String,
    /// 配置全文。
    pub content: String,
}

/// 读取由本程序生成的共享 FishingBot 配置。
pub(crate) fn fishingbot_config_view() -> Result<FishingbotConfigView, String> {
    let fishingbot_config_path = &config::Config::get().paths.fishingbot_config;

    let content = std::fs::read_to_string(fishingbot_config_path).map_err(|e| {
        text::get().botconfig_read_failed(
            &fishingbot_config_path.display().to_string(),
            &e.to_string(),
        )
    })?;

    // 配置文件不截断；超过附件上限时返回错误。
    if content.len() > MAX_ATTACHMENT_BYTES {
        return Err(text::get().botconfig_too_large(content.len(), MAX_ATTACHMENT_BYTES));
    }

    let file_name = fishingbot_config_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("conf.json")
        .to_string();

    Ok(FishingbotConfigView { file_name, content })
}
