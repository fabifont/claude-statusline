use clap::{Parser, ValueEnum};

/// Command-line options for claude-statusline.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "claude-statusline",
    version,
    about = "Minimal offline Claude Code status line formatter"
)]
pub struct Cli {
    /// Preview tools for configuration and output formatting.
    ///
    /// Use `--preview` for a default sample preview, or pass a mode:
    /// `sample`, `validate`, `config`, `explain`, `colors`.
    #[arg(long, value_enum, default_missing_value = "sample", num_args = 0..=1)]
    pub preview: Option<PreviewMode>,

    /// Update `~/.claude/settings.json` with a command-based `statusLine` block.
    #[arg(long = "auto-setup", conflicts_with = "preview")]
    pub auto_setup: bool,
}

/// Available preview modes.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub enum PreviewMode {
    /// Render a deterministic sample status line.
    Sample,
    /// Validate the resolved configuration and report issues.
    Validate,
    /// Show resolved config path, source, and item ordering.
    Config,
    /// Explain all supported fields and config keys.
    Explain,
    /// Preview supported ANSI colors.
    Colors,
}
