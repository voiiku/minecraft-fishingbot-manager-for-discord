use crate::config;
use crate::text::report::Report;

/// 返回管理员名单（名称与 Discord 用户 ID）。
pub fn show_admins() -> String {
    let admins = &config::Config::get().admins;
    let i18n_text = crate::config::Config::get().text();

    let mut report = Report::new(
        crate::config::Config::get().i18n.language,
        crate::config::Config::get().runtime.max_reply_chars,
    );
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
