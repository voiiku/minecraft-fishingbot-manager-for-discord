use crate::operations::PreparedControl;
use crate::runtime::bot_instances::fishing_pause::set_fishing_pause;

pub fn unpause_single_bot(name: &str, caller_id: u64) -> Result<String, String> {
    prepare_unpause_single_bot(name, caller_id)?.execute()
}

pub(crate) fn prepare_unpause_single_bot(
    name: &str,
    caller_id: u64,
) -> Result<PreparedControl, String> {
    PreparedControl::for_bot(
        name,
        caller_id,
        |text| text.command_started_notice(&format!("/unpause {name}")),
        |bot| set_fishing_pause(&bot, false),
    )
}
