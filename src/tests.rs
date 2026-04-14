use super::*;
use chrono::TimeZone;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn item(kind: ItemKind, label: Option<&str>, color: Option<&str>) -> ItemConfig {
    ItemConfig {
        kind,
        label: label.map(ToString::to_string),
        color: color.map(ToString::to_string),
        enabled: true,
        command: None,
        args: Vec::new(),
        timeout_ms: None,
    }
}

fn command_item(command: &str, args: &[&str]) -> ItemConfig {
    ItemConfig {
        kind: ItemKind::Command,
        label: None,
        color: None,
        enabled: true,
        command: Some(command.to_string()),
        args: args.iter().map(ToString::to_string).collect(),
        timeout_ms: Some(200),
    }
}

fn command_item_with_timeout(command: &str, args: &[&str], timeout_ms: u64) -> ItemConfig {
    let mut item = command_item(command, args);
    item.timeout_ms = Some(timeout_ms);
    item
}

fn fixed_now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc
        .with_ymd_and_hms(2024, 1, 1, 14, 30, 0)
        .single()
        .expect("valid timestamp")
}

fn fixed_now_system() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(10_000)
}

#[test]
fn format_duration_uses_compact_units() {
    assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    assert_eq!(format_duration(Duration::from_secs(59 * 60)), "59m");
    assert_eq!(
        format_duration(Duration::from_secs(2 * 3600 + 30 * 60)),
        "2h 30m"
    );
    assert_eq!(format_duration(Duration::from_secs(26 * 3600)), "1d 2h");
}

#[test]
fn peak_hours_regular_window() {
    let peak = PeakHours {
        start_hour: 13,
        end_hour: 19,
    };
    let remaining = peak
        .remaining_until_window_end(ClockTime::from_hms(14, 30, 0))
        .expect("should be in peak window");

    assert_eq!(format_duration(remaining), "4h 30m");
    assert!(
        peak.remaining_until_window_end(ClockTime::from_hms(19, 0, 0))
            .is_none()
    );
}

#[test]
fn peak_hours_cross_midnight_window() {
    let peak = PeakHours {
        start_hour: 22,
        end_hour: 2,
    };

    let remaining_late = peak
        .remaining_until_window_end(ClockTime::from_hms(23, 30, 0))
        .expect("should be in peak window");
    let remaining_early = peak
        .remaining_until_window_end(ClockTime::from_hms(1, 0, 0))
        .expect("should be in peak window");

    assert_eq!(format_duration(remaining_late), "2h 30m");
    assert_eq!(format_duration(remaining_early), "1h 0m");
    assert!(
        peak.remaining_until_window_end(ClockTime::from_hms(12, 0, 0))
            .is_none()
    );
}

#[test]
fn config_parsing_preserves_item_order_and_color_toggle() {
    let raw = r#"
separator = " / "
colors_enabled = false

[[items]]
kind = "model"

[[items]]
kind = "context"
"#;

    let cfg: Config = toml::from_str(raw).expect("valid config");

    assert_eq!(cfg.separator, " / ");
    assert_eq!(cfg.timezone, "Europe/Rome");
    assert!(!cfg.colors_enabled);
    assert_eq!(cfg.items.len(), 2);
    assert_eq!(cfg.items[0].kind, ItemKind::Model);
    assert_eq!(cfg.items[1].kind, ItemKind::Context);
}

#[test]
fn config_parsing_supports_command_item_fields() {
    let raw = r#"
[[items]]
kind = "command"
command = "printf"
args = ["[CAVEMAN]"]
timeout_ms = 150
"#;

    let cfg: Config = toml::from_str(raw).expect("valid config");

    assert_eq!(cfg.items.len(), 1);
    let item = &cfg.items[0];
    assert_eq!(item.kind, ItemKind::Command);
    assert_eq!(item.command.as_deref(), Some("printf"));
    assert_eq!(item.args, vec!["[CAVEMAN]".to_string()]);
    assert_eq!(item.timeout_ms, Some(150));
}

#[test]
fn parse_input_tolerates_invalid_json() {
    let parsed = parse_input("this is not json");
    assert!(parsed.model.is_none());
    assert!(parsed.context_window.is_none());
    assert!(parsed.rate_limits.is_none());
    assert!(parsed.cost.is_none());
}

