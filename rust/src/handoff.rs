use crate::error::{BridgeError, BridgeResult};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const ENVELOPE_PREFIX: &str = "CKB1.";

const KEY_BYTES: usize = 32;
const HMAC_BLOCK_BYTES: usize = 64;
const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;
const MAX_TASK_BYTES: usize = 256 * 1024;
const DEFAULT_ENVELOPE_TTL_SECONDS: i64 = 6 * 60 * 60;
const MAX_ENVELOPE_TTL_SECONDS: i64 = 24 * 60 * 60;
const MAX_CLOCK_SKEW_SECONDS: i64 = 5 * 60;
const PROMPT_RETENTION_SECONDS: u64 = 24 * 60 * 60;
const TARGET_AGENT_TYPE: &str = "kimi_frontend";
const TASK_OPEN: &str = "[KIMI_TASK]";
const TASK_CLOSE: &str = "[/KIMI_TASK]";

#[derive(Clone)]
pub struct HandoffVerifier {
    key: [u8; KEY_BYTES],
}

impl HandoffVerifier {
    pub fn from_state_dir_if_present(state_dir: &Path) -> BridgeResult<Option<Self>> {
        let key_path = state_dir.join("handoff.key");
        match fs::read_to_string(&key_path) {
            Ok(encoded) => Ok(Some(Self {
                key: decode_key(encoded.trim())?,
            })),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(handoff_error(
                "The local handoff signing key could not be read.",
                "handoff_key_unavailable",
            )),
        }
    }

    pub fn verify_for_recipient(
        &self,
        envelope: &str,
        recipient: &str,
        now: i64,
    ) -> BridgeResult<String> {
        if envelope.len() > MAX_ENVELOPE_BYTES {
            return Err(invalid_envelope("The local handoff envelope is too large."));
        }
        let encoded = envelope
            .strip_prefix(ENVELOPE_PREFIX)
            .ok_or_else(|| invalid_envelope("The local handoff envelope prefix is invalid."))?;
        let (payload_hex, signature_hex) = encoded
            .split_once('.')
            .ok_or_else(|| invalid_envelope("The local handoff envelope structure is invalid."))?;
        if signature_hex.contains('.') {
            return Err(invalid_envelope(
                "The local handoff envelope structure is invalid.",
            ));
        }
        let payload = hex_decode(payload_hex)
            .ok_or_else(|| invalid_envelope("The local handoff payload encoding is invalid."))?;
        let supplied_signature = hex_decode(signature_hex)
            .filter(|signature| signature.len() == KEY_BYTES)
            .ok_or_else(|| invalid_envelope("The local handoff signature is invalid."))?;
        let expected_signature = hmac_sha256(&self.key, &payload);
        if !constant_time_equal(&supplied_signature, &expected_signature) {
            return Err(invalid_envelope(
                "The local handoff signature could not be verified.",
            ));
        }

        let value: Value = serde_json::from_slice(&payload)
            .map_err(|_| invalid_envelope("The local handoff payload is not valid JSON."))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid_envelope("The local handoff payload must be an object."))?;
        let version = object.get("version").and_then(Value::as_u64);
        if version != Some(1) {
            return Err(invalid_envelope(
                "The local handoff envelope version is unsupported.",
            ));
        }
        let task_name = required_string(object, "task_name")?;
        let agent_type = required_string(object, "agent_type")?;
        let session_id = required_string(object, "session_id")?;
        let turn_id = required_string(object, "turn_id")?;
        if !safe_identifier(task_name, 256)
            || !safe_identifier(session_id, 128)
            || !safe_identifier(turn_id, 128)
            || agent_type != TARGET_AGENT_TYPE
        {
            return Err(invalid_envelope(
                "The local handoff routing metadata is invalid.",
            ));
        }
        let recipient_task_name = recipient.rsplit('/').next().unwrap_or(recipient);
        if recipient_task_name != task_name {
            return Err(invalid_envelope(
                "The local handoff recipient does not match the spawned task.",
            ));
        }
        let created_at = object
            .get("created_at")
            .and_then(Value::as_i64)
            .ok_or_else(|| invalid_envelope("The local handoff timestamp is invalid."))?;
        let expires_at = object
            .get("expires_at")
            .and_then(Value::as_i64)
            .ok_or_else(|| invalid_envelope("The local handoff expiry is invalid."))?;
        if created_at > now.saturating_add(MAX_CLOCK_SKEW_SECONDS)
            || expires_at < now
            || expires_at <= created_at
            || expires_at.saturating_sub(created_at) > MAX_ENVELOPE_TTL_SECONDS
        {
            return Err(invalid_envelope(
                "The local handoff envelope is expired or has invalid timing.",
            ));
        }
        let task = required_string(object, "task")?;
        if task.trim().is_empty() || task.len() > MAX_TASK_BYTES {
            return Err(invalid_envelope(
                "The local handoff task is empty or too large.",
            ));
        }
        Ok(task.to_owned())
    }

    #[cfg(test)]
    fn from_key(key: [u8; KEY_BYTES]) -> Self {
        Self { key }
    }
}

