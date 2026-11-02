//! 启动结果及其报告转换。

#[cfg(test)]
pub(crate) use crate::models::BotStartEntry;
pub use crate::models::{BotStartOutcome, BotStartResults};
use crate::text;
use crate::utils::{IntoReplyText, Report};

impl BotStartResults {
    /// 根据启动结果生成报告，计数由调用方设置。
    pub(crate) fn into_report(self) -> Report {
        let mut report = Report::new();
        let i18n_text = text::get();

        for (bot_dir_entry, outcome) in self.iter() {
            match outcome {
                BotStartOutcome::Succeeded => {
                    report
                        .succeeded
                        .push(if self.online_confirmed(&bot_dir_entry.bot_name) {
                            i18n_text.start_command_sent_confirmed(&bot_dir_entry.bot_name)
                        } else {
                            i18n_text.start_command_sent_unconfirmed(&bot_dir_entry.bot_name)
                        })
                }
                BotStartOutcome::Skipped => report
                    .skipped
                    .push(i18n_text.startall_skipped_running(&bot_dir_entry.bot_name)),
                BotStartOutcome::Cancelled => report
                    .skipped
                    .push(i18n_text.start_cancelled(&bot_dir_entry.bot_name)),
                BotStartOutcome::Failure(failure_line) => report.failed.push(failure_line.clone()),
            }
        }

        report
    }
}

impl IntoReplyText for BotStartResults {
    fn to_reply_text(self) -> String {
        self.into_report().render()
    }
}
