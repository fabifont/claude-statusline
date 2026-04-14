use serde::Deserialize;
use std::time::Duration;

const SECONDS_PER_DAY: u32 = 24 * 60 * 60;

/// Input payload received from Claude Code over stdin.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct StatusInput {
    pub model: Option<Model>,
    pub context_window: Option<ContextWindow>,
    pub rate_limits: Option<RateLimits>,
    pub cost: Option<Cost>,
}

/// Active model metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct Model {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

/// Context window utilization.
#[derive(Debug, Deserialize, Clone)]
pub struct ContextWindow {
    pub used_percentage: Option<f64>,
}

/// Session cost information.
#[derive(Debug, Deserialize, Clone)]
pub struct Cost {
    pub total_cost_usd: Option<f64>,
}

/// Rate limit windows exposed by the Claude payload.
#[derive(Debug, Deserialize, Clone)]
pub struct RateLimits {
    pub five_hour: Option<RateLimitWindow>,
    pub seven_day: Option<RateLimitWindow>,
}

/// Details for a single rate limit window.
#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitWindow {
    pub used_percentage: Option<f64>,
    pub resets_at: Option<i64>,
}

/// Top-level user configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_true")]
    pub colors_enabled: bool,
    #[serde(default)]
    pub peak_hours: PeakHours,
    #[serde(default = "default_items")]
    pub items: Vec<ItemConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            separator: default_separator(),
            timezone: default_timezone(),
            colors_enabled: default_true(),
            peak_hours: PeakHours::default(),
            items: default_items(),
        }
    }
}

/// Configurable item in the rendered status line.
#[derive(Debug, Deserialize, Clone)]
pub struct ItemConfig {
    pub kind: ItemKind,
    pub label: Option<String>,
    pub color: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
}

/// Supported status line item kinds.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    FiveHour,
    SevenDay,
    Context,
    Peak,
    Model,
    Cost,
    Command,
}

/// Peak-hour configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct PeakHours {
    #[serde(default = "default_peak_start")]
    pub start_hour: u32,
    #[serde(default = "default_peak_end")]
    pub end_hour: u32,
}

impl PeakHours {
    /// Return remaining duration until peak window end for the provided local clock time.
    pub fn remaining_until_window_end(&self, local_time: ClockTime) -> Option<Duration> {
        let start_secs = self.start_hour.checked_mul(3600)?;
        let end_secs = self.end_hour.checked_mul(3600)?;
        if start_secs >= SECONDS_PER_DAY || end_secs >= SECONDS_PER_DAY || start_secs == end_secs {
            return None;
        }

        let now_secs = local_time.seconds_since_midnight();
        let remaining_secs = if start_secs < end_secs {
            if now_secs < start_secs || now_secs >= end_secs {
                return None;
            }
            end_secs - now_secs
        } else if now_secs >= start_secs {
            (SECONDS_PER_DAY - now_secs) + end_secs
        } else if now_secs < end_secs {
            end_secs - now_secs
        } else {
            return None;
        };

        Some(Duration::from_secs(u64::from(remaining_secs)))
    }
}

impl Default for PeakHours {
    fn default() -> Self {
        Self {
            start_hour: default_peak_start(),
            end_hour: default_peak_end(),
        }
    }
}

/// Lightweight local-time representation used for deterministic rendering and tests.
#[derive(Clone, Copy)]
pub struct ClockTime {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl ClockTime {
    /// Build a clock time from its components.
    pub fn from_hms(hour: u32, minute: u32, second: u32) -> Self {
        Self {
            hour,
            minute,
            second,
        }
    }

    /// Return seconds elapsed since midnight.
    pub fn seconds_since_midnight(self) -> u32 {
        self.hour * 3600 + self.minute * 60 + self.second
    }
}

pub fn default_separator() -> String {
    " | ".to_string()
}

pub fn default_timezone() -> String {
    "Europe/Rome".to_string()
}

pub fn default_peak_start() -> u32 {
    13
}

pub fn default_peak_end() -> u32 {
    19
}

pub fn default_true() -> bool {
    true
}

pub fn default_items() -> Vec<ItemConfig> {
    vec![
        ItemConfig {
            kind: ItemKind::FiveHour,
            label: Some("5h".into()),
            color: Some("cyan".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
        ItemConfig {
            kind: ItemKind::SevenDay,
            label: Some("7d".into()),
            color: Some("blue".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
        ItemConfig {
            kind: ItemKind::Context,
            label: Some("ctx".into()),
            color: Some("magenta".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
        ItemConfig {
            kind: ItemKind::Peak,
            label: Some("🔥".into()),
            color: Some("red".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
        ItemConfig {
            kind: ItemKind::Model,
            label: None,
            color: Some("green".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
        ItemConfig {
            kind: ItemKind::Cost,
            label: Some("$".into()),
            color: Some("yellow".into()),
            enabled: true,
            command: None,
            args: Vec::new(),
            timeout_ms: None,
        },
    ]
}
