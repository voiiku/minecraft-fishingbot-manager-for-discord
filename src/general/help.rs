//! `/help`：帮助菜单。

use crate::config;
use crate::text;
use crate::utils::Report;

/// 返回帮助菜单。
///
/// 内容来自配置 `text.help`。
pub(crate) fn help() -> String {
    let mut report = Report::new();
    report.set_title(text::get().help_title());
    report.content = Some(config::get().text.help.clone());

    report.render()
}
