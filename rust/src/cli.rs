use crate::error::{BridgeError, BridgeResult, sanitize_provider_error};
use crate::handoff::{
    HandoffVerifier, capture_user_prompt, default_state_dir, rewrite_pre_tool_use,
};
use crate::hook_config::{default_hooks_file, hooks_status, install_hooks, uninstall_hooks};
use crate::protocol::translate_responses_request_with_handoff;
use crate::server::{ServerConfig, is_loopback, is_port_available, serve};
use crate::sse::SseDecoder;
use crate::{DEFAULT_UPSTREAM, VERSION};
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

pub async fn run(args: Vec<String>) -> i32 {
    let parsed = parse_args(&args);
    let json_output = parsed.flag("json");
    let mut diagnostic_failed = false;
    let result = match parsed.command.as_deref() {
        None | Some("help" | "--help" | "-h") => {
            print!("{}", help_text());
            Ok(())
        }
        Some("version" | "--version" | "-v") => {
            println!("{VERSION}");
            Ok(())
        }
        Some("serve") => serve_command(&parsed).await,
        Some("doctor") => doctor_command(&parsed, json_output).await.map(|ok| {
            diagnostic_failed = !ok;
        }),
        Some("hook") => hook_command(&parsed),
        Some("hooks") => hooks_command(&parsed, json_output),
        Some("translate-request") => translate_request_command(&parsed, json_output),
        Some("request") => request_command(&parsed, json_output).await,
        Some(command) => Err(BridgeError::new(format!(
            "Unknown command: {command}. Run --help for usage."
        ))
        .code("unknown_command")),
    };
    match result {
        Ok(()) => i32::from(diagnostic_failed),
        Err(error) => {
            if json_output {
                let mut envelope = error.envelope();
                envelope
                    .as_object_mut()
                    .expect("error envelope is an object")
                    .insert("ok".into(), Value::Bool(false));
                println!("{envelope}");
            } else {
                eprintln!("Error: {}", error.message);
            }
            1
        }
    }
}

async fn serve_command(parsed: &ParsedArgs) -> BridgeResult<()> {
    let config = ServerConfig {
        host: parsed.string("host", "127.0.0.1"),
        port: parsed.integer_u16("port", 8787),
        upstream: parsed.string("upstream", DEFAULT_UPSTREAM),
        model: parsed.string("model", "k3"),
        timeout_ms: parsed.integer_u64("timeout-ms", 7_200_000),
        max_body_bytes: parsed.integer_usize("max-body-bytes", 128 * 1024 * 1024),
        allow_non_loopback: parsed.flag("allow-non-loopback"),
        allow_insecure_upstream: parsed.flag("allow-insecure-upstream"),
        quiet: parsed.flag("quiet"),
    };
    serve(config).await
}

