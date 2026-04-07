use std::time::Duration;

const SUPPORTED_COLORS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

/// Return a stable list of all supported color names.
pub fn supported_color_names() -> &'static [&'static str] {
    &SUPPORTED_COLORS
}

/// Check if a color name is supported by the formatter.
pub fn is_supported_color(color: &str) -> bool {
    matches!(
        color.to_ascii_lowercase().as_str(),
        "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "bright_black"
            | "gray"
            | "grey"
            | "bright_red"
            | "bright_green"
            | "bright_yellow"
            | "bright_blue"
            | "bright_magenta"
            | "bright_cyan"
            | "bright_white"
    )
}

/// Format a percentage with at most one decimal place.
pub fn format_pct(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract().abs() < f64::EPSILON {
        format!("{rounded:.0}%")
    } else {
        format!("{rounded:.1}%")
    }
}

/// Format duration into compact day/hour/minute/second segments.
pub fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;
    let seconds = total % 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

/// Apply an ANSI color escape when colors are enabled and valid.
pub fn apply_color(text: &str, color: Option<&str>, colors_enabled: bool) -> String {
    if !colors_enabled {
        return text.to_string();
    }

    let Some(color) = color else {
        return text.to_string();
    };

    let code = match color.to_ascii_lowercase().as_str() {
        "black" => "30",
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "blue" => "34",
        "magenta" => "35",
        "cyan" => "36",
        "white" => "37",
        "bright_black" | "gray" | "grey" => "90",
        "bright_red" => "91",
        "bright_green" => "92",
        "bright_yellow" => "93",
        "bright_blue" => "94",
        "bright_magenta" => "95",
        "bright_cyan" => "96",
        "bright_white" => "97",
        _ => return text.to_string(),
    };

    format!("\x1b[{code}m{text}\x1b[0m")
}