pub fn default_state_dir() -> BridgeResult<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        handoff_error(
            "HOME is unavailable, so the local handoff directory cannot be resolved.",
            "handoff_state_unavailable",
        )
    })?;
    #[cfg(target_os = "macos")]
    {
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("codex-kimi-bridge")
            .join("handoff-v1"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
            .join("codex-kimi-bridge")
            .join("handoff-v1"))
    }
}

pub fn capture_user_prompt(input: &Value, state_dir: &Path) -> BridgeResult<()> {
    require_hook_event(input, "UserPromptSubmit")?;
    let object = input.as_object().ok_or_else(|| {
        handoff_error(
            "The UserPromptSubmit hook input must be a JSON object.",
            "invalid_hook_input",
        )
    })?;
    let session_id = hook_identifier(object, "session_id")?;
    let turn_id = hook_identifier(object, "turn_id")?;
    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .filter(|prompt| !prompt.trim().is_empty())
        .ok_or_else(|| {
            handoff_error(
                "The UserPromptSubmit hook did not contain a visible prompt.",
                "missing_hook_prompt",
            )
        })?;
    if prompt.len() > MAX_TASK_BYTES {
        return Err(handoff_error(
            "The visible user prompt is too large for a local Kimi handoff.",
            "hook_prompt_too_large",
        ));
    }

    let prompts_dir = state_dir.join("prompts");
    ensure_private_directory(state_dir)?;
    ensure_private_directory(&prompts_dir)?;
    cleanup_stale_prompts(&prompts_dir);
    let path = prompt_path(&prompts_dir, session_id, turn_id);
    write_private_atomic(&path, prompt.as_bytes())
}

pub fn rewrite_pre_tool_use(
    input: &Value,
    state_dir: &Path,
    now: i64,
) -> BridgeResult<Option<Value>> {
    require_hook_event(input, "PreToolUse")?;
    let object = input.as_object().ok_or_else(|| {
        handoff_error(
            "The PreToolUse hook input must be a JSON object.",
            "invalid_hook_input",
        )
    })?;
    let tool_input = object.get("tool_input").and_then(Value::as_object);
    let agent_type = tool_input
        .and_then(|value| value.get("agent_type"))
        .and_then(Value::as_str);
    if agent_type != Some(TARGET_AGENT_TYPE) {
        return Ok(None);
    }

    let rewritten =
        rewrite_kimi_agent_call(object, tool_input.expect("checked above"), state_dir, now);
    Ok(Some(match rewritten {
        Ok(updated_input) => json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "updatedInput": updated_input,
            }
        }),
        Err(_) => denied_hook_output(),
    }))
}