async fn doctor_command(parsed: &ParsedArgs, json_output: bool) -> BridgeResult<bool> {
    let host = parsed.string("host", "127.0.0.1");
    let port = parsed.integer_u16("port", 8787);
    let upstream = parsed.string("upstream", DEFAULT_UPSTREAM);
    let upstream_url = validate_upstream(&upstream, parsed.flag("allow-insecure-upstream"))?;
    if !is_loopback(&host) && !parsed.flag("allow-non-loopback") {
        return Err(BridgeError::new(
            "Refusing to diagnose a non-loopback bind without --allow-non-loopback.",
        )
        .code("unsafe_bind_address"));
    }
    let port_available = is_port_available(&host, port).await;
    let mut local_service = None;
    let mut local_version = None;
    let mut local_implementation = None;
    if !port_available {
        let local_url = format!("http://{host}:{port}/health");
        if let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(Policy::none())
            .build()
        {
            if let Ok(response) = client.get(local_url).send().await {
                if response.status().is_success() {
                    if let Ok(value) = response.json::<Value>().await {
                        local_service = value
                            .get("service")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        local_version = value
                            .get("version")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        local_implementation = value
                            .get("implementation")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                    }
                }
            }
        }
    }
    let live = parsed.flag("live");
    let auth = detect_auth(live).await;
    let bind_ok = port_available || local_service.as_deref() == Some("codex-kimi-bridge");
    let mut upstream_check = json!({
        "ok": upstream_url.scheme() == "https" || (parsed.flag("allow-insecure-upstream") && is_loopback(upstream_url.host_str().unwrap_or(""))),
        "url": redacted_url(&upstream_url),
        "live_checked": false,
        "reachable": Value::Null,
    });
    if live {
        upstream_check["live_checked"] = Value::Bool(true);
        if let Some(secret) = auth.secret.as_deref() {
            let client = reqwest::Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|_| {
                    BridgeError::new("The Rust HTTPS client could not be initialized.")
                        .status(500)
                        .code("http_client_initialization_failed")
                })?;
            match client
                .post(upstream_url.clone())
                .bearer_auth(secret)
                .header("content-type", "application/json")
                .header("user-agent", format!("codex-kimi-bridge/{VERSION}"))
                .json(&json!({
                    "model": parsed.string("model", "k3"),
                    "messages": [{ "role": "user", "content": "Reply with OK only." }],
                    "max_completion_tokens": 8,
                    "reasoning_effort": "low",
                    "stream": false,
                    "prompt_cache_key": "codex-kimi-bridge-doctor",
                }))
                .send()
                .await
            {
                Ok(response) => {
                    upstream_check["reachable"] = Value::Bool(response.status().is_success());
                    if !response.status().is_success() {
                        upstream_check["http_status"] = json!(response.status().as_u16());
                    }
                }
                Err(_) => {
                    upstream_check["reachable"] = Value::Bool(false);
                    upstream_check["error"] = Value::String(
                        "The live request could not reach the configured upstream.".into(),
                    );
                }
            }
        } else {
            upstream_check["reachable"] = Value::Bool(false);
            upstream_check["error"] = Value::String(
                "No KIMI_CODE_API_KEY environment variable or macOS Keychain item was found."
                    .into(),
            );
        }
    }
    let upstream_ok = upstream_check["ok"].as_bool() == Some(true);
    let live_ok = !live || upstream_check["reachable"].as_bool() == Some(true);
    let ok = bind_ok && upstream_ok && live_ok;
    let result = json!({
        "ok": ok,
        "version": VERSION,
        "implementation": "rust",
        "checks": {
            "runtime": {
                "ok": true,
                "name": "rust",
                "version": VERSION,
                "external_runtime_required": false,
            },
            "bind": {
                "ok": bind_ok,
                "host": host,
                "port": port,
                "port_available": port_available,
                "running_service": local_service,
                "running_version": local_version,
                "running_implementation": local_implementation,
            },
            "upstream": upstream_check,
            "auth": {
                "available": auth.available,
                "source": auth.source,
                "note": "serve mode normally receives the Bearer token from Codex and does not store it",
            },
            "privacy": {
                "request_body_logging": false,
                "credential_logging": false,
                "default_bind_is_loopback": is_loopback(&host),
            }
        }
    });
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
        );
    } else {
        println!("codex-kimi-bridge {VERSION} (Rust)");
        println!("Runtime: Rust single binary");
        println!(
            "Port {host}:{port}: {}",
            if port_available {
                "available"
            } else if bind_ok {
                "bridge already running"
            } else {
                "occupied"
            }
        );
        println!("Auth for test commands: {}", auth.source);
        println!(
            "Upstream: {}",
            if live {
                if live_ok { "reachable" } else { "failed" }
            } else {
                "not contacted"
            }
        );
    }
    Ok(ok)
}

