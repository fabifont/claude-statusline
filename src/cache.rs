use crate::models::{RateLimitWindow, RateLimits, RateLimitsCacheAge, StatusInput};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_PATH_ENV: &str = "CLAUDE_STATUSLINE_CACHE_PATH";
const CACHE_DIR: &str = ".cache/claude-statusline";
const CACHE_FILE_NAME: &str = "rate_limits.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct RateLimitsCache {
    five_hour: Option<CachedWindow>,
    seven_day: Option<CachedWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedWindow {
    used_percentage: f64,
    resets_at: i64,
    #[serde(default)]
    cached_at: Option<i64>,
}

pub fn hydrate_missing_rate_limits(input: &mut StatusInput, now_system: SystemTime) {
    input.rate_limits_cache_age = None;

    let Some(cache_path) = resolve_cache_path() else {
        return;
    };

    hydrate_missing_rate_limits_at_path(input, now_system, &cache_path);
}

pub fn persist_rate_limits(input: &StatusInput, now_system: SystemTime) {
    let Some(cache_path) = resolve_cache_path() else {
        return;
    };

    persist_rate_limits_at_path(input, now_system, &cache_path);
}

fn hydrate_missing_rate_limits_at_path(
    input: &mut StatusInput,
    now_system: SystemTime,
    cache_path: &Path,
) {
    let Some(now_epoch) = system_time_to_epoch_seconds(now_system) else {
        return;
    };

    let Some(cache) = load_cache(cache_path, now_epoch) else {
        return;
    };

    let fill_five = input
        .rate_limits
        .as_ref()
        .and_then(|limits| limits.five_hour.as_ref())
        .is_none_or(|window| !window_has_renderable_values(window));
    let fill_seven = input
        .rate_limits
        .as_ref()
        .and_then(|limits| limits.seven_day.as_ref())
        .is_none_or(|window| !window_has_renderable_values(window));

    if !fill_five && !fill_seven {
        return;
    }

    let mut did_fill = false;
    let mut cache_age = RateLimitsCacheAge::default();
    let limits = input.rate_limits.get_or_insert_with(empty_rate_limits);

    if fill_five && let Some(window) = cache.five_hour {
        cache_age.five_hour_seconds = window.age_seconds(now_epoch);
        limits.five_hour = Some(window.into_window());
        did_fill = true;
    }

    if fill_seven && let Some(window) = cache.seven_day {
        cache_age.seven_day_seconds = window.age_seconds(now_epoch);
        limits.seven_day = Some(window.into_window());
        did_fill = true;
    }

    if did_fill {
        input.rate_limits_cache_age = Some(cache_age);
    }

    if !did_fill && limits.five_hour.is_none() && limits.seven_day.is_none() {
        input.rate_limits = None;
    }
}

fn persist_rate_limits_at_path(input: &StatusInput, now_system: SystemTime, cache_path: &Path) {
    let Some(now_epoch) = system_time_to_epoch_seconds(now_system) else {
        return;
    };

    let mut cache = load_cache(cache_path, now_epoch).unwrap_or_default();
    let mut did_update = false;

    if let Some(window) = input
        .rate_limits
        .as_ref()
        .and_then(|limits| limits.five_hour.as_ref())
        .and_then(|window| CachedWindow::from_input_window(window, now_epoch))
    {
        cache.five_hour = Some(window);
        did_update = true;
    }

    if let Some(window) = input
        .rate_limits
        .as_ref()
        .and_then(|limits| limits.seven_day.as_ref())
        .and_then(|window| CachedWindow::from_input_window(window, now_epoch))
    {
        cache.seven_day = Some(window);
        did_update = true;
    }

    if !did_update || (cache.five_hour.is_none() && cache.seven_day.is_none()) {
        return;
    }

    if let Some(parent) = cache_path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }

    let Ok(raw) = serde_json::to_string(&cache) else {
        return;
    };

    let _ = fs::write(cache_path, raw);
}

fn load_cache(cache_path: &Path, now_epoch: i64) -> Option<RateLimitsCache> {
    let raw = fs::read_to_string(cache_path).ok()?;
    let mut parsed = serde_json::from_str::<RateLimitsCache>(&raw).ok()?;

    if parsed
        .five_hour
        .as_ref()
        .is_some_and(|window| !window.is_usable(now_epoch))
    {
        parsed.five_hour = None;
    }

    if parsed
        .seven_day
        .as_ref()
        .is_some_and(|window| !window.is_usable(now_epoch))
    {
        parsed.seven_day = None;
    }

    if parsed.five_hour.is_none() && parsed.seven_day.is_none() {
        None
    } else {
        Some(parsed)
    }
}

fn resolve_cache_path() -> Option<PathBuf> {
    if let Ok(path) = env::var(CACHE_PATH_ENV)
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join(CACHE_DIR).join(CACHE_FILE_NAME))
}

fn empty_rate_limits() -> RateLimits {
    RateLimits {
        five_hour: None,
        seven_day: None,
    }
}

fn window_has_renderable_values(window: &RateLimitWindow) -> bool {
    window.used_percentage.is_some_and(|pct| pct.is_finite()) && window.resets_at.is_some()
}

fn system_time_to_epoch_seconds(now: SystemTime) -> Option<i64> {
    let duration = now.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_secs()).ok()
}

impl CachedWindow {
    fn from_input_window(window: &RateLimitWindow, now_epoch: i64) -> Option<Self> {
        let used_percentage = window.used_percentage?;
        let resets_at = window.resets_at?;

        if !used_percentage.is_finite() || resets_at <= now_epoch {
            return None;
        }

        Some(Self {
            used_percentage,
            resets_at,
            cached_at: Some(now_epoch),
        })
    }

    fn is_usable(&self, now_epoch: i64) -> bool {
        self.used_percentage.is_finite() && self.resets_at > now_epoch
    }