fn rewrite_kimi_agent_call(
    hook: &Map<String, Value>,
    tool_input: &Map<String, Value>,
    state_dir: &Path,
    now: i64,
) -> BridgeResult<Value> {
    let session_id = hook_identifier(hook, "session_id")?;
    let turn_id = hook_identifier(hook, "turn_id")?;
    let task_name = tool_input
        .get("task_name")
        .and_then(Value::as_str)
        .filter(|value| safe_identifier(value, 256))
        .ok_or_else(|| {
            handoff_error(
                "The Kimi spawn task_name is missing or invalid.",
                "invalid_hook_input",
            )
        })?;
    let explicit_task = tool_input
        .get("message")
        .and_then(Value::as_str)
        .and_then(extract_marked_task);
    let prompt;
    let task = if let Some(task) = explicit_task {
        task
    } else {
        prompt = fs::read_to_string(prompt_path(&state_dir.join("prompts"), session_id, turn_id))
            .map_err(|_| {
            handoff_error(
                "No captured user prompt or explicit marked task is available for this Kimi spawn.",
                "missing_handoff_prompt",
            )
        })?;
        extract_task(&prompt)
    };
    if task.is_empty() || task.len() > MAX_TASK_BYTES {
        return Err(handoff_error(
            "The captured Kimi task is empty or too large.",
            "invalid_handoff_task",
        ));
    }
    let key = load_or_create_key(state_dir)?;
    let payload = json!({
        "version": 1,
        "session_id": session_id,
        "turn_id": turn_id,
        "task_name": task_name,
        "agent_type": TARGET_AGENT_TYPE,
        "created_at": now,
        "expires_at": now.saturating_add(DEFAULT_ENVELOPE_TTL_SECONDS),
        "task": task,
    });
    let envelope = sign_payload(&key, &payload)?;
    let mut updated_input = tool_input.clone();
    updated_input.insert("message".into(), Value::String(envelope));
    updated_input.insert("fork_turns".into(), Value::String("none".into()));
    Ok(Value::Object(updated_input))
}

fn denied_hook_output() -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Kimi handoff preparation failed. Send a new visible user task and retry; the provider was not contacted."
        }
    })
}

fn sign_payload(key: &[u8; KEY_BYTES], payload: &Value) -> BridgeResult<String> {
    let bytes = serde_json::to_vec(payload).map_err(|_| {
        handoff_error(
            "The local handoff payload could not be serialized.",
            "handoff_serialization_failed",
        )
    })?;
    let signature = hmac_sha256(key, &bytes);
    Ok(format!(
        "{ENVELOPE_PREFIX}{}.{}",
        hex_encode(&bytes),
        hex_encode(&signature)
    ))
}

fn load_or_create_key(state_dir: &Path) -> BridgeResult<[u8; KEY_BYTES]> {
    ensure_private_directory(state_dir)?;
    let key_path = state_dir.join("handoff.key");
    match fs::read_to_string(&key_path) {
        Ok(encoded) => return decode_key(encoded.trim()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(handoff_error(
                "The local handoff signing key could not be read.",
                "handoff_key_unavailable",
            ));
        }
    }

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut key = [0_u8; KEY_BYTES];
    key[..16].copy_from_slice(first.as_bytes());
    key[16..].copy_from_slice(second.as_bytes());
    let encoded = format!("{}\n", hex_encode(&key));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    match options.open(&key_path) {
        Ok(mut file) => {
            file.write_all(encoded.as_bytes()).map_err(|_| {
                handoff_error(
                    "The local handoff signing key could not be written.",
                    "handoff_key_unavailable",
                )
            })?;
            file.sync_all().map_err(|_| {
                handoff_error(
                    "The local handoff signing key could not be committed.",
                    "handoff_key_unavailable",
                )
            })?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let encoded = fs::read_to_string(&key_path).map_err(|_| {
                handoff_error(
                    "The local handoff signing key could not be read after creation.",
                    "handoff_key_unavailable",
                )
            })?;
            decode_key(encoded.trim())
        }
        Err(_) => Err(handoff_error(
            "The local handoff signing key could not be created.",
            "handoff_key_unavailable",
        )),
    }
}