fn translate_request_command(parsed: &ParsedArgs, json_output: bool) -> BridgeResult<()> {
    let source = read_json_input(parsed)?;
    let handoff_verifier = load_handoff_verifier(parsed)?;
    let translated = translate_responses_request_with_handoff(
        source,
        &parsed.string("model", "k3"),
        None,
        handoff_verifier.as_ref(),
    )?;
    if json_output {
        let tool_kinds: Map<String, Value> = translated
            .context
            .tool_map
            .iter()
            .map(|(name, mapping)| (name.clone(), Value::String(mapping.kind.as_str().into())))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "request": translated.body,
                "tool_kinds": tool_kinds,
            }))
            .unwrap()
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&translated.body).unwrap()
        );
    }
    Ok(())
}

fn hook_command(parsed: &ParsedArgs) -> BridgeResult<()> {
    let action = parsed
        .positionals
        .first()
        .map(String::as_str)
        .ok_or_else(|| {
            BridgeError::new("A hook action is required: user-prompt-submit or pre-tool-use.")
                .code("missing_hook_action")
        })?;
    let state_dir = parsed
        .value("state-dir")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_state_dir)?;
    let input = read_stdin_json()?;
    match action {
        "user-prompt-submit" => capture_user_prompt(&input, &state_dir),
        "pre-tool-use" => {
            if let Some(output) = rewrite_pre_tool_use(&input, &state_dir, unix_seconds())? {
                println!("{output}");
            }
            Ok(())
        }
        _ => Err(BridgeError::new(format!(
            "Unknown hook action: {action}. Use user-prompt-submit or pre-tool-use."
        ))
        .code("unknown_hook_action")),
    }
}

fn hooks_command(parsed: &ParsedArgs, json_output: bool) -> BridgeResult<()> {
    let action = parsed
        .positionals
        .first()
        .map(String::as_str)
        .unwrap_or("status");
    let hooks_file = parsed
        .value("hooks-file")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_hooks_file)?;
    let result = match action {
        "install" => {
            let binary = parsed
                .value("binary")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| {
                    std::env::current_exe().map_err(|_| {
                        BridgeError::new(
                            "The current bridge executable path could not be resolved.",
                        )
                        .code("binary_path_unavailable")
                    })
                })?;
            install_hooks(&hooks_file, &binary)?
        }
        "status" => hooks_status(&hooks_file)?,
        "uninstall" => uninstall_hooks(&hooks_file)?,
        _ => {
            return Err(BridgeError::new(format!(
                "Unknown hooks action: {action}. Use install, status, or uninstall."
            ))
            .code("unknown_hooks_action"));
        }
    };
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
        );
    } else {
        println!("Codex Kimi handoff hooks: {action}");
        println!("Hooks file: {}", hooks_file.display());
        println!(
            "Installed: {}",
            if result["installed"].as_bool() == Some(true) {
                "yes"
            } else {
                "no"
            }
        );
        if let Some(backup) = result["backup"].as_str() {
            println!("Backup: {backup}");
        }
        if let Some(next_step) = result["next_step"].as_str() {
            println!("Next: {next_step}");
        }
    }
    Ok(())
}

fn load_handoff_verifier(parsed: &ParsedArgs) -> BridgeResult<Option<HandoffVerifier>> {
    match parsed.value("handoff-state-dir") {
        Some(path) => HandoffVerifier::from_state_dir_if_present(&PathBuf::from(path)),
        None => match default_state_dir() {
            Ok(path) => HandoffVerifier::from_state_dir_if_present(&path),
            Err(_) => Ok(None),
        },
    }
}

