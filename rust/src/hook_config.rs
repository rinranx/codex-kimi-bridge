use crate::error::{BridgeError, BridgeResult};
use serde_json::{Map, Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const MANAGED_MARKER: &str = "codex-kimi-bridge-handoff-v1";
const AGENT_TOOL_MATCHER: &str =
    "^(Agent|spawn_agent|collaborationspawn_agent|collaboration[.:_]+spawn_agent)$";
const EVENTS: [&str; 2] = ["UserPromptSubmit", "PreToolUse"];

pub fn default_hooks_file() -> BridgeResult<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        hook_config_error(
            "HOME is unavailable, so the Codex hooks file cannot be resolved.",
            "hooks_file_unavailable",
        )
    })?;
    Ok(PathBuf::from(home).join(".codex").join("hooks.json"))
}

pub fn install_hooks(hooks_file: &Path, binary: &Path) -> BridgeResult<Value> {
    let (original, existed) = read_hooks_file(hooks_file)?;
    let mut updated = original.clone();
    remove_managed_hooks(&mut updated)?;
    let hooks = hooks_object_mut(&mut updated)?;
    append_hook_group(
        hooks,
        "UserPromptSubmit",
        None,
        managed_command(binary, "user-prompt-submit"),
    )?;
    append_hook_group(
        hooks,
        "PreToolUse",
        Some(AGENT_TOOL_MATCHER),
        managed_command(binary, "pre-tool-use"),
    )?;

    let changed = updated != original;
    let backup = if changed && existed {
        Some(backup_hooks_file(hooks_file)?)
    } else {
        None
    };
    if changed {
        write_hooks_file(hooks_file, &updated)?;
    }
    Ok(hooks_result(
        "installed",
        changed,
        hooks_file,
        backup.as_deref(),
        &updated,
    ))
}

pub fn uninstall_hooks(hooks_file: &Path) -> BridgeResult<Value> {
    let (original, existed) = read_hooks_file(hooks_file)?;
    let mut updated = original.clone();
    let removed = remove_managed_hooks(&mut updated)?;
    let changed = removed > 0;
    let backup = if changed && existed {
        Some(backup_hooks_file(hooks_file)?)
    } else {
        None
    };
    if changed {
        write_hooks_file(hooks_file, &updated)?;
    }
    Ok(hooks_result(
        "uninstalled",
        changed,
        hooks_file,
        backup.as_deref(),
        &updated,
    ))
}

pub fn hooks_status(hooks_file: &Path) -> BridgeResult<Value> {
    let (value, _) = read_hooks_file(hooks_file)?;
    Ok(hooks_result("status", false, hooks_file, None, &value))
}

fn read_hooks_file(path: &Path) -> BridgeResult<(Value, bool)> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let value: Value = serde_json::from_str(&text).map_err(|_| {
                hook_config_error(
                    "The existing Codex hooks file is not valid JSON; it was not changed.",
                    "invalid_hooks_file",
                )
            })?;
            if !value.is_object() {
                return Err(hook_config_error(
                    "The existing Codex hooks file must contain a JSON object; it was not changed.",
                    "invalid_hooks_file",
                ));
            }
            Ok((value, true))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((Value::Object(Map::new()), false))
        }
        Err(_) => Err(hook_config_error(
            "The Codex hooks file could not be read; it was not changed.",
            "hooks_file_unavailable",
        )),
    }
}

fn hooks_object_mut(value: &mut Value) -> BridgeResult<&mut Map<String, Value>> {
    let root = value.as_object_mut().ok_or_else(|| {
        hook_config_error(
            "The Codex hooks root must be an object.",
            "invalid_hooks_file",
        )
    })?;
    if !root.contains_key("hooks") {
        root.insert("hooks".into(), Value::Object(Map::new()));
    }
    root.get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            hook_config_error(
                "The Codex hooks field must be an object; it was not changed.",
                "invalid_hooks_file",
            )
        })
}

fn append_hook_group(
    hooks: &mut Map<String, Value>,
    event: &str,
    matcher: Option<&str>,
    command: String,
) -> BridgeResult<()> {
    if !hooks.contains_key(event) {
        hooks.insert(event.into(), Value::Array(Vec::new()));
    }
    let groups = hooks
        .get_mut(event)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            hook_config_error(
                format!("The Codex {event} hook list must be an array; it was not changed."),
                "invalid_hooks_file",
            )
        })?;
    let mut group = Map::new();
    if let Some(matcher) = matcher {
        group.insert("matcher".into(), Value::String(matcher.into()));
    }
    group.insert(
        "hooks".into(),
        json!([{
            "type": "command",
            "command": command,
            "timeout": 10,
        }]),
    );
    groups.push(Value::Object(group));
    Ok(())
}

fn remove_managed_hooks(value: &mut Value) -> BridgeResult<usize> {
    let Some(root) = value.as_object_mut() else {
        return Err(hook_config_error(
            "The Codex hooks root must be an object.",
            "invalid_hooks_file",
        ));
    };
    let Some(hooks_value) = root.get_mut("hooks") else {
        return Ok(0);
    };
    let hooks = hooks_value.as_object_mut().ok_or_else(|| {
        hook_config_error(
            "The Codex hooks field must be an object; it was not changed.",
            "invalid_hooks_file",
        )
    })?;
    let mut removed = 0;
    for event in EVENTS {
        let Some(groups_value) = hooks.get_mut(event) else {
            continue;
        };
        let groups = groups_value.as_array_mut().ok_or_else(|| {
            hook_config_error(
                format!("The Codex {event} hook list must be an array; it was not changed."),
                "invalid_hooks_file",
            )
        })?;
        for group in groups.iter_mut() {
            let Some(commands) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            let before = commands.len();
            commands.retain(|hook| !is_managed_hook(hook));
            removed += before.saturating_sub(commands.len());
        }
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|commands| !commands.is_empty())
        });
    }
    Ok(removed)
}

