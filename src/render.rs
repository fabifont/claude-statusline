use crate::format::{apply_color, format_duration, format_pct};
use crate::models::{
    ClockTime, Config, ItemConfig, ItemKind, PeakHours, RateLimitWindow, StatusInput,
};
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 120;
const COMMAND_POLL_INTERVAL_MS: u64 = 10;

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
        ItemKind::Command => render_external_command_item(item),
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

fn render_external_command_item(item: &ItemConfig) -> Option<String> {
    let command = item.command.as_deref()?.trim();
    if command.is_empty() {
        return None;
    }

    let timeout_ms = item.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS).max(1);
    let output = run_external_command(command, &item.args, timeout_ms)?;

    let label = item.label.as_deref().map(str::trim).unwrap_or("");
    if label.is_empty() {
        Some(output)
    } else {
        Some(format!("{label} {output}"))
    }
}

fn run_external_command(command: &str, args: &[String], timeout_ms: u64) -> Option<String> {
    let deadline = Instant::now().checked_add(Duration::from_millis(timeout_ms))?;
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let mut stdout_reader = Some(thread::spawn(move || {
        let mut stdout = stdout;
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }));

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_reader.take()?.join().ok().flatten()?;
                if !status.success() {
                    return None;
                }

                return normalize_external_output(&String::from_utf8_lossy(&stdout));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Some(reader) = stdout_reader.take() {
                        let _ = reader.join();
                    }
                    return None;
                }
                thread::sleep(Duration::from_millis(COMMAND_POLL_INTERVAL_MS));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(reader) = stdout_reader.take() {
                    let _ = reader.join();
                }
                return None;
            }
        }
    }
}

fn normalize_external_output(raw: &str) -> Option<String> {
    let collapsed = raw
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
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