fn read_stdin_json() -> BridgeResult<Value> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).map_err(|_| {
        BridgeError::new("Could not read hook JSON from stdin.").code("input_read_failed")
    })?;
    serde_json::from_str(&text)
        .map_err(|_| BridgeError::new("Hook input must be valid JSON.").code("invalid_json"))
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn request_command(parsed: &ParsedArgs, json_output: bool) -> BridgeResult<()> {
    let url = reqwest::Url::parse(&parsed.string("url", "http://127.0.0.1:8787/v1/responses"))
        .map_err(|_| BridgeError::new("The local bridge URL is invalid.").code("invalid_url"))?;
    if !is_loopback(url.host_str().unwrap_or("")) {
        return Err(BridgeError::new(
            "The request command only sends API credentials to a loopback bridge URL.",
        )
        .code("unsafe_request_url"));
    }
    let auth = detect_auth(true).await;
    let secret = auth.secret.ok_or_else(|| {
        BridgeError::new(
            "No API key is available. Set KIMI_CODE_API_KEY or store codex-kimi-code-api-key in macOS Keychain.",
        )
        .status(401)
        .kind("authentication_error")
        .code("missing_api_key")
    })?;
    let body = if parsed.flags.contains_key("file")
        || parsed.positionals.first().is_some_and(|value| value == "-")
    {
        read_json_input(parsed)?
    } else {
        let input = parsed
            .value("input")
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| parsed.positionals.join(" "));
        if input.trim().is_empty() {
            return Err(BridgeError::new(
                "request requires --input, positional text, or --file.",
            ));
        }
        json!({
            "model": parsed.string("model", "k3"),
            "input": input,
            "reasoning": { "effort": parsed.string("effort", "low") },
            "stream": parsed.flag("stream"),
        })
    };
    let streaming = body.get("stream").and_then(Value::as_bool) == Some(true);
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(2 * 60 * 60))
        .build()
        .map_err(|_| {
            BridgeError::new("The Rust HTTP client could not be initialized.")
                .status(500)
                .code("http_client_initialization_failed")
        })?;
    let response = client
        .post(url)
        .bearer_auth(secret)
        .header("content-type", "application/json")
        .header(
            "accept",
            if streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .json(&body)
        .send()
        .await
        .map_err(|_| {
            BridgeError::new("The request command could not reach the local bridge.")
                .status(502)
                .kind("connection_error")
                .code("local_bridge_unreachable")
        })?;
    let status = response.status();
    if !status.is_success() {
        let value = response.json::<Value>().await.unwrap_or_else(
            |_| json!({ "error": { "message": format!("HTTP {}", status.as_u16()) } }),
        );
        let envelope = sanitize_provider_error(&value, status.as_u16());
        let error = envelope.get("error").cloned().unwrap_or_default();
        return Err(BridgeError::new(
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("The local bridge rejected the request."),
        )
        .status(status.as_u16())
        .kind(
            error
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("bridge_error"),
        )
        .code(
            error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("bridge_error"),
        ));
    }
    if !streaming {
        let value = response.json::<Value>().await.map_err(|_| {
            BridgeError::new("The local bridge returned invalid JSON.")
                .status(502)
                .code("invalid_bridge_response")
        })?;
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        );
        return Ok(());
    }
    let mut decoder = SseDecoder::default();
    let mut stream = response.bytes_stream();
    let mut events = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            BridgeError::new("The local bridge stream ended unexpectedly.")
                .status(502)
                .code("invalid_bridge_stream")
        })?;
        for frame in decoder.push(&chunk)? {
            if frame.data == "[DONE]" {
                break;
            }
            let event: Value = serde_json::from_str(&frame.data).map_err(|_| {
                BridgeError::new("The local bridge stream contained invalid JSON.")
                    .status(502)
                    .code("invalid_bridge_stream")
            })?;
            if json_output {
                events.push(event);
            } else if event.get("type").and_then(Value::as_str)
                == Some("response.output_text.delta")
            {
                print!(
                    "{}",
                    event.get("delta").and_then(Value::as_str).unwrap_or("")
                );
            }
        }
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "events": events })).unwrap()
        );
    } else {
        println!();
    }
    Ok(())
}

#[derive(Debug)]
struct AuthStatus {
    available: bool,
    source: String,
    secret: Option<String>,
}