fn decode_key(encoded: &str) -> BridgeResult<[u8; KEY_BYTES]> {
    let bytes = hex_decode(encoded)
        .filter(|bytes| bytes.len() == KEY_BYTES)
        .ok_or_else(|| {
            handoff_error(
                "The local handoff signing key is malformed.",
                "invalid_handoff_key",
            )
        })?;
    let mut key = [0_u8; KEY_BYTES];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn extract_task(prompt: &str) -> &str {
    extract_marked_task(prompt).unwrap_or_else(|| prompt.trim())
}

fn extract_marked_task(prompt: &str) -> Option<&str> {
    if let Some(open) = prompt.rfind(TASK_OPEN) {
        let tail = &prompt[open + TASK_OPEN.len()..];
        if let Some(close) = tail.find(TASK_CLOSE) {
            let marked = tail[..close].trim();
            if !marked.is_empty() {
                return Some(marked);
            }
        }
    }
    None
}

fn prompt_path(prompts_dir: &Path, session_id: &str, turn_id: &str) -> PathBuf {
    prompts_dir.join(format!("{session_id}--{turn_id}.txt"))
}

fn ensure_private_directory(path: &Path) -> BridgeResult<()> {
    fs::create_dir_all(path).map_err(|_| {
        handoff_error(
            "The local handoff directory could not be created.",
            "handoff_state_unavailable",
        )
    })?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| {
        handoff_error(
            "The local handoff directory permissions could not be secured.",
            "handoff_state_unavailable",
        )
    })?;
    Ok(())
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> BridgeResult<()> {
    let parent = path.parent().ok_or_else(|| {
        handoff_error(
            "The local handoff path is invalid.",
            "handoff_state_unavailable",
        )
    })?;
    let temporary = parent.join(format!(".handoff-{}.tmp", Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(handoff_error(
            "The visible user prompt could not be stored for the local handoff.",
            "handoff_state_unavailable",
        ));
    }
    Ok(())
}

fn cleanup_stale_prompts(prompts_dir: &Path) {
    let now = SystemTime::now();
    let Ok(entries) = fs::read_dir(prompts_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("txt") {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > Duration::from_secs(PROMPT_RETENTION_SECONDS));
        if stale {
            let _ = fs::remove_file(path);
        }
    }
}

fn require_hook_event<'a>(
    input: &'a Value,
    expected: &str,
) -> BridgeResult<&'a Map<String, Value>> {
    let object = input.as_object().ok_or_else(|| {
        handoff_error(
            "The Codex hook input must be a JSON object.",
            "invalid_hook_input",
        )
    })?;
    if object.get("hook_event_name").and_then(Value::as_str) != Some(expected) {
        return Err(handoff_error(
            "The Codex hook event name is invalid.",
            "invalid_hook_input",
        ));
    }
    Ok(object)
}

fn hook_identifier<'a>(object: &'a Map<String, Value>, field: &str) -> BridgeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| safe_identifier(value, 128))
        .ok_or_else(|| {
            handoff_error(
                format!("The Codex hook {field} is missing or invalid."),
                "invalid_hook_input",
            )
        })
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> BridgeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_envelope("The local handoff payload is incomplete."))
}

