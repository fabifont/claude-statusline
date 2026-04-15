use crate::cli::PreviewMode;
use crate::config::{
    ConfigLoadOutcome, ConfigSource, load_config_outcome, parse_timezone_or_default,
    validate_config,
};
use crate::error::StatuslineError;
use crate::format::{apply_color, supported_color_names};
use crate::models::{ContextWindow, Cost, Model, RateLimitWindow, RateLimits, StatusInput};
use crate::render::build_status_line;
use chrono::{TimeZone, Utc};
use std::fmt::Write;
use std::time::{Duration, UNIX_EPOCH};

/// Execute a preview mode and return user-facing output.
pub fn run_preview(mode: PreviewMode) -> Result<String, StatuslineError> {
    let outcome = load_config_outcome();

    match mode {
        PreviewMode::Sample => Ok(render_sample(&outcome)),
        PreviewMode::Validate => render_validation(&outcome),
        PreviewMode::Config => Ok(render_config(&outcome)),
        PreviewMode::Explain => Ok(render_explain()),
        PreviewMode::Colors => Ok(render_colors()),
    }
}

fn render_sample(outcome: &ConfigLoadOutcome) -> String {
    let input = sample_input();
    let tz = parse_timezone_or_default(&outcome.config.timezone);
    let now_utc = Utc
        .with_ymd_and_hms(2024, 1, 1, 14, 30, 0)
        .single()
        .expect("static timestamp must be valid");
    let now_system = UNIX_EPOCH + Duration::from_secs(10_000);
    let line = build_status_line(&input, &outcome.config, tz, now_utc, now_system);

    let mut out = String::new();
    let _ = writeln!(out, "Sample output:");
    let _ = writeln!(out, "{line}");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Resolved config: {} ({})",
        outcome.resolved_path.display(),
        source_label(outcome.source)
    );

    if !outcome.warnings.is_empty() {
        let _ = writeln!(out, "Warnings:");
        for warning in &outcome.warnings {
            let _ = writeln!(out, "- {warning}");
        }
    }

    out.trim_end().to_string()
}

fn render_validation(outcome: &ConfigLoadOutcome) -> Result<String, StatuslineError> {
    let report = validate_config(outcome);
    let mut out = String::new();

    let _ = writeln!(out, "Config validation report");
    let _ = writeln!(
        out,
        "Path: {} ({})",
        outcome.resolved_path.display(),
        source_label(outcome.source)
    );
    let _ = writeln!(
        out,
        "File exists: {}",
        if outcome.file_exists { "yes" } else { "no" }
    );

    if report.errors.is_empty() {
        let _ = writeln!(out, "Status: valid");
    } else {
        let _ = writeln!(out, "Status: invalid");
        let _ = writeln!(out, "Errors:");
        for error in &report.errors {
            let _ = writeln!(out, "- {error}");
        }
    }

    if !report.warnings.is_empty() {
        let _ = writeln!(out, "Warnings:");
        for warning in &report.warnings {
            let _ = writeln!(out, "- {warning}");
        }
    }

    let output = out.trim_end().to_string();
    if report.is_valid() {
        Ok(output)
    } else {
        Err(StatuslineError::ValidationFailed(output))
    }
}

fn render_config(outcome: &ConfigLoadOutcome) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Resolved config");
    let _ = writeln!(out, "Path: {}", outcome.resolved_path.display());
    let _ = writeln!(out, "Source: {}", source_label(outcome.source));
    let _ = writeln!(
        out,
        "File exists: {}",
        if outcome.file_exists { "yes" } else { "no" }
    );
    let _ = writeln!(out, "Timezone: {}", outcome.config.timezone);
    let _ = writeln!(
        out,
        "Peak window: {} -> {}",
        outcome.config.peak_hours.start_hour, outcome.config.peak_hours.end_hour
    );
    let _ = writeln!(out, "Colors enabled: {}", outcome.config.colors_enabled);
    let _ = writeln!(out, "Items:");

    for (index, item) in outcome.config.items.iter().enumerate() {
        let label = item.label.as_deref().unwrap_or("<default>");
        let color = item.color.as_deref().unwrap_or("<none>");
        let _ = writeln!(
            out,
            "- [{index}] {:?} label={label} color={color} enabled={}",
            item.kind, item.enabled
        );
    }

    if !outcome.warnings.is_empty() {
        let _ = writeln!(out, "Warnings:");
        for warning in &outcome.warnings {
            let _ = writeln!(out, "- {warning}");
        }
    }

    out.trim_end().to_string()
}

fn render_explain() -> String {
    [
        "Supported item kinds:",
        "- five_hour: 5-hour rate limit percentage and reset duration",
        "- seven_day: 7-day rate limit percentage and reset duration",
        "- limits_age: age of cached fallback rate-limit data",
        "- context: context window used percentage",
        "- peak: indicator and remaining time inside configured peak window",
        "- model: active model display name or id",
        "- cost: session cost in USD",
        "- command: run an external command and render its stdout",
        "",
        "Config keys:",
        "- separator: text used between items",
        "- timezone: IANA timezone name for local peak-time logic",
        "- colors_enabled: toggles ANSI colors on or off",
        "- peak_hours.start_hour and peak_hours.end_hour: 0..=23",
        "- items: ordered list of rendered fields",
        "- command item fields: command, args (array), timeout_ms",
    ]
    .join("\n")
}

fn render_colors() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Color preview");
    for color in supported_color_names() {
        let rendered = apply_color(color, Some(color), true);
        let _ = writeln!(out, "- {rendered}");
    }
    out.trim_end().to_string()
}

fn source_label(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::EnvVar => "CLAUDE_STATUSLINE_CONFIG",
        ConfigSource::ClaudeHome => "~/.claude/statusline.toml",
        ConfigSource::Xdg => "~/.config/claude-statusline/config.toml",
        ConfigSource::Fallback => "fallback path",
    }
}

fn sample_input() -> StatusInput {
    StatusInput {
        model: Some(Model {
            id: Some("claude-3-7-sonnet".to_string()),
            display_name: Some("Claude 3.7".to_string()),
        }),
        context_window: Some(ContextWindow {
            used_percentage: Some(12.34),
        }),
        rate_limits: Some(RateLimits {
            five_hour: Some(RateLimitWindow {
                used_percentage: Some(50.0),
                resets_at: Some(13_660),
            }),
            seven_day: Some(RateLimitWindow {
                used_percentage: Some(80.5),
                resets_at: Some(186_400),
            }),
        }),
        cost: Some(Cost {
            total_cost_usd: Some(12.34),
        }),
        rate_limits_cache_age: None,
    }
}