async fn detect_auth(read_secret: bool) -> AuthStatus {
    if let Ok(secret) = std::env::var("KIMI_CODE_API_KEY") {
        if !secret.is_empty() {
            return AuthStatus {
                available: true,
                source: "env:KIMI_CODE_API_KEY".into(),
                secret: read_secret.then_some(secret),
            };
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("/usr/bin/security");
        command.args(["find-generic-password", "-s", "codex-kimi-code-api-key"]);
        if read_secret {
            command.arg("-w");
        }
        command.kill_on_drop(true);
        if let Ok(output) = command.output().await {
            if output.status.success() {
                let secret = if read_secret {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
                } else {
                    None
                };
                return AuthStatus {
                    available: true,
                    source: "macOS Keychain:codex-kimi-code-api-key".into(),
                    secret,
                };
            }
        }
    }
    AuthStatus {
        available: false,
        source: "missing".into(),
        secret: None,
    }
}

fn validate_upstream(value: &str, allow_insecure: bool) -> BridgeResult<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| {
        BridgeError::new("The upstream URL is invalid.").code("invalid_upstream_url")
    })?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            BridgeError::new("The upstream URL must not contain embedded credentials.")
                .code("unsafe_upstream_url"),
        );
    }
    if url.scheme() != "https" && !(allow_insecure && is_loopback(url.host_str().unwrap_or(""))) {
        return Err(BridgeError::new(
            "The upstream URL must use HTTPS. Plain HTTP is allowed only for an explicit loopback test server.",
        )
        .code("unsafe_upstream_url"));
    }
    Ok(url)
}

fn redacted_url(url: &reqwest::Url) -> String {
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!(
        "{}://{}{}{}",
        url.scheme(),
        url.host_str().unwrap_or("unknown"),
        port,
        url.path()
    )
}

fn read_json_input(parsed: &ParsedArgs) -> BridgeResult<Value> {
    let text = if let Some(file) = parsed.value("file") {
        std::fs::read_to_string(file).map_err(|_| {
            BridgeError::new(format!("Could not read JSON input file: {file}."))
                .code("input_read_failed")
        })?
    } else if let Some(file) = parsed
        .positionals
        .first()
        .filter(|file| file.as_str() != "-")
    {
        std::fs::read_to_string(file).map_err(|_| {
            BridgeError::new(format!("Could not read JSON input file: {file}."))
                .code("input_read_failed")
        })?
    } else {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map_err(|_| {
            BridgeError::new("Could not read JSON input from stdin.").code("input_read_failed")
        })?;
        text
    };
    serde_json::from_str(&text)
        .map_err(|_| BridgeError::new("Input must be valid JSON.").code("invalid_json"))
}

#[derive(Debug, Default)]
struct ParsedArgs {
    command: Option<String>,
    flags: BTreeMap<String, String>,
    positionals: Vec<String>,
}

impl ParsedArgs {
    fn flag(&self, name: &str) -> bool {
        self.flags.get(name).is_some_and(|value| value == "true")
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.flags
            .get(name)
            .map(String::as_str)
            .filter(|value| *value != "true")
    }

    fn string(&self, name: &str, fallback: &str) -> String {
        self.value(name).unwrap_or(fallback).to_owned()
    }

    fn integer_u16(&self, name: &str, fallback: u16) -> u16 {
        self.value(name)
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    }

    fn integer_u64(&self, name: &str, fallback: u64) -> u64 {
        self.value(name)
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    }

    fn integer_usize(&self, name: &str, fallback: usize) -> usize {
        self.value(name)
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(fallback)
    }
}

