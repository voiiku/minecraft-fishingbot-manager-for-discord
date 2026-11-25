use crate::config;

/// 游戏侧配置附件数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishingbotConfigView {
    /// 配置文件名（附件的文件名就是它，通常是 `config.json`）。
    pub file_name: String,
    /// 配置全文。
    pub content: String,
}

/// 读取由本程序生成的共享 FishingBot 配置。
pub fn show_bot_config() -> Result<FishingbotConfigView, String> {
    let fishingbot_config_path = &config::Config::get().paths.fishingbot_config;

    let content = std::fs::read_to_string(fishingbot_config_path).map_err(|e| {
        crate::config::Config::get().text().botconfig_read_failed(
            &fishingbot_config_path.display().to_string(),
            &e.to_string(),
        )
    })?;

    // 配置文件不截断；超过附件上限时返回错误。
    let max_attachment_bytes = config::Config::get().constants.max_attachment_bytes;
    if content.len() > max_attachment_bytes {
        return Err(crate::config::Config::get()
            .text()
            .botconfig_too_large(content.len(), max_attachment_bytes));
    }

    let file_name = fishingbot_config_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("config.json")
        .to_string();

    Ok(FishingbotConfigView { file_name, content })
}
