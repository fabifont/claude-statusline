use crate::error::StatuslineError;
use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_STATUSLINE_CONFIG: &str = include_str!("../statusline.toml.example");
const PROJECT_CONFIG_MARKER: &str = "claude-statusline-project: fabifont/claude-statusline";

/// Update `~/.claude/settings.json` with a command-based `statusLine` block.
pub fn setup_claude_config() -> Result<String, StatuslineError> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(StatuslineError::HomeDirMissing)?;

    let settings_path = home.join(".claude").join("settings.json");
    let settings_action = upsert_statusline_in_file(&settings_path, "claude-statusline")?;
    let config_path = home.join(".claude").join("statusline.toml");
    let config_action = ensure_default_statusline_config(&config_path)?;

    Ok(format!(
        "Updated Claude config at {} ({settings_action}); default statusline config at {} ({config_action})",
        settings_path.display(),
        config_path.display()
    ))
}

fn ensure_default_statusline_config(path: &Path) -> Result<&'static str, StatuslineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| StatuslineError::ClaudeSettingsCreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    if path.exists() {
        let existing = fs::read_to_string(path).unwrap_or_default();
        if existing.contains(PROJECT_CONFIG_MARKER) {
            return Ok("kept existing project config");
        }

        write_default_statusline_config(path)?;
        return Ok("replaced non-project config");
    }

    write_default_statusline_config(path)?;

    Ok("created default config")
}

fn write_default_statusline_config(path: &Path) -> Result<(), StatuslineError> {
    fs::write(path, DEFAULT_STATUSLINE_CONFIG).map_err(|source| {
        StatuslineError::StatuslineConfigWrite {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn upsert_statusline_in_file(path: &Path, command: &str) -> Result<&'static str, StatuslineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| StatuslineError::ClaudeSettingsCreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut root = match fs::read_to_string(path) {
        Ok(raw) => {
            serde_json::from_str(&raw).map_err(|source| StatuslineError::ClaudeSettingsParse {
                path: path.to_path_buf(),
                source,
            })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(source) => {
            return Err(StatuslineError::ClaudeSettingsRead {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let action = upsert_statusline_value(&mut root, command)?;

    let mut rendered =
        serde_json::to_string_pretty(&root).map_err(StatuslineError::ClaudeSettingsSerialize)?;
    rendered.push('\n');

    fs::write(path, rendered).map_err(|source| StatuslineError::ClaudeSettingsWrite {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(action)
}

pub(crate) fn upsert_statusline_value(
    root: &mut Value,
    command: &str,
) -> Result<&'static str, StatuslineError> {
    let root_obj = root
        .as_object_mut()
        .ok_or(StatuslineError::ClaudeSettingsRootNotObject)?;

    let action = match root_obj.get("statusLine") {
        Some(Value::Object(_)) => "updated existing statusLine",
        Some(_) => "replaced non-object statusLine",
        None => "created statusLine",
    };

    let mut statusline = Map::new();
    statusline.insert("type".to_string(), Value::String("command".to_string()));
    statusline.insert("command".to_string(), Value::String(command.to_string()));
    root_obj.insert("statusLine".to_string(), Value::Object(statusline));

    Ok(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn upsert_creates_statusline_when_missing() {
        let mut root = json!({"theme":"dark"});
        let action = upsert_statusline_value(&mut root, "claude-statusline").expect("must update");

        assert_eq!(action, "created statusLine");
        assert_eq!(root["statusLine"]["type"], "command");
        assert_eq!(root["statusLine"]["command"], "claude-statusline");
    }

    #[test]
    fn upsert_replaces_existing_object_and_removes_padding() {
        let mut root = json!({
            "statusLine": {
                "type": "command",
                "command": "old-command",
                "padding": 1,
                "other": "will-be-removed"
            },
            "other": true
        });

        let action = upsert_statusline_value(&mut root, "new-command").expect("must update");

        assert_eq!(action, "updated existing statusLine");
        assert_eq!(root["statusLine"]["type"], "command");
        assert_eq!(root["statusLine"]["command"], "new-command");
        assert!(root["statusLine"].get("padding").is_none());
        assert!(root["statusLine"].get("other").is_none());
        assert_eq!(root["statusLine"].as_object().expect("object").len(), 2);
        assert_eq!(root["other"], true);
    }

    #[test]
    fn upsert_replaces_non_object_statusline() {
        let mut root = json!({"statusLine":"enabled"});

        let action = upsert_statusline_value(&mut root, "claude-statusline").expect("must update");

        assert_eq!(action, "replaced non-object statusLine");
        assert_eq!(root["statusLine"]["type"], "command");
    }

    #[test]
    fn upsert_rejects_non_object_root() {
        let mut root = json!([1, 2, 3]);
        let err = upsert_statusline_value(&mut root, "claude-statusline").expect_err("must fail");

        assert!(matches!(err, StatuslineError::ClaudeSettingsRootNotObject));
    }

    #[test]
    fn ensure_default_config_creates_when_missing() {
        let base = std::env::temp_dir().join(format!(
            "claude-statusline-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let config_path = base.join(".claude").join("statusline.toml");

        let action = ensure_default_statusline_config(&config_path).expect("must create config");

        assert_eq!(action, "created default config");
        let raw = std::fs::read_to_string(&config_path).expect("must read config");
        assert_eq!(raw, DEFAULT_STATUSLINE_CONFIG);

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn ensure_default_config_keeps_existing() {
        let base = std::env::temp_dir().join(format!(
            "claude-statusline-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let config_path = base.join(".claude").join("statusline.toml");
        std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(
            &config_path,
            "# claude-statusline-project: fabifont/claude-statusline\nseparator = \" :: \"\n",
        )
        .expect("write");

        let action = ensure_default_statusline_config(&config_path).expect("must keep config");

        assert_eq!(action, "kept existing project config");
        let raw = std::fs::read_to_string(&config_path).expect("must read config");
        assert_eq!(
            raw,
            "# claude-statusline-project: fabifont/claude-statusline\nseparator = \" :: \"\n"
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn ensure_default_config_replaces_unmarked_existing() {
        let base = std::env::temp_dir().join(format!(
            "claude-statusline-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let config_path = base.join(".claude").join("statusline.toml");
        std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&config_path, "separator = \" :: \"\n").expect("write");

        let action =
            ensure_default_statusline_config(&config_path).expect("must replace unmarked config");

        assert_eq!(action, "replaced non-project config");
        let raw = std::fs::read_to_string(&config_path).expect("must read config");
        assert_eq!(raw, DEFAULT_STATUSLINE_CONFIG);

        let _ = std::fs::remove_dir_all(base);
    }
}
