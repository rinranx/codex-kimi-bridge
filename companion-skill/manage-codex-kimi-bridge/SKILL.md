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

## Choose one of three startup modes

When the user asks to “start” or “open” the bridge, diagnose first and use the mode they selected. Never create competing listeners on port 8787.

1. **Codex on demand:** run `$HOME/.local/bin/codex-kimi-bridge serve` in the current task only when the user accepts that the process may end with the task or terminal session. This uses no bridge memory while stopped.
2. **Visible Terminal launcher:** open the repository or installation kit's `start-codex-kimi-bridge.command`. The user can see sanitized logs and stop it with `Control+C`.
3. **macOS LaunchAgent:** use the bundled `install-launchagent.command` only after the user explicitly asks for login-time automatic startup. It installs `~/Library/LaunchAgents/io.github.rinranx.codex-kimi-bridge.plist`, starts the same Rust binary in the background, and recovers it after an unexpected exit. The LaunchAgent is configuration for the existing `launchd`; it is not a second resident manager process.

For an installed LaunchAgent, inspect without changing state:

```sh
launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"
curl -s http://127.0.0.1:8787/health
```

Use `launchctl kickstart -k "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"` only when the installed service needs an explicit restart. Use the bundled `uninstall-launchagent.command` to stop automatic startup and move only the plist to Trash. Do not delete the binary, Codex configuration, Keychain item, or logs unless the user separately requests it.

The bundled LaunchAgent is configured for the default Kimi Code upstream. Kimi API Open Platform users must customize its `ProgramArguments` with the correct HTTPS upstream and model before installation; do not silently apply the membership defaults.

## Check Codex configuration

Inspect rather than rewrite configuration unless the user requests changes. Preserve unrelated TOML and avoid duplicate tables. The provider must include:

```toml
[model_providers.codex_kimi_bridge]
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
```

The auth command should retrieve the Keychain service and must not embed a secret in TOML. The `kimi_frontend` agent must reference `model_provider = "codex_kimi_bridge"` and must not contain role-unsupported fields such as `model_supports_reasoning_summaries`. Confirm Multi-agent v2 is enabled before diagnosing subagent scheduling.

## Troubleshoot

- `missing_api_key` or HTTP 401: inspect the provider auth command and Keychain service without displaying the value.
- Model access error: compare `model`, context window, and auto-compact limit with the user's Kimi Code membership or Open Platform account.
- `unsupported_tool_type`: version 0.3.0 and later support top-level `function` and `custom` plus `namespace` containers whose children are `function` or `custom`. Report any other Responses hosted tool type; do not silently remove it.
- Port occupied: determine whether Rust, the named Node fallback, an old 0.1.0 command, or an unknown service owns 8787. Ask before stopping anything.
- LaunchAgent failed: inspect its `launchctl print` state and `~/Library/Logs/codex-kimi-bridge.error.log`. Repeated restarts usually indicate a port conflict; identify the listener before changing state.
- A path resolves to old npm 0.1.0: explain the one-time command collision and ask before running `npm uninstall --global codex-kimi-bridge`.
- Stream or tool-call failure: reproduce offline with `translate-request`, then run the relevant source tests. For a namespaced call, verify the request is flattened to a collision-safe upstream name and the response restores the original `namespace + name` in both streaming and non-streaming paths. Custom-tool streaming remains experimental.
- Recursive subagents: the bridge does not impose a depth limit, a child/grandchild concurrency cap, or a mandatory stop-condition policy. Inspect Codex's own `[agents]` limits and role features instead of adding bridge-specific restrictions.
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