fn safe_identifier(value: &str, max_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_length
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'@')
        })
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; KEY_BYTES] {
    let mut normalized = [0_u8; HMAC_BLOCK_BYTES];
    if key.len() > HMAC_BLOCK_BYTES {
        normalized[..KEY_BYTES].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; HMAC_BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; HMAC_BLOCK_BYTES];
    for index in 0..HMAC_BLOCK_BYTES {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(data);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Some((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?))
        .collect()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn invalid_envelope(message: &str) -> BridgeError {
    handoff_error(message, "invalid_handoff_envelope")
        .param("input")
        .kind("invalid_request_error")
}

fn handoff_error(message: impl Into<String>, code: &str) -> BridgeError {
    BridgeError::new(message).code(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_state_dir() -> PathBuf {
        std::env::temp_dir().join(format!("codex-kimi-handoff-test-{}", Uuid::new_v4()))
    }

    fn user_prompt_input() -> Value {
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "session_123",
            "turn_id": "turn_456",
            "prompt": "Before\n[KIMI_TASK]\nReturn KIMI_SIGNED_HANDOFF_OK.\n[/KIMI_TASK]\nAfter"
        })
    }

    fn pre_tool_input() -> Value {
        json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session_123",
            "turn_id": "turn_456",
            "tool_name": "spawn_agent",
            "tool_input": {
                "agent_type": "kimi_frontend",
                "task_name": "signed_handoff_test",
                "fork_turns": "3",
                "message": "gAAAA_OPAQUE_PROVIDER_STATE"
            }
        })
    }

    #[test]
    fn hook_captures_prompt_rewrites_spawn_and_verifies_task() {
        let state_dir = temporary_state_dir();
        capture_user_prompt(&user_prompt_input(), &state_dir).unwrap();
        let output = rewrite_pre_tool_use(&pre_tool_input(), &state_dir, 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(
            output["hookSpecificOutput"]["updatedInput"]["fork_turns"],
            "none"
        );
        let envelope = output["hookSpecificOutput"]["updatedInput"]["message"]
            .as_str()
            .unwrap();
        assert!(envelope.starts_with(ENVELOPE_PREFIX));
        assert!(!envelope.contains("KIMI_SIGNED_HANDOFF_OK"));
        assert!(!envelope.contains("gAAAA"));

        let verifier = HandoffVerifier::from_state_dir_if_present(&state_dir)
            .unwrap()
            .unwrap();
        let task = verifier
            .verify_for_recipient(envelope, "/root/signed_handoff_test", 1_001)
            .unwrap();
        assert_eq!(task, "Return KIMI_SIGNED_HANDOFF_OK.");

        #[cfg(unix)]
        {
            let key_mode = fs::metadata(state_dir.join("handoff.key"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(key_mode, 0o600);
        }
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn rejects_tampered_expired_and_wrong_recipient_envelopes() {
        let key = [7_u8; KEY_BYTES];
        let verifier = HandoffVerifier::from_key(key);
        let payload = json!({
            "version": 1,
            "session_id": "session_123",
            "turn_id": "turn_456",
            "task_name": "signed_handoff_test",
            "agent_type": "kimi_frontend",
            "created_at": 1_000,
            "expires_at": 2_000,
            "task": "Return OK."
        });
        let envelope = sign_payload(&key, &payload).unwrap();
        assert!(
            verifier
                .verify_for_recipient(&envelope, "/root/other_task", 1_001)
                .is_err()
        );
        assert!(
            verifier
                .verify_for_recipient(&envelope, "/root/signed_handoff_test", 2_001)
                .is_err()
        );
        let mut tampered = envelope.into_bytes();
        let index = tampered.len() - 1;
        tampered[index] = if tampered[index] == b'0' { b'1' } else { b'0' };
        assert!(
            verifier
                .verify_for_recipient(
                    std::str::from_utf8(&tampered).unwrap(),
                    "/root/signed_handoff_test",
                    1_001,
                )
                .is_err()
        );
    }

    #[test]
    fn kimi_spawn_is_denied_when_no_visible_prompt_was_captured() {
        let state_dir = temporary_state_dir();
        let output = rewrite_pre_tool_use(&pre_tool_input(), &state_dir, 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            output["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .contains("provider was not contacted")
        );
    }

    #[test]
    fn explicit_marked_tool_task_supports_recursive_handoff_without_prompt_cache() {
        let state_dir = temporary_state_dir();
        let mut input = pre_tool_input();
        input["tool_input"]["message"] =
            Value::String("Context\n[KIMI_TASK]\nReturn KIMI_RECURSIVE_OK.\n[/KIMI_TASK]".into());
        input["tool_input"]["task_name"] = Value::String("recursive_kimi".into());
        let output = rewrite_pre_tool_use(&input, &state_dir, 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "allow");
        let envelope = output["hookSpecificOutput"]["updatedInput"]["message"]
            .as_str()
            .unwrap();
        let verifier = HandoffVerifier::from_state_dir_if_present(&state_dir)
            .unwrap()
            .unwrap();
        assert_eq!(
            verifier
                .verify_for_recipient(envelope, "/root/recursive_kimi", 1_001)
                .unwrap(),
            "Return KIMI_RECURSIVE_OK."
        );
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn non_kimi_agent_calls_are_not_rewritten() {
        let mut input = pre_tool_input();
        input["tool_input"]["agent_type"] = Value::String("worker".into());
        assert!(
            rewrite_pre_tool_use(&input, &temporary_state_dir(), 1_000)
                .unwrap()
                .is_none()
        );
    }
}