fn parse_args(args: &[String]) -> ParsedArgs {
    let boolean_flags: BTreeSet<&str> = [
        "json",
        "live",
        "stream",
        "quiet",
        "allow-non-loopback",
        "allow-insecure-upstream",
        "help",
        "version",
    ]
    .into_iter()
    .collect();
    let mut parsed = ParsedArgs::default();
    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        if parsed.command.is_none() && !token.starts_with('-') {
            parsed.command = Some(token.clone());
            index += 1;
            continue;
        }
        if token == "--" {
            parsed.positionals.extend_from_slice(&args[index + 1..]);
            break;
        }
        if let Some(flag) = token.strip_prefix("--") {
            if let Some((name, value)) = flag.split_once('=') {
                parsed.flags.insert(name.into(), value.into());
            } else if boolean_flags.contains(flag) {
                parsed.flags.insert(flag.into(), "true".into());
                if parsed.command.is_none() && flag == "help" {
                    parsed.command = Some("help".into());
                } else if parsed.command.is_none() && flag == "version" {
                    parsed.command = Some("version".into());
                }
            } else if args
                .get(index + 1)
                .is_some_and(|next| !next.starts_with('-'))
            {
                parsed.flags.insert(flag.into(), args[index + 1].clone());
                index += 1;
            } else {
                parsed.flags.insert(flag.into(), "true".into());
            }
        } else if token == "-h" {
            parsed.command.get_or_insert_with(|| "help".into());
        } else if token == "-v" {
            parsed.command.get_or_insert_with(|| "version".into());
        } else {
            parsed.positionals.push(token.clone());
        }
        index += 1;
    }
    parsed
}

fn help_text() -> String {
    format!(
        r#"codex-kimi-bridge {VERSION}

Single-binary Rust bridge: Codex Responses API -> Kimi Chat Completions.

Usage:
  codex-kimi-bridge serve [options]
  codex-kimi-bridge doctor [--json] [--live]
  codex-kimi-bridge hook <user-prompt-submit|pre-tool-use>
  codex-kimi-bridge hooks <install|status|uninstall> [--json]
  codex-kimi-bridge translate-request [--file request.json | -] [--json]
  codex-kimi-bridge request [text] [--stream] [--json]

Commands:
  serve              Listen for Codex on 127.0.0.1:8787.
  doctor             Check bind port, privacy defaults, auth source, and config.
  hook               Trusted local Codex task-handoff hook entry point.
  hooks              Safely merge, inspect, or remove the handoff hooks.
  translate-request  Offline conversion of a Responses request to Kimi Chat JSON.
  request             Explicit raw test request to a running bridge.
  version             Print the bridge version.

Serve options:
  --host <host>                 Default: 127.0.0.1
  --port <port>                 Default: 8787
  --model <model>               Default: k3
  --upstream <url>              Default: {DEFAULT_UPSTREAM}
  --timeout-ms <ms>             Default: 7200000
  --max-body-bytes <bytes>      Default: 134217728
  --quiet                       Suppress startup and sanitized error logs.
  --allow-non-loopback          Explicitly permit a non-loopback bind address.
  --allow-insecure-upstream     Permit plain HTTP only for loopback test servers.

Doctor options:
  --live                        Make one small Kimi request; consumes a tiny amount of quota.
  --json                        Stable machine-readable output.

Hook options:
  --state-dir <path>            Override the private local handoff state directory.

Hooks management options:
  --hooks-file <path>           Override the Codex hooks.json path.
  --binary <path>               Binary path written by hooks install.
  --json                        Stable machine-readable output.

Translate options:
  --handoff-state-dir <path>    Verify local CKB1 handoff envelopes from this directory.

Request auth precedence:
  1. KIMI_CODE_API_KEY
  2. macOS Keychain service codex-kimi-code-api-key

Security defaults:
  - API keys and request bodies are never logged.
  - The server binds only to loopback unless explicitly overridden.
  - The upstream must use HTTPS unless it is an explicit loopback test server.
  - Upstream redirects are rejected.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_json_before_command_is_supported() {
        let args = ["--json", "translate-request", "-"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        let parsed = parse_args(&args);
        assert_eq!(parsed.command.as_deref(), Some("translate-request"));
        assert!(parsed.flag("json"));
        assert_eq!(parsed.positionals, ["-"]);
    }
}
