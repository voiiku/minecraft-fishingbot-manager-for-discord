//! 日志级别判定：本项目与第三方依赖两档上限（纯函数，无 IO）。
//!
//! 分档只看日志 target 前缀：本 crate 的 lib 与 bin 用同一个前缀，其余都算第三方。

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

    /// 显式给开关的构造，仅供测试（生产路径走 [`LogLevels::from_config`]）。
    ///
    /// `false`（默认）：本项目自己的记录 `INFO` 起，第三方 crate 只放到 `WARN` —— 它们按心跳节奏打
    /// `INFO`（`recv; do_heartbeat;`），会把本项目的关键几行淹没。
    /// `true`：两档都放到 `DEBUG`（打开 debug 就是全都要）。
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

    /// 默认（`[log] debug = false`）：本项目自己的记录按配置放，第三方 crate 只放到 `WARN`
    /// （否则 serenity 的 `recv; do_heartbeat;` 会把本项目的关键几行淹没）。
    #[test]
    fn third_party_records_are_capped_to_warn_by_default() {
        let levels = LogLevels::new(false);

        assert_eq!(levels.own, LevelFilter::Info);
        assert_eq!(levels.dependencies, LevelFilter::Warn);

        // "是不是自己的记录"只看 target 前缀：lib 与 bin 都是 `minecraft_fishingbot_manager_for_discord`。
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

    /// `debug = true` 时两档都放到 `DEBUG`：本项目自己的记录 + 第三方 crate 的
    ///（"打开 debug 就是全都要"）。
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
