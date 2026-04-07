use crate::format::{apply_color, format_duration, format_pct};
use crate::models::{
    ClockTime, Config, ItemConfig, ItemKind, PeakHours, RateLimitWindow, StatusInput,
};
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct RenderContext<'a> {
    peak_hours: &'a PeakHours,
    colors_enabled: bool,
    local_time: ClockTime,
    now_system: SystemTime,
}

/// Parse the Claude status payload; invalid JSON yields an empty default payload.
pub fn parse_input(raw: &str) -> StatusInput {
    serde_json::from_str(raw).unwrap_or_default()
}

/// Build the rendered status line according to config item order.
pub fn build_status_line(
    input: &StatusInput,
    config: &Config,
    tz: Tz,
    now_utc: DateTime<Utc>,
    now_system: SystemTime,
) -> String {
    let now_local = now_utc.with_timezone(&tz);
    let ctx = RenderContext {
        peak_hours: &config.peak_hours,
        colors_enabled: config.colors_enabled,
        local_time: ClockTime::from_hms(now_local.hour(), now_local.minute(), now_local.second()),
        now_system,
    };

    let pieces: Vec<String> = config
        .items
        .iter()
        .filter(|item| item.enabled)
        .filter_map(|item| render_item(item, input, &ctx))
        .collect();

    pieces.join(&config.separator)
}

fn render_item(item: &ItemConfig, input: &StatusInput, ctx: &RenderContext<'_>) -> Option<String> {
    let rendered = match item.kind {
        ItemKind::FiveHour => {
            let label = item.label.as_deref().unwrap_or("5h");
            Some(
                input
                    .rate_limits
                    .as_ref()
                    .and_then(|limits| limits.five_hour.as_ref())
                    .and_then(|window| render_rate_limit(window, label, ctx.now_system))
                    .unwrap_or_else(|| format!("{label} --")),
            )
        }
        ItemKind::SevenDay => {
            let label = item.label.as_deref().unwrap_or("7d");
            Some(
                input
                    .rate_limits
                    .as_ref()
                    .and_then(|limits| limits.seven_day.as_ref())
                    .and_then(|window| render_rate_limit(window, label, ctx.now_system))
                    .unwrap_or_else(|| format!("{label} --")),
            )
        }
        ItemKind::Context => {
            let pct = valid_f64(input.context_window.as_ref()?.used_percentage?)?;
            Some(format!(
                "{} {}",
                item.label.as_deref().unwrap_or("ctx"),
                format_pct(pct)
            ))
        }
        ItemKind::Peak => render_peak(item.label.as_deref().unwrap_or("🔥"), ctx),
        ItemKind::Model => {
            let model = input.model.as_ref()?;
            let name = model
                .display_name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .or_else(|| model.id.as_deref().filter(|name| !name.trim().is_empty()))?;
            Some(name.to_owned())
        }
        ItemKind::Cost => {
            let cost = valid_f64(input.cost.as_ref()?.total_cost_usd?)?;
            let label = item.label.as_deref().unwrap_or("$");
            if label == "$" {
                Some(format!("${cost:.2}"))
            } else {
                Some(format!("{label} ${cost:.2}"))
            }
        }
    }?;

    Some(apply_color(
        &rendered,
        item.color.as_deref(),
        ctx.colors_enabled,
    ))
}

/// Render a rate-limit item.
pub fn render_rate_limit(
    window: &RateLimitWindow,
    label: &str,
    now_system: SystemTime,
) -> Option<String> {
    let pct = valid_f64(window.used_percentage?)?;
    let reset = window.resets_at?;
    let remaining = seconds_until(reset, now_system)?;
    Some(format!(
        "{label} {} {}",
        format_pct(pct),
        format_duration(remaining)
    ))
}

fn render_peak(label: &str, ctx: &RenderContext<'_>) -> Option<String> {
    let remaining = ctx.peak_hours.remaining_until_window_end(ctx.local_time)?;
    Some(format!("{label} {}", format_duration(remaining)))
}

fn seconds_until(reset_epoch_seconds: i64, now_system: SystemTime) -> Option<Duration> {
    let reset = UNIX_EPOCH.checked_add(Duration::from_secs(reset_epoch_seconds.max(0) as u64))?;
    match reset.duration_since(now_system) {
        Ok(duration) => Some(duration),
        Err(_) => Some(Duration::from_secs(0)),
    }
}

fn valid_f64(value: f64) -> Option<f64> {
    if value.is_finite() { Some(value) } else { None }
}
