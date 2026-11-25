use super::pause_report::set_all_bots_fishing_pause;
use crate::config::Config;
use crate::operations::{PreparedControl, require_admin};

pub fn unpause_all_bots(caller_id: u64) -> Result<String, String> {
    prepare_unpause_all_bots(caller_id)?.execute()
}

pub(crate) fn prepare_unpause_all_bots(caller_id: u64) -> Result<PreparedControl, String> {
    require_admin(caller_id)?;
    let notice = Config::get().text().command_started_notice("/unpauseall");
    Ok(PreparedControl::new(notice, || {
        set_all_bots_fishing_pause(false)
    }))
}
