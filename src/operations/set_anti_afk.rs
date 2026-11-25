use crate::{operations::require_admin, runtime::anti_afk};

pub async fn set_anti_afk(enabled: bool, caller_id: u64) -> Result<String, String> {
    require_admin(caller_id)?;
    let t = crate::config::Config::get().text();
    if enabled {
        anti_afk::start_anti_afk().await.map(|started| {
            if started {
                t.anti_afk_started()
            } else {
                t.anti_afk_already_running()
            }
            .to_string()
        })
    } else {
        anti_afk::stop_anti_afk()
            .await
            .map(|()| t.anti_afk_stopped().to_string())
    }
}
