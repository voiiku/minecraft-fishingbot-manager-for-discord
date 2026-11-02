//! 按日志 target 前缀区分应用和依赖的级别上限。

use log::LevelFilter;

use crate::config;

/// 本 crate 的日志 target 前缀。
const OWN_TARGET_PREFIX: &str = env!("CARGO_CRATE_NAME");

/// 本项目与第三方依赖的日志级别上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LogLevels {
    /// 本项目（`minecraft_fishingbot_manager_for_discord`）记录的上限。
    pub(super) own: LevelFilter,
    /// 第三方 crate 记录的上限。
    pub(super) dependencies: LevelFilter,
}

impl LogLevels {
    /// 按配置里的 `[log] debug` 算两档上限。
    pub(super) fn from_config() -> Self {
        Self::new(config::Config::get().log.debug)
    }

    /// `debug` 为 false 时应用从 INFO、依赖从 WARN 开始记录；为 true 时均从 DEBUG 开始。
    pub(super) fn new(debug: bool) -> Self {
        if debug {
            Self {
                own: LevelFilter::Debug,
                dependencies: LevelFilter::Debug,
            }
        } else {
            Self {
                own: LevelFilter::Info,
                dependencies: LevelFilter::Warn,
            }
        }
    }

    /// 根据日志 target 选择级别上限。
    pub(super) fn ceiling_for(&self, target: &str) -> LevelFilter {
        if target.starts_with(OWN_TARGET_PREFIX) {
            self.own
        } else {
            self.dependencies
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认配置下，本项目记录从 `INFO` 开始，第三方 crate 仅记录 `WARN` 及以上。
    /// 这可以避免 serenity 的心跳日志掩盖本项目的关键记录。
    #[test]
    fn third_party_records_are_capped_to_warn_by_default() {
        let levels = LogLevels::new(false);

        assert_eq!(levels.own, LevelFilter::Info);
        assert_eq!(levels.dependencies, LevelFilter::Warn);

        // 项目记录通过 target 前缀识别：lib 与 bin 共用同一个 crate 前缀。
        assert_eq!(
            levels.ceiling_for("minecraft_fishingbot_manager_for_discord"),
            LevelFilter::Info
        );
        assert_eq!(
            levels.ceiling_for("minecraft_fishingbot_manager_for_discord::utils::bot_status"),
            LevelFilter::Info
        );
        assert_eq!(
            levels.ceiling_for("serenity::gateway::shard"),
            LevelFilter::Warn
        );
        assert_eq!(levels.ceiling_for("poise::framework"), LevelFilter::Warn);
    }

    /// `debug = true` 时两档上限都提升到 `DEBUG`。
    #[test]
    fn debug_widens_both_tiers() {
        let levels = LogLevels::new(true);

        assert_eq!(levels.own, LevelFilter::Debug);
        assert_eq!(levels.dependencies, LevelFilter::Debug);
        assert_eq!(
            levels.ceiling_for("serenity::gateway::shard"),
            LevelFilter::Debug
        );
        assert_eq!(levels.ceiling_for("poise::framework"), LevelFilter::Debug);
    }
}