#[test]
fn missing_fields_show_rate_limit_placeholders() {
    let input = parse_input(r#"{"context_window":{"used_percentage":42.0}}"#);
    let config = Config {
        separator: " | ".to_string(),
        timezone: "Europe/Rome".to_string(),
        colors_enabled: true,
        peak_hours: PeakHours::default(),
        items: vec![
            item(ItemKind::FiveHour, Some("5h"), None),
            item(ItemKind::SevenDay, Some("7d"), None),
            item(ItemKind::Context, Some("ctx"), None),
            item(ItemKind::Cost, Some("$"), None),
        ],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::Europe::Rome,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(line, "5h -- | 7d -- | ctx 42%");
}

#[test]
fn model_and_cost_require_real_values() {
    let input = parse_input(r#"{"model":{},"cost":{}}"#);
    let config = Config {
        separator: " | ".to_string(),
        timezone: "Europe/Rome".to_string(),
        colors_enabled: true,
        peak_hours: PeakHours::default(),
        items: vec![
            item(ItemKind::Model, None, Some("green")),
            item(ItemKind::Cost, Some("$"), Some("yellow")),
        ],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::Europe::Rome,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(line, "");
}

#[test]
fn remaining_time_uses_reference_now() {
    let now = fixed_now_system();
    let window = RateLimitWindow {
        used_percentage: Some(50.0),
        resets_at: Some(10_000 + 3600 + 60),
    };

    let rendered = render_rate_limit(&window, "5h", now).expect("rendered");
    assert_eq!(rendered, "5h 50% 1h 1m");
}

#[test]
fn format_pct_rounds_to_single_decimal() {
    assert_eq!(format_pct(50.04), "50%");
    assert_eq!(format_pct(50.05), "50.1%");
    assert_eq!(format_pct(99.99), "100%");
}

#[test]
fn golden_statusline_with_colors_enabled() {
    let input = parse_input(
        r#"{
            "model":{"display_name":"Claude 3.7"},
            "context_window":{"used_percentage":12.34},
            "cost":{"total_cost_usd":12.34},
            "rate_limits":{
                "five_hour":{"used_percentage":50.0,"resets_at":13660},
                "seven_day":{"used_percentage":80.5,"resets_at":186400}
            }
        }"#,
    );

    let config = Config {
        separator: " | ".to_string(),
        timezone: "UTC".to_string(),
        colors_enabled: true,
        peak_hours: PeakHours {
            start_hour: 13,
            end_hour: 19,
        },
        items: vec![
            item(ItemKind::FiveHour, Some("5h"), Some("cyan")),
            item(ItemKind::SevenDay, Some("7d"), Some("blue")),
            item(ItemKind::Context, Some("ctx"), Some("magenta")),
            item(ItemKind::Peak, Some("🔥"), Some("red")),
            item(ItemKind::Model, None, Some("green")),
            item(ItemKind::Cost, Some("$"), Some("yellow")),
        ],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::UTC,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(
        line,
        "\x1b[36m5h 50% 1h 1m\x1b[0m | \x1b[34m7d 80.5% 2d 1h\x1b[0m | \x1b[35mctx 12.3%\x1b[0m | \x1b[31m🔥 4h 30m\x1b[0m | \x1b[32mClaude 3.7\x1b[0m | \x1b[33m$12.34\x1b[0m"
    );
}

#[test]
fn golden_statusline_with_colors_disabled() {
    let input = parse_input(
        r#"{
            "model":{"display_name":"Claude 3.7"},
            "context_window":{"used_percentage":12.34},
            "cost":{"total_cost_usd":12.34},
            "rate_limits":{
                "five_hour":{"used_percentage":50.0,"resets_at":13660},
                "seven_day":{"used_percentage":80.5,"resets_at":186400}
            }
        }"#,
    );

    let config = Config {
        separator: " | ".to_string(),
        timezone: "UTC".to_string(),
        colors_enabled: false,
        peak_hours: PeakHours {
            start_hour: 13,
            end_hour: 19,
        },
        items: vec![
            item(ItemKind::FiveHour, Some("5h"), Some("cyan")),
            item(ItemKind::SevenDay, Some("7d"), Some("blue")),
            item(ItemKind::Context, Some("ctx"), Some("magenta")),
            item(ItemKind::Peak, Some("🔥"), Some("red")),
            item(ItemKind::Model, None, Some("green")),
            item(ItemKind::Cost, Some("$"), Some("yellow")),
        ],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::UTC,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(
        line,
        "5h 50% 1h 1m | 7d 80.5% 2d 1h | ctx 12.3% | 🔥 4h 30m | Claude 3.7 | $12.34"
    );
}

#[test]
fn preview_sample_mode_works() {
    let output = execute(
        Cli {
            preview: Some(PreviewMode::Sample),
            auto_setup: false,
        },
        None,
    )
    .expect("sample preview should work");

    assert!(output.contains("Sample output:"));
    assert!(output.contains("Resolved config:"));
}

#[test]
fn preview_colors_lists_expected_colors() {
    let output = execute(
        Cli {
            preview: Some(PreviewMode::Colors),
            auto_setup: false,
        },
        None,
    )
    .expect("color preview should work");

    assert!(output.contains("Color preview"));
    assert!(output.contains("bright_cyan"));
}

#[test]
fn validate_config_reports_invalid_timezone() {
    let outcome = ConfigLoadOutcome {
        config: Config {
            timezone: "Invalid/Timezone".to_string(),
            ..Config::default()
        },
        resolved_path: PathBuf::from("statusline.toml"),
        source: ConfigSource::Fallback,
        file_exists: true,
        warnings: Vec::new(),
    };

    let report = validate_config(&outcome);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("invalid timezone"))
    );
}

#[test]
fn validate_config_reports_invalid_color() {
    let mut config = Config::default();
    config.items[0].color = Some("ultraviolet".to_string());
    let outcome = ConfigLoadOutcome {
        config,
        resolved_path: PathBuf::from("statusline.toml"),
        source: ConfigSource::Fallback,
        file_exists: true,
        warnings: Vec::new(),
    };

    let report = validate_config(&outcome);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("unsupported color"))
    );
}

