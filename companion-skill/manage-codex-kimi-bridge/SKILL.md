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

## Use the native Kimi subagent safely

Codex Desktop's provider-private `encrypted_content` remains opaque and cannot be decrypted by the bridge. Version `0.4.0` uses two user-trusted Codex Hooks to capture a visible task before private wrapping and carry it through a locally signed `CKB1` envelope:

1. Run `codex-kimi-bridge hooks status --json`. This is read-only. Require both managed hooks before using the signed path. The `PreToolUse` matcher must cover the known Codex names `Agent`, `spawn_agent`, `collaborationspawn_agent`, and separator-based `collaboration.spawn_agent` compatibility forms. Multi-Agent V2 currently flattens the namespace to `collaborationspawn_agent`; `^Agent$` and separator-only matchers miss that Desktop path.
2. If they are absent and the user asks to enable the feature, run `codex-kimi-bridge hooks install`. It merges into `~/.codex/hooks.json`, preserves unrelated hooks, and backs up an existing file. The user must restart Desktop, open `/hooks`, review both commands, and trust them; never bypass hook trust.
3. Prefer asking the user to put the exact task between `[KIMI_TASK]` and `[/KIMI_TASK]`. Without markers, the whole current visible user prompt is handed off. Never include credentials or secrets.
4. Spawn `kimi_frontend` with `fork_turns = "none"`. The trusted `PreToolUse` hook rewrites only that agent type, binds the envelope to `task_name`, and leaves other agent calls untouched.
5. Let Kimi report progress as ordinary assistant commentary and finish with an ordinary final answer. Do not ask it to call `send_message` or `followup_task` back to an OpenAI parent; those cross-provider tool messages can become opaque provider state.
6. For a Kimi-created `kimi_frontend` descendant, put the exact descendant task between `[KIMI_TASK]` markers in the `spawn_agent.message`. This explicit marked tool input can be signed without a new `UserPromptSubmit` event.

If the signed hooks are unavailable, an explicit `[KIMI_TASK]` in completed visible history plus the smallest positive `fork_turns` value remains a compatibility fallback. It is not the default and should be reported as less reliable.

The envelope authenticates source and integrity; it is not encryption. Visible prompts are cached with mode `0600` under `~/Library/Caches/codex-kimi-bridge/handoff-v1`, stale prompt files are cleaned after 24 hours, and envelopes expire after six hours. Never describe this as decryption or confidential storage.

The bridge emits Responses assistant messages with `phase = "commentary"` during tool work and `phase = "final_answer"` for terminal text so Desktop can classify the child transcript like a native provider. This is protocol compatibility, not UI spoofing; Codex still owns the thread ID, status, panel, and result delivery.

## Troubleshoot

- `missing_api_key` or HTTP 401: inspect the provider auth command and Keychain service without displaying the value.
- Model access error: compare `model`, context window, and auto-compact limit with the user's Kimi Code membership or Open Platform account.
- `unsupported_tool_type`: version 0.3.0 and later support top-level `function` and `custom` plus `namespace` containers whose children are `function` or `custom`. Report any other Responses hosted tool type; do not silently remove it.
- `unsupported_input_item` for `agent_message`: released version 0.3.0 and earlier do not accept the native Codex Desktop inter-agent item. Version 0.3.1 and later normalize it to a `user` Chat message, retain validated `author` and `recipient` routes in a fixed metadata prefix, and never forward the item ID or `internal_chat_message_metadata_passthrough`. Check the running binary version before diagnosing Kimi or falling back to a direct text request.
- `unsupported_content_part` for `encrypted_content`: version 0.3.1 accepts `agent_message` but rejects this part. Version 0.3.2 and later safely omit opaque values. Version `0.4.0` recognizes only its own `CKB1` envelope after HMAC, recipient, agent-type, and expiry verification; it still never decrypts or forwards OpenAI provider state.
- `missing_handoff_envelope` or Kimi returns `Payload 为空`: check the running version, then run `hooks status --json`. Confirm the current user request triggered `UserPromptSubmit`, Desktop trusts both hooks, and `task_name` matches the child recipient. Do not work around a failed signature by forwarding the raw encrypted value.
- Hook installation collides with existing hooks: inspect the timestamped backup and current JSON. The managed installer marks only its two command entries and preserves unrelated groups. Never replace the whole file. Use `hooks uninstall` to remove only managed entries.
- The Kimi child runs but its transcript is missing or collapsed in Desktop: inspect the saved child session structurally. Assistant Responses messages should contain `phase = "commentary"` for tool work and `phase = "final_answer"` for terminal text. Do not alter thread IDs or patch the signed Desktop app.
- Port occupied: determine whether Rust, the named Node fallback, an old 0.1.0 command, or an unknown service owns 8787. Ask before stopping anything.
- LaunchAgent failed: inspect its `launchctl print` state and `~/Library/Logs/codex-kimi-bridge.error.log`. Repeated restarts usually indicate a port conflict; identify the listener before changing state.
- A path resolves to old npm 0.1.0: explain the one-time command collision and ask before running `npm uninstall --global codex-kimi-bridge`.
- Stream or tool-call failure: reproduce offline with `translate-request`, then run the relevant source tests. For a namespaced call, verify the request is flattened to a collision-safe upstream name and the response restores the original `namespace + name` in both streaming and non-streaming paths. Custom-tool streaming remains experimental.
- Recursive subagents: namespace translation remains available, but cross-provider `send_message` and `followup_task` are not a safe result channel because their content may be provider-private. Prefer automatic final-result delivery. A Kimi-created `kimi_frontend` descendant must put its exact task in `[KIMI_TASK]` markers inside the spawn message so the hook can sign it without a user-prompt cache. The bridge does not impose its own depth or concurrency limit.
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

For protocol changes, also compare both implementations against `compat/responses-request.json`. For `agent_message`, require unit coverage for text, multimodal content, route-metadata injection rejection, omission of internal passthrough metadata, omission of every unknown string and non-string `encrypted_content` value, unchanged translation when only opaque provider state changes, valid signed handoff delivery, tampered/expired/wrong-recipient rejection, empty-payload fail-closed behavior, and marked recursive handoff without a prompt cache. Test hook configuration merge/update/uninstall without touching the user's real hooks file. Require terminal assistant text to carry `phase = "final_answer"` and tool-progress text to carry `phase = "commentary"`. Report all checks and explicitly state whether a real Codex `spawn_agent`/live Kimi request was skipped.
