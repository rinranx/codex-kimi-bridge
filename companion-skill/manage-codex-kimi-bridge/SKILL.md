---
name: manage-codex-kimi-bridge
description: Operate and diagnose the local Rust Kimi bridge used by Codex Desktop. Use when asked to start, stop, inspect, test, troubleshoot, upgrade, or configure codex-kimi-bridge; when Codex reports provider, Responses API, streaming, tool-call, authentication, model-access, or port 8787 errors; or when choosing between the Rust default and Node fallback.
---

# Manage Codex Kimi Bridge

Treat the Rust single binary `codex-kimi-bridge` as the default implementation. Do not substitute an unrelated npm package or run an `npx` package with a similar name. The repository's fallback has the distinct command `codex-kimi-bridge-node` and should be used only after a confirmed Rust compatibility problem.

## Diagnose before changing state

1. Resolve both possible commands with `command -v codex-kimi-bridge` and `command -v codex-kimi-bridge-node`.
2. Run `codex-kimi-bridge --version` and `codex-kimi-bridge doctor --json`. The normal installed Rust path is `~/.local/bin/codex-kimi-bridge`.
3. When a service is running, inspect `http://127.0.0.1:8787/health`. The default must report `service: codex-kimi-bridge` and `implementation: rust`.
4. If port 8787 is occupied by an unknown process, identify it before proposing termination. Never stop or kill it without user authorization.
5. Run `doctor --live` or `request` only after the user explicitly authorizes a real provider test; either command contacts Kimi and consumes quota.

Never print, copy, or log an API key. The server receives a Bearer token from Codex. On macOS, explicit test commands may read the Keychain service `codex-kimi-code-api-key` without displaying its value.

## Start the Rust default

Run:

```sh
$HOME/.local/bin/codex-kimi-bridge serve
```

Expected defaults:

- Local endpoint: `http://127.0.0.1:8787/v1`
- Upstream: `https://api.kimi.com/coding/v1/chat/completions`
- Model: `k3`
- Startup marker: `implementation: rust`

For another Kimi Code model, pass `--model`. For a Kimi API Open Platform key, pass its official HTTPS `--upstream` and enabled `--model`; do not mix membership and Open Platform credentials or model IDs.

Keep loopback binding and HTTPS defaults. Use `--allow-non-loopback` or `--allow-insecure-upstream` only after explaining the exposure and receiving explicit user direction.

## Check Codex configuration

Inspect rather than rewrite configuration unless the user requests changes. Preserve unrelated TOML and avoid duplicate tables. The provider must include:

```toml
[model_providers.codex_kimi_bridge]
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
```

The auth command should retrieve the Keychain service and must not embed a secret in TOML. The `kimi_frontend` agent must reference `model_provider = "codex_kimi_bridge"`. Confirm Multi-agent v2 is enabled before diagnosing subagent scheduling.

## Troubleshoot

- `missing_api_key` or HTTP 401: inspect the provider auth command and Keychain service without displaying the value.
- Model access error: compare `model`, context window, and auto-compact limit with the user's Kimi Code membership or Open Platform account.
- `unsupported_tool_type`: report which Responses hosted tool cannot be translated; do not silently remove it.
- Port occupied: determine whether Rust, the named Node fallback, an old 0.1.0 command, or an unknown service owns 8787. Ask before stopping anything.
- A path resolves to old npm 0.1.0: explain the one-time command collision and ask before running `npm uninstall --global codex-kimi-bridge`.
- Stream or tool-call failure: reproduce offline with `translate-request`, then run the relevant source tests. Custom-tool streaming remains experimental.
- Long reasoning continuity: explain that reasoning state exists only in the current bridge process. Start a new Kimi subagent task after a mid-tool-chain restart.
- Rust compatibility problem: stop Rust first, then use the repository's `codex-kimi-bridge-node`; never run both on 8787.

## Validate source changes

After Rust changes, run from `rust/`:

```sh
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
```

After Node fallback changes, run from `node/`:

```sh
npm run check
npm test
npm run smoke
```

For protocol changes, also compare both implementations against `compat/responses-request.json`. Report all checks and explicitly state whether a live Kimi request was skipped.
