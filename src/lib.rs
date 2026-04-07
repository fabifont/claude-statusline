#![doc = include_str!("../README.md")]

pub mod cli;
pub mod config;
pub mod error;
pub mod format;
pub mod models;
pub mod preview;
pub mod render;
pub mod setup;

use clap::Parser;
use std::io::{self, Read};
use std::process::ExitCode;

pub use cli::{Cli, PreviewMode};
pub use config::{ConfigLoadOutcome, ConfigSource, parse_timezone_or_default, validate_config};
pub use format::{
    apply_color, format_duration, format_pct, is_supported_color, supported_color_names,
};
pub use models::{
    ClockTime, Config, ContextWindow, Cost, ItemConfig, ItemKind, Model, PeakHours,
    RateLimitWindow, RateLimits, StatusInput,
};
pub use render::{build_status_line, parse_input, render_rate_limit};
pub use setup::setup_claude_config;

/// Execute the CLI using environment-provided arguments.
pub fn execute_from_env_args() -> ExitCode {
    let cli = Cli::parse();
    match execute(cli, None) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// Execute the application using a parsed CLI and optional stdin override.
pub fn execute(cli: Cli, stdin_override: Option<&str>) -> Result<String, error::StatuslineError> {
    if cli.auto_setup {
        if cli.preview.is_some() {
            return Err(error::StatuslineError::InvalidSetupUsage(
                "--auto-setup cannot be used with --preview".to_string(),
            ));
        }

        return setup::setup_claude_config();
    }

    if let Some(mode) = cli.preview {
        return preview::run_preview(mode);
    }

    run_normal_mode(stdin_override)
}

fn run_normal_mode(stdin_override: Option<&str>) -> Result<String, error::StatuslineError> {
    let outcome = config::load_config_outcome();
    let config = outcome.config;
    let input_buf = match stdin_override {
        Some(raw) => raw.to_string(),
        None => read_stdin()?,
    };

    let input = parse_input(&input_buf);
    let tz = parse_timezone_or_default(&config.timezone);

    Ok(build_status_line(
        &input,
        &config,
        tz,
        chrono::Utc::now(),
        std::time::SystemTime::now(),
    ))
}

fn read_stdin() -> Result<String, error::StatuslineError> {
    let mut input_buf = String::new();
    io::stdin()
        .read_to_string(&mut input_buf)
        .map_err(error::StatuslineError::StdinRead)?;
    Ok(input_buf)
}

#[cfg(test)]
mod tests;