fn is_managed_hook(value: &Value) -> bool {
    value
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains(MANAGED_MARKER))
}

fn managed_command(binary: &Path, action: &str) -> String {
    format!(
        "{} hook {action} # {MANAGED_MARKER}",
        shell_quote(&binary.to_string_lossy())
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn backup_hooks_file(path: &Path) -> BridgeResult<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = path.with_file_name(format!(
        "{}.backup.{timestamp}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("hooks.json")
    ));
    fs::copy(path, &backup).map_err(|_| {
        hook_config_error(
            "The existing Codex hooks file could not be backed up; it was not changed.",
            "hooks_backup_failed",
        )
    })?;
    #[cfg(unix)]
    fs::set_permissions(&backup, fs::Permissions::from_mode(0o600)).map_err(|_| {
        hook_config_error(
            "The Codex hooks backup permissions could not be secured; it was not changed.",
            "hooks_backup_failed",
        )
    })?;
    Ok(backup)
}

fn write_hooks_file(path: &Path, value: &Value) -> BridgeResult<()> {
    let parent = path.parent().ok_or_else(|| {
        hook_config_error("The Codex hooks path is invalid.", "hooks_file_unavailable")
    })?;
    fs::create_dir_all(parent).map_err(|_| {
        hook_config_error(
            "The Codex hooks directory could not be created.",
            "hooks_file_unavailable",
        )
    })?;
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| {
        hook_config_error(
            "The Codex hooks file could not be serialized.",
            "hooks_write_failed",
        )
    })?;
    bytes.push(b'\n');
    let temporary = parent.join(format!(".hooks-{}.tmp", Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(hook_config_error(
            "The Codex hooks file could not be written atomically.",
            "hooks_write_failed",
        ));
    }
    Ok(())
}

fn hooks_result(
    action: &str,
    changed: bool,
    hooks_file: &Path,
    backup: Option<&Path>,
    value: &Value,
) -> Value {
    let (prompt_count, pre_tool_count) = managed_hook_counts(value);
    json!({
        "ok": true,
        "action": action,
        "changed": changed,
        "hooks_file": hooks_file,
        "backup": backup,
        "installed": prompt_count > 0 && pre_tool_count > 0,
        "managed_hooks": {
            "UserPromptSubmit": prompt_count,
            "PreToolUse": pre_tool_count,
        },
        "trust_required": prompt_count > 0 && pre_tool_count > 0,
        "next_step": if prompt_count > 0 && pre_tool_count > 0 {
            "Restart Codex Desktop, open /hooks, review both commands, and trust them."
        } else {
            "No Codex Kimi handoff hooks are installed."
        },
    })
}

fn managed_hook_counts(value: &Value) -> (usize, usize) {
    let count = |event: &str| {
        value
            .get("hooks")
            .and_then(|hooks| hooks.get(event))
            .and_then(Value::as_array)
            .map(|groups| {
                groups
                    .iter()
                    .filter_map(|group| group.get("hooks").and_then(Value::as_array))
                    .flatten()
                    .filter(|hook| is_managed_hook(hook))
                    .count()
            })
            .unwrap_or(0)
    };
    (count("UserPromptSubmit"), count("PreToolUse"))
}

fn hook_config_error(message: impl Into<String>, code: &str) -> BridgeError {
    BridgeError::new(message).code(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_update_and_uninstall_preserve_unrelated_hooks() {
        let directory =
            std::env::temp_dir().join(format!("codex-kimi-hook-config-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let hooks_file = directory.join("hooks.json");
        fs::write(
            &hooks_file,
            serde_json::to_vec_pretty(&json!({
                "hooks": {
                    "UserPromptSubmit": [{
                        "hooks": [{
                            "type": "command",
                            "command": "keep-me",
                            "timeout": 5
                        }]
                    }],
                    "PreToolUse": [{
                        "matcher": "^Agent$",
                        "hooks": [{
                            "type": "command",
                            "command": "'/old/codex-kimi-bridge' hook pre-tool-use # codex-kimi-bridge-handoff-v1",
                            "timeout": 10
                        }]
                    }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let binary = Path::new("/Users/example/.local/bin/codex-kimi-bridge");
        let installed = install_hooks(&hooks_file, binary).unwrap();
        assert_eq!(installed["changed"], true);
        assert_eq!(installed["installed"], true);
        assert!(installed["backup"].as_str().is_some());
        let text = fs::read_to_string(&hooks_file).unwrap();
        assert!(text.contains("keep-me"));
        assert!(text.contains(
            "\"matcher\": \"^(Agent|spawn_agent|collaborationspawn_agent|collaboration[.:_]+spawn_agent)$\""
        ));
        assert!(!text.contains("\"matcher\": \"^Agent$\""));
        assert!(!text.contains("/old/codex-kimi-bridge"));
        assert!(text.contains(MANAGED_MARKER));

        let unchanged = install_hooks(&hooks_file, binary).unwrap();
        assert_eq!(unchanged["changed"], false);
        assert!(unchanged["backup"].is_null());

        let removed = uninstall_hooks(&hooks_file).unwrap();
        assert_eq!(removed["changed"], true);
        assert_eq!(removed["installed"], false);
        let text = fs::read_to_string(&hooks_file).unwrap();
        assert!(text.contains("keep-me"));
        assert!(!text.contains(MANAGED_MARKER));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn shell_quotes_binary_paths() {
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\"'\"'b'");
    }
}
