---
name: manage-codex-kimi-bridge
description: Operate and diagnose the zero-dependency local Kimi Code bridge used by Codex Desktop. Use when asked to start, stop, inspect, test, troubleshoot, upgrade, or configure codex-kimi-bridge; when Codex reports provider, Responses API, streaming, tool-call, authentication, or port 8787 errors; or when replacing another Kimi bridge implementation.
---

# Manage Codex Kimi Bridge

Use the installed `codex-kimi-bridge` command. Do not substitute an unrelated npm bridge or run an `npx` package with a similar name.

## Diagnose first

1. Resolve the executable with `command -v codex-kimi-bridge`.
2. Run `codex-kimi-bridge doctor --json` for read-only checks.
3. If port 8787 is occupied by an unknown process, inspect it before proposing termination. Never kill it without user authorization.
4. Run `codex-kimi-bridge doctor --live` only when the user explicitly wants a live provider test; it contacts Kimi and consumes a small amount of quota.

Never print, copy, or log the API key. The server normally receives the Bearer token from Codex. On macOS, test commands may read the Keychain service `codex-kimi-code-api-key` without displaying its value.

## Start

Run:

```sh
codex-kimi-bridge serve
```

Expected defaults:

- Local endpoint: `http://127.0.0.1:8787/v1`
- Upstream: `https://api.kimi.com/coding/v1/chat/completions`
- Model: `k3`

Keep loopback binding and HTTPS upstream defaults. Use `--allow-non-loopback` or `--allow-insecure-upstream` only after clearly explaining the exposure and receiving explicit user direction.

## Check Codex configuration

Inspect rather than rewrite configuration unless the user asks for changes. The provider must use:

```toml
[model_providers.codex_kimi_bridge]
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
```

The auth command should retrieve the Keychain item and must not embed the secret in TOML. The `kimi_frontend` agent should continue referencing `model_provider = "codex_kimi_bridge"`.

## Troubleshoot

- `missing_api_key` or HTTP 401: verify the provider auth command and Keychain item without exposing the value.
- `unsupported_tool_type`: report which Responses built-in tool cannot be translated; do not silently remove it.
- Port occupied: identify whether this bridge or the old bridge owns 8787. Ask before stopping a process.
- Stream/tool-call failure: reproduce offline with `translate-request`, then run the package tests. Treat custom-tool streaming compatibility as experimental.
- Long reasoning continuity: explain that version 0.1.0 preserves Kimi `reasoning_content` only in the current bridge process. If the process restarts during an unfinished tool chain, start a new Kimi subagent task.

After a code or configuration change, run `npm run check` and `npm test` from the bridge source directory. Report which checks ran and whether a live Kimi request was intentionally skipped.
