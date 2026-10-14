//! `/show admins`：列出管理员名单。

use crate::config;
use crate::text;
use crate::utils::Report;

/// 返回管理员名单（名称与 Discord 用户 ID）。
pub(crate) fn admins_list() -> String {
    let admins = config::admins();
    let i18n_text = text::get();

    let mut report = Report::new();
    report.set_title(i18n_text.admins_title());
    if admins.is_empty() {
        report.basics.push(i18n_text.admins_empty().to_string());
        return report.render();
    }

    report
        .basics
        .push(i18n_text.common_total_count(admins.iter().count()));
    report.basics.extend(
        admins
            .iter()
            .map(|(user_id, name)| i18n_text.admins_row(name, user_id)),
    );

    report.render()
}
