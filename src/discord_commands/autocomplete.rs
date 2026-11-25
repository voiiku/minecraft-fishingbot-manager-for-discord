//! Discord autocomplete：候选的来源、筛选、显示与数量上限。

use poise::serenity_prelude as serenity;

use crate::config;
use crate::operations::queries;

use super::Context;

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
        match tokio::task::spawn_blocking(move || queries::list_bot_candidates(caller_id)).await {
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
                ::log::warn!(
                    "{}",
                    crate::config::Config::get()
                        .text()
                        .autocomplete_lookup_failed(&error)
                );
                Vec::new()
            }
            Err(error) => {
                ::log::warn!(
                    "{}",
                    crate::config::Config::get()
                        .text()
                        .autocomplete_task_failed(&error.to_string())
                );
                Vec::new()
            }
        };

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

/// 按状态范围、输入前缀和条数上限组装显示名与实际 option 值。
fn select_candidates(
    candidates: &[(String, bool, bool)],
    partial: &str,
    scope: BotNameScope,
    max_choices: usize,
) -> Vec<(String, String)> {
    let partial = partial.trim().to_lowercase();
    let i18n_text = crate::config::Config::get().text();

    candidates
        .iter()
        .filter(|(_, is_running, in_queue)| match scope {
            BotNameScope::RunningOnly => *is_running,
            // 排队中的机器人可能随时启动，不属于已停止状态。
            BotNameScope::StoppedOnly => !*is_running && !*in_queue,
            BotNameScope::AnyState => true,
        })
        .filter(|(bot_name, _, _)| bot_name.starts_with(&partial))
        .take(max_choices)
        .map(|(bot_name, is_running, in_queue)| {
            let display_name = match (scope, is_running, in_queue) {
                (BotNameScope::AnyState, true, _) => {
                    format!("{bot_name}{}", i18n_text.bot_candidate_running_note())
                }
                (BotNameScope::AnyState, false, true) => {
                    format!("{bot_name}{}", i18n_text.bot_candidate_queued_note())
                }
                (BotNameScope::AnyState, false, false) => {
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

    /// 测试需要文案时显式加载示例配置。
    fn init_test_config() {
        crate::config::init_for_test();
    }

    /// 测试用候选上限（真实上限来自 `runtime.max_autocomplete_choices`）。
    const TEST_MAX_CHOICES: usize = 25;

    fn sample_candidates() -> Vec<(String, bool, bool)> {
        [
            // duck 运行中，red 排队中，cup 与 duck2 均未运行且未排队。
            ("duck", true, false),
            ("red", false, true),
            ("cup", false, false),
            ("duck2", false, false),
        ]
        .into_iter()
        .map(|(name, is_running, in_queue)| (name.to_string(), is_running, in_queue))
        .collect()
    }

    fn values(
        candidates: &[(String, bool, bool)],
        partial: &str,
        scope: BotNameScope,
    ) -> Vec<String> {
        select_candidates(candidates, partial, scope, TEST_MAX_CHOICES)
            .into_iter()
            .map(|(_, value)| value)
            .collect()
    }

    #[test]
    fn scopes_filter_states_and_keep_running_candidates_first() {
        init_test_config();
        let candidates = sample_candidates();

        assert_eq!(
            values(&candidates, "", BotNameScope::RunningOnly),
            ["duck"].map(String::from)
        );
        // 排队中的机器人不属于“已停止”；`/start` 等指令会在入队前拒绝它。
        assert_eq!(
            values(&candidates, "", BotNameScope::StoppedOnly),
            ["cup", "duck2"].map(String::from)
        );
        // 排序优先级：运行中、排队中、未运行且未排队。
        assert_eq!(
            values(&candidates, "", BotNameScope::AnyState),
            ["duck", "red", "cup", "duck2"].map(String::from)
        );
    }

    #[test]
    fn prefix_matching_is_case_insensitive_and_empty_prefix_matches_all() {
        init_test_config();
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
        init_test_config();
        let pairs = select_candidates(
            &sample_candidates(),
            "",
            BotNameScope::AnyState,
            TEST_MAX_CHOICES,
        );
        let i18n_text = crate::config::Config::get().text();
        assert_eq!(
            pairs[0],
            (
                format!("duck{}", i18n_text.bot_candidate_running_note()),
                "duck".to_string()
            )
        );
        assert_eq!(
            pairs[1],
            (
                format!("red{}", i18n_text.bot_candidate_queued_note()),
                "red".to_string()
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
        init_test_config();
        let many: Vec<(String, bool, bool)> = (0..TEST_MAX_CHOICES + 10)
            .map(|index| (format!("bot{index:02}"), index % 3 == 0, index % 3 == 1))
            .collect();
        assert_eq!(
            select_candidates(&many, "", BotNameScope::AnyState, TEST_MAX_CHOICES).len(),
            TEST_MAX_CHOICES
        );
    }
}
