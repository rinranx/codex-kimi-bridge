use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_codex-kimi-bridge")
}

fn run_with_stdin(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn reports_version() {
    let output = Command::new(binary()).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "0.2.0-alpha.2\n");
}

#[test]
fn supports_global_json_before_translate_request() {
    let output = run_with_stdin(
        &["--json", "translate-request", "-"],
        &json!({
            "model": "k3",
            "input": "Hello",
            "reasoning": { "effort": "medium" },
            "stream": false
        })
        .to_string(),
    );
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["request"]["reasoning_effort"], "high");
    assert_eq!(value["request"]["messages"][0]["content"], "Hello");
}

#[test]
fn json_errors_are_stable_and_do_not_echo_the_request() {
    let output = run_with_stdin(
        &["--json", "translate-request", "-"],
        &json!({
            "model": "k3",
            "input": "PRIVATE_SECRET_BODY",
            "tools": [{ "type": "unknown" }]
        })
        .to_string(),
    );
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "unsupported_tool_type");
    assert!(!text.contains("PRIVATE_SECRET_BODY"));
}
