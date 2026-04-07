use std::path::PathBuf;
use thiserror::Error;

/// Application-level errors.
#[derive(Debug, Error)]
pub enum StatuslineError {
    /// Reading stdin failed.
    #[error("failed to read stdin: {0}")]
    StdinRead(#[source] std::io::Error),

    /// Preview validation failed with user-facing diagnostics.
    #[error("{0}")]
    ValidationFailed(String),

    /// Setup command arguments are inconsistent.
    #[error("invalid setup usage: {0}")]
    InvalidSetupUsage(String),

    /// Cannot determine the current user's home directory.
    #[error("failed to resolve home directory")]
    HomeDirMissing,

    /// Failed to create the Claude config directory.
    #[error("failed to create Claude config directory {path}: {source}")]
    ClaudeSettingsCreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to read existing Claude settings.
    #[error("failed to read Claude settings at {path}: {source}")]
    ClaudeSettingsRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Existing Claude settings are invalid JSON.
    #[error("failed to parse Claude settings at {path}: {source}")]
    ClaudeSettingsParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    /// Claude settings root must be a JSON object.
    #[error("Claude settings root must be a JSON object")]
    ClaudeSettingsRootNotObject,

    /// Failed to serialize updated Claude settings.
    #[error("failed to serialize Claude settings: {0}")]
    ClaudeSettingsSerialize(#[source] serde_json::Error),

    /// Failed to write updated Claude settings.
    #[error("failed to write Claude settings at {path}: {source}")]
    ClaudeSettingsWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to write default statusline config.
    #[error("failed to write default statusline config at {path}: {source}")]
    StatuslineConfigWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
