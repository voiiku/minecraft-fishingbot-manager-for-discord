use crate::{config, text::report::Report};

/// 返回 `/help` 帮助文本。
pub fn help() -> String {
    let config = config::Config::get();
    let mut report = Report::new(config.i18n.language, config.runtime.max_reply_chars);
    report.set_title(config.text().help_title());
    report.content = Some(config.text.help.clone());
    report.render()
}
