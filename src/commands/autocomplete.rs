//! Discord autocomplete：候选的来源、筛选、显示与数量上限。

use poise::serenity_prelude as serenity;

use minecraft_fishingbot_manager_for_discord::{
    BotDirEntry, RequiredPermission, all_bot_dir_entries, bot_dir_entries_of_owner, config,
    has_permission, split_bot_entries_by_is_running,
};

use super::{Context, text};

/// 按命令语义筛选的机器人运行状态范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BotNameScope {
    RunningOnly,
    StoppedOnly,
    AnyState,
}

/// 返回有权限且正在运行的机器人候选。
pub(crate) async fn autocomplete_running_bot_names(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    bot_name_choices(ctx, partial, BotNameScope::RunningOnly).await
}

/// 返回有权限且未运行的机器人候选。
pub(crate) async fn autocomplete_stopped_bot_names(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    bot_name_choices(ctx, partial, BotNameScope::StoppedOnly).await
}

/// 返回有权限的全部机器人候选。
pub(crate) async fn autocomplete_any_bot_names(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    bot_name_choices(ctx, partial, BotNameScope::AnyState).await
}

/// 在线程池查询候选，避免目录与 `screen -list` 等同步 I/O 阻塞 Tokio worker。
async fn bot_name_choices(
    ctx: Context<'_>,
    partial: &str,
    scope: BotNameScope,
) -> serenity::CreateAutocompleteResponse {
    let caller_id = ctx.author().id.get();
    let partial = partial.to_string();
    let choices =
        match tokio::task::spawn_blocking(move || bot_name_candidate_states(caller_id)).await {
            Ok(Ok(candidates)) => {
                let max_choices = config::Config::get().runtime.max_autocomplete_choices;
                select_candidates(&candidates, &partial, scope, max_choices)
                    .into_iter()
                    .map(|(display_name, bot_name)| {
                        serenity::AutocompleteChoice::new(display_name, bot_name)
                    })
                    .collect()
            }
            Ok(Err(error)) => {
                ::log::warn!("Autocomplete candidate lookup failed: {error}");
                Vec::new()
            }
            Err(error) => {
                ::log::warn!("Autocomplete candidate lookup task failed: {error}");
                Vec::new()
            }
        };

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

/// 返回调用者有权限查看的机器人名称与运行状态快照。
/// 管理员得到全部机器人；普通用户只得到自己名下的机器人（与 `OwnerOrAdmin` 同一口径）。
/// 只查一次 `screen -list` 快照；运行中的排前面。
fn bot_name_candidate_states(caller_id: u64) -> Result<Vec<(String, bool)>, String> {
    let bot_dir_entries = if has_permission(caller_id, RequiredPermission::MustBeAdmin) {
        all_bot_dir_entries()?
    } else {
        bot_dir_entries_of_owner(caller_id)?
    };
    let (running_entries, stopped_entries) = split_bot_entries_by_is_running(&bot_dir_entries);

    Ok(bot_names_sorted(running_entries)
        .into_iter()
        .map(|bot_name| (bot_name, true))
        .chain(
            bot_names_sorted(stopped_entries)
                .into_iter()
                .map(|bot_name| (bot_name, false)),
        )
        .collect())
}

/// 取条目里的机器人名并按名字排序（候选按名有序，方便查找）。
fn bot_names_sorted(bot_dir_entries: Vec<BotDirEntry>) -> Vec<String> {
    let mut bot_names: Vec<String> = bot_dir_entries
        .into_iter()
        .map(|bot_dir_entry| bot_dir_entry.bot_name)
        .collect();
    bot_names.sort();

    bot_names
}

/// 按状态范围、输入前缀和条数上限组装显示名与实际 option 值。
fn select_candidates(
    candidates: &[(String, bool)],
    partial: &str,
    scope: BotNameScope,
    max_choices: usize,
) -> Vec<(String, String)> {
    let partial = partial.trim().to_lowercase();
    let i18n_text = text::get();

    candidates
        .iter()
        .filter(|(_, is_running)| match scope {
            BotNameScope::RunningOnly => *is_running,
            BotNameScope::StoppedOnly => !*is_running,
            BotNameScope::AnyState => true,
        })
        .filter(|(bot_name, _)| bot_name.starts_with(&partial))
        .take(max_choices)
        .map(|(bot_name, is_running)| {
            let display_name = match (scope, is_running) {
                (BotNameScope::AnyState, true) => {
                    format!("{bot_name}{}", i18n_text.bot_candidate_running_note())
                }
                (BotNameScope::AnyState, false) => {
                    format!("{bot_name}{}", i18n_text.bot_candidate_stopped_note())
                }
                _ => bot_name.clone(),
            };
            (display_name, bot_name.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用候选上限（真实上限来自 `runtime.max_autocomplete_choices`）。
    const TEST_MAX_CHOICES: usize = 25;

    fn sample_candidates() -> Vec<(String, bool)> {
        [
            ("duck", true),
            ("red", true),
            ("cup", false),
            ("duck2", false),
        ]
        .into_iter()
        .map(|(name, is_running)| (name.to_string(), is_running))
        .collect()
    }

    fn values(candidates: &[(String, bool)], partial: &str, scope: BotNameScope) -> Vec<String> {
        select_candidates(candidates, partial, scope, TEST_MAX_CHOICES)
            .into_iter()
            .map(|(_, value)| value)
            .collect()
    }

    #[test]
    fn scopes_filter_states_and_keep_running_candidates_first() {
        let candidates = sample_candidates();

        assert_eq!(
            values(&candidates, "", BotNameScope::RunningOnly),
            ["duck", "red"].map(String::from)
        );
        assert_eq!(
            values(&candidates, "", BotNameScope::StoppedOnly),
            ["cup", "duck2"].map(String::from)
        );
        assert_eq!(
            values(&candidates, "", BotNameScope::AnyState),
            ["duck", "red", "cup", "duck2"].map(String::from)
        );
    }

    #[test]
    fn prefix_matching_is_case_insensitive_and_empty_prefix_matches_all() {
        let candidates = sample_candidates();
        assert_eq!(
            values(&candidates, "DU", BotNameScope::AnyState),
            ["duck", "duck2"].map(String::from)
        );
        assert_eq!(values(&candidates, "", BotNameScope::AnyState).len(), 4);
        assert!(values(&candidates, "zzz", BotNameScope::AnyState).is_empty());
    }

    #[test]
    fn any_state_display_name_marks_state_but_value_stays_plain() {
        let pairs = select_candidates(
            &sample_candidates(),
            "",
            BotNameScope::AnyState,
            TEST_MAX_CHOICES,
        );
        let i18n_text = text::get();
        assert_eq!(
            pairs[0],
            (
                format!("duck{}", i18n_text.bot_candidate_running_note()),
                "duck".to_string()
            )
        );
        assert_eq!(
            pairs[2],
            (
                format!("cup{}", i18n_text.bot_candidate_stopped_note()),
                "cup".to_string()
            )
        );
    }

    #[test]
    fn candidates_are_capped_at_the_platform_limit() {
        let many: Vec<(String, bool)> = (0..TEST_MAX_CHOICES + 10)
            .map(|index| (format!("bot{index:02}"), index % 2 == 0))
            .collect();
        assert_eq!(
            select_candidates(&many, "", BotNameScope::AnyState, TEST_MAX_CHOICES).len(),
            TEST_MAX_CHOICES
        );
    }
}