    fn into_window(self) -> RateLimitWindow {
        RateLimitWindow {
            used_percentage: Some(self.used_percentage),
            resets_at: Some(self.resets_at),
        }
    }

    fn age_seconds(&self, now_epoch: i64) -> Option<u64> {
        let cached_at = self.cached_at?;
        if cached_at > now_epoch {
            return Some(0);
        }
        u64::try_from(now_epoch - cached_at).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn now_system() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(10_000)
    }

    fn unique_temp_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "claude-statusline-cache-test-{}-{}-{suffix}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn write_cache_file(path: &Path, cache: &RateLimitsCache) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        let raw = serde_json::to_string(cache).expect("serialize cache");
        std::fs::write(path, raw).expect("write cache file");
    }

    #[test]
    fn hydrate_uses_cache_for_missing_windows() {
        let path = unique_temp_path("hydrate-missing");
        let now = now_system();

        write_cache_file(
            &path,
            &RateLimitsCache {
                five_hour: Some(CachedWindow {
                    used_percentage: 42.0,
                    resets_at: 11_000,
                    cached_at: Some(9_900),
                }),
                seven_day: Some(CachedWindow {
                    used_percentage: 64.0,
                    resets_at: 80_000,
                    cached_at: Some(9_700),
                }),
            },
        );

        let mut input = StatusInput::default();
        hydrate_missing_rate_limits_at_path(&mut input, now, &path);

        let limits = input.rate_limits.expect("limits should be populated");
        assert_eq!(
            limits
                .five_hour
                .expect("five hour")
                .used_percentage
                .expect("pct"),
            42.0
        );
        assert_eq!(
            limits
                .seven_day
                .expect("seven day")
                .used_percentage
                .expect("pct"),
            64.0
        );

        let age = input
            .rate_limits_cache_age
            .expect("cache age should be populated for hydrated windows");
        assert_eq!(age.five_hour_seconds, Some(100));
        assert_eq!(age.seven_day_seconds, Some(300));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn hydrate_keeps_live_values_and_only_fills_missing() {
        let path = unique_temp_path("hydrate-partial");
        let now = now_system();

        write_cache_file(
            &path,
            &RateLimitsCache {
                five_hour: Some(CachedWindow {
                    used_percentage: 42.0,
                    resets_at: 11_000,
                    cached_at: Some(9_900),
                }),
                seven_day: Some(CachedWindow {
                    used_percentage: 64.0,
                    resets_at: 80_000,
                    cached_at: Some(9_850),
                }),
            },
        );

        let mut input = StatusInput {
            rate_limits: Some(RateLimits {
                five_hour: Some(RateLimitWindow {
                    used_percentage: Some(12.0),
                    resets_at: Some(12_000),
                }),
                seven_day: None,
            }),
            ..StatusInput::default()
        };

        hydrate_missing_rate_limits_at_path(&mut input, now, &path);

        let limits = input.rate_limits.expect("limits should exist");
        assert_eq!(
            limits
                .five_hour
                .expect("five hour")
                .used_percentage
                .expect("pct"),
            12.0
        );
        assert_eq!(
            limits
                .seven_day
                .expect("seven day")
                .used_percentage
                .expect("pct"),
            64.0
        );

        let age = input
            .rate_limits_cache_age
            .expect("cache age should be populated when cache fills at least one window");
        assert_eq!(age.five_hour_seconds, None);
        assert_eq!(age.seven_day_seconds, Some(150));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn hydrate_ignores_expired_cached_windows() {
        let path = unique_temp_path("hydrate-expired");
        let now = now_system();

        write_cache_file(
            &path,
            &RateLimitsCache {
                five_hour: Some(CachedWindow {
                    used_percentage: 42.0,
                    resets_at: 9_999,
                    cached_at: Some(9_900),
                }),
                seven_day: Some(CachedWindow {
                    used_percentage: 64.0,
                    resets_at: 80_000,
                    cached_at: Some(9_820),
                }),
            },
        );

        let mut input = StatusInput::default();
        hydrate_missing_rate_limits_at_path(&mut input, now, &path);

        let limits = input.rate_limits.expect("limits should be present");
        assert!(limits.five_hour.is_none());
        assert_eq!(
            limits
                .seven_day
                .expect("seven day")
                .used_percentage
                .expect("pct"),
            64.0
        );

        let age = input
            .rate_limits_cache_age
            .expect("cache age should be populated for hydrated seven-day window");
        assert_eq!(age.five_hour_seconds, None);
        assert_eq!(age.seven_day_seconds, Some(180));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persist_updates_only_windows_present_in_input() {
        let path = unique_temp_path("persist-partial");
        let now = now_system();

        let first = StatusInput {
            rate_limits: Some(RateLimits {
                five_hour: Some(RateLimitWindow {
                    used_percentage: Some(25.0),
                    resets_at: Some(12_000),
                }),
                seven_day: None,
            }),
            ..StatusInput::default()
        };

        persist_rate_limits_at_path(&first, now, &path);

        let second = StatusInput {
            rate_limits: Some(RateLimits {
                five_hour: None,
                seven_day: Some(RateLimitWindow {
                    used_percentage: Some(66.0),
                    resets_at: Some(70_000),
                }),
            }),
            ..StatusInput::default()
        };

        persist_rate_limits_at_path(&second, now, &path);

        let raw = std::fs::read_to_string(&path).expect("cache exists");
        let cached: RateLimitsCache = serde_json::from_str(&raw).expect("valid cache");

        assert_eq!(cached.five_hour.expect("five hour").used_percentage, 25.0);
        assert_eq!(cached.seven_day.expect("seven day").used_percentage, 66.0);

        let _ = std::fs::remove_file(path);
    }
}