#[test]
fn validate_config_reports_missing_command_for_command_item() {
    let outcome = ConfigLoadOutcome {
        config: Config {
            items: vec![ItemConfig {
                kind: ItemKind::Command,
                label: None,
                color: None,
                enabled: true,
                command: None,
                args: Vec::new(),
                timeout_ms: Some(100),
            }],
            ..Config::default()
        },
        resolved_path: PathBuf::from("statusline.toml"),
        source: ConfigSource::Fallback,
        file_exists: true,
        warnings: Vec::new(),
    };

    let report = validate_config(&outcome);
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("requires a non-empty command"))
    );
}

#[test]
fn validate_config_allows_multiple_command_items() {
    let outcome = ConfigLoadOutcome {
        config: Config {
            items: vec![
                command_item("printf", &["one"]),
                command_item("printf", &["two"]),
            ],
            ..Config::default()
        },
        resolved_path: PathBuf::from("statusline.toml"),
        source: ConfigSource::Fallback,
        file_exists: true,
        warnings: Vec::new(),
    };

    let report = validate_config(&outcome);
    assert!(report.is_valid());
    assert!(report.warnings.is_empty());
}

#[test]
fn validate_config_warns_when_non_command_item_sets_command_fields() {
    let outcome = ConfigLoadOutcome {
        config: Config {
            items: vec![ItemConfig {
                kind: ItemKind::Model,
                label: None,
                color: None,
                enabled: true,
                command: Some("   ".to_string()),
                args: Vec::new(),
                timeout_ms: None,
            }],
            ..Config::default()
        },
        resolved_path: PathBuf::from("statusline.toml"),
        source: ConfigSource::Fallback,
        file_exists: true,
        warnings: Vec::new(),
    };

    let report = validate_config(&outcome);
    assert!(report.is_valid());
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("ignores command/args/timeout_ms"))
    );
}

#[cfg(unix)]
#[test]
fn command_item_renders_external_output() {
    let input = parse_input("{}");
    let config = Config {
        separator: " | ".to_string(),
        timezone: "UTC".to_string(),
        colors_enabled: false,
        peak_hours: PeakHours::default(),
        items: vec![command_item("sh", &["-c", "printf '[CAVEMAN]' "])],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::UTC,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(line, "[CAVEMAN]");
}

#[cfg(unix)]
#[test]
fn command_item_collapses_multiline_output() {
    let input = parse_input("{}");
    let config = Config {
        separator: " | ".to_string(),
        timezone: "UTC".to_string(),
        colors_enabled: false,
        peak_hours: PeakHours::default(),
        items: vec![command_item("sh", &["-c", "printf 'alpha\\n\\nbeta\\n'"])],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::UTC,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(line, "alpha beta");
}

#[cfg(unix)]
#[test]
fn command_item_times_out_without_rendering() {
    let input = parse_input("{}");
    let config = Config {
        separator: " | ".to_string(),
        timezone: "UTC".to_string(),
        colors_enabled: false,
        peak_hours: PeakHours::default(),
        items: vec![command_item_with_timeout(
            "sh",
            &["-c", "sleep 1; printf late"],
            20,
        )],
    };

    let line = build_status_line(
        &input,
        &config,
        chrono_tz::UTC,
        fixed_now_utc(),
        fixed_now_system(),
    );

    assert_eq!(line, "");
}
