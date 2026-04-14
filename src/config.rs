use crate::format::is_supported_color;
use crate::models::{Config, ItemKind};
use chrono_tz::Tz;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Source used to resolve the configuration file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    EnvVar,
    ClaudeHome,
    Xdg,
    Fallback,
}

/// Runtime config loading result with diagnostics.
#[derive(Debug, Clone)]
pub struct ConfigLoadOutcome {
    pub config: Config,
    pub resolved_path: PathBuf,
    pub source: ConfigSource,
    pub file_exists: bool,
    pub warnings: Vec<String>,
}

/// Validation report for preview mode.
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    /// True when no validation errors were found.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Parse an IANA timezone and fall back to Europe/Rome when invalid.
pub fn parse_timezone_or_default(timezone: &str) -> Tz {
    timezone.parse().unwrap_or(chrono_tz::Europe::Rome)
}

/// Load configuration for runtime usage, falling back to defaults when needed.
pub fn load_config_outcome() -> ConfigLoadOutcome {
    let (path, source) = resolve_config_path_with_source();
    let mut warnings = Vec::new();

    match fs::read_to_string(&path) {
        Ok(raw) => match toml::from_str::<Config>(&raw) {
            Ok(config) => ConfigLoadOutcome {
                config,
                resolved_path: path,
                source,
                file_exists: true,
                warnings,
            },
            Err(error) => {
                warnings.push(format!(
                    "failed to parse {}: {error}; using defaults",
                    path.display()
                ));
                ConfigLoadOutcome {
                    config: Config::default(),
                    resolved_path: path,
                    source,
                    file_exists: true,
                    warnings,
                }
            }
        },
        Err(error) => {
            let file_exists = path.exists();
            if file_exists {
                warnings.push(format!(
                    "failed to read {}: {error}; using defaults",
                    path.display()
                ));
            }
            ConfigLoadOutcome {
                config: Config::default(),
                resolved_path: path,
                source,
                file_exists,
                warnings,
            }
        }
    }
}

/// Resolve the config path according to documented lookup order.
pub fn resolve_config_path() -> PathBuf {
    resolve_config_path_with_source().0
}

fn resolve_config_path_with_source() -> (PathBuf, ConfigSource) {
    if let Ok(path) = env::var("CLAUDE_STATUSLINE_CONFIG") {
        return (PathBuf::from(path), ConfigSource::EnvVar);
    }

    if let Some(home) = home_dir() {
        let claude = home.join(".claude").join("statusline.toml");
        if claude.exists() {
            return (claude, ConfigSource::ClaudeHome);
        }

        let xdg = home
            .join(".config")
            .join("claude-statusline")
            .join("config.toml");
        if xdg.exists() {
            return (xdg, ConfigSource::Xdg);
        }

        return (claude, ConfigSource::Fallback);
    }

    (PathBuf::from("statusline.toml"), ConfigSource::Fallback)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

/// Validate configuration and collect actionable diagnostics.
pub fn validate_config(outcome: &ConfigLoadOutcome) -> ValidationReport {
    let mut report = ValidationReport::default();

    if !outcome.file_exists {
        report.warnings.push(format!(
            "config file not found at {}; defaults will be used",
            outcome.resolved_path.display()
        ));
    }

    for warning in &outcome.warnings {
        report.errors.push(warning.clone());
    }

    if outcome.config.timezone.parse::<Tz>().is_err() {
        report.errors.push(format!(
            "invalid timezone {:?}; expected IANA timezone like UTC or Europe/Rome",
            outcome.config.timezone
        ));
    }

    let peak = &outcome.config.peak_hours;
    if peak.start_hour >= 24 || peak.end_hour >= 24 {
        report
            .errors
            .push("peak_hours start_hour and end_hour must be in 0..=23".to_string());
    }
    if peak.start_hour == peak.end_hour {
        report
            .errors
            .push("peak_hours start_hour and end_hour must be different".to_string());
    }

    if outcome.config.items.is_empty() {
        report
            .warnings
            .push("items list is empty; status line will always be empty".to_string());
    }

    let mut seen = HashSet::<ItemKind>::new();
    let mut enabled_count = 0usize;
    for item in &outcome.config.items {
        if item.enabled {
            enabled_count += 1;
        }

        if item.kind != ItemKind::Command && !seen.insert(item.kind) {
            report.warnings.push(format!(
                "duplicate item kind {:?}; duplicates are allowed but can be confusing",
                item.kind
            ));
        }

        if let Some(color) = item.color.as_deref()
            && !is_supported_color(color)
        {
            report.errors.push(format!(
                "unsupported color {:?} for item {:?}",
                color, item.kind
            ));
        }

        let has_command_field = item.command.is_some();
        let has_command = item
            .command
            .as_deref()
            .is_some_and(|cmd| !cmd.trim().is_empty());
        let has_command_config =
            has_command_field || !item.args.is_empty() || item.timeout_ms.is_some();

        if item.kind == ItemKind::Command {
            if !has_command {
                report
                    .errors
                    .push("item kind command requires a non-empty command field".to_string());
            }

            if let Some(timeout_ms) = item.timeout_ms
                && timeout_ms == 0
            {
                report
                    .errors
                    .push("item kind command timeout_ms must be greater than 0".to_string());
            }
        } else if has_command_config {
            report.warnings.push(format!(
                "item kind {:?} ignores command/args/timeout_ms fields",
                item.kind
            ));
        }
    }

    if enabled_count == 0 {
        report
            .warnings
            .push("all items are disabled; status line will be empty".to_string());
    }

    report
}
