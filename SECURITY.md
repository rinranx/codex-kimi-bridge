# Security

This policy describes the default Rust `codex-kimi-bridge` implementation. The separately named Node fallback follows the same network and secret-handling boundaries; its implementation-specific notes are in [`node/SECURITY.md`](node/SECURITY.md).

## Secret handling

The server does not persist API keys. Codex supplies a Bearer token with each Responses request, and the bridge forwards it to the configured upstream. Request bodies, credentials, Authorization headers, and Kimi reasoning content are never logged.

The explicit `request` and `doctor --live` commands may read `KIMI_CODE_API_KEY` or the macOS Keychain service `codex-kimi-code-api-key`. They never print the secret. `request` refuses to send credentials to a non-loopback bridge URL.

## Network boundaries

- Default bind: `127.0.0.1:8787`
- Default upstream: `https://api.kimi.com/coding/v1/chat/completions`
- A non-loopback bind requires `--allow-non-loopback`
- Plain HTTP is accepted only for an explicit loopback test upstream with `--allow-insecure-upstream`
- Upstream URLs containing a username or password are rejected
- Upstream redirects are rejected to avoid credential forwarding
- Browser CORS access is not enabled

Do not expose the bridge through a public reverse proxy. It is designed for one local Codex installation and is not a public authentication gateway.

## In-memory reasoning state

Kimi requires `reasoning_content` across multi-step tool calls. The bridge keeps it only in process memory, keyed by tool call ID. The cache is bounded to 512 entries and 64 MiB, expires after two hours, and is cleared on process exit.

## Inter-agent metadata and local handoff

Version `0.4.1` converts Codex `agent_message` items to upstream `user` messages. Only strictly validated `author` and `recipient` routes are retained in a fixed metadata prefix. The Responses item ID and `internal_chat_message_metadata_passthrough`, including internal turn IDs, are deliberately omitted from the upstream request. Invalid route metadata is rejected rather than sanitized into prompt text.

Unknown `agent_message.content[].encrypted_content` is opaque OpenAI provider state. A third-party bridge does not have the keys or provider context needed to decrypt it. The bridge omits every unknown value, whether it is a string or another JSON type; it never infers meaning from byte equality, reinterprets provider ciphertext as text, or forwards it to Kimi.

Version `0.4.0` adds a separate local format, `CKB1`, created only by user-trusted Codex Hooks. `UserPromptSubmit` stores the visible task locally. `PreToolUse` rewrites only `agent_type = "kimi_frontend"`, binds the task to the child `task_name`, and signs the payload with HMAC-SHA256. The bridge accepts it only when the signature, fixed agent type, recipient, creation time, and expiry all verify. Tampered, expired, wrong-recipient, or unverifiable envelopes fail before an upstream request. A valid envelope may be retried until it expires, so it is an authenticated handoff rather than a one-time token.

Version `0.4.1` adds a fail-closed follow-up guard. After a signed Kimi spawn, the hook stores a hash-named record binding the parent `session_id` to the child `task_name`. A later `send_message` or `followup_task` targeting that registered child is denied before delivery because Codex may otherwise expose only an opaque provider-private body to the child. The guard does not inspect or store the follow-up message and does not affect unregistered or non-Kimi targets. Registrations expire after six hours. Canonical task paths are supported; an opaque agent ID that the spawn hook never observed cannot be mapped safely and still falls back to the bridge's `unsupported_cross_provider_followup` error if delivered.

HMAC supplies authenticity and integrity, not confidentiality. Prompt cache files, target records, and the signing key use mode `0600` inside a mode-`0700` directory under `~/Library/Caches/codex-kimi-bridge/handoff-v1`; stale files are cleaned after 24 hours and envelopes/target registrations live for six hours. Any process running as the same macOS user may be able to read those files. Do not place API keys, passwords, or other secrets in a delegated task.

The hook installer merges with existing `~/.codex/hooks.json`, creates a timestamped backup before changing an existing file, and marks only its own two entries for later removal. Codex still requires the user to review and trust non-managed hooks through `/hooks`. Do not bypass that trust step.

If an initial `agent_message` contains only an empty payload shell and no verified local envelope or explicit visible-history fallback, conversion fails with `missing_handoff_envelope`. If the shell is a later `MESSAGE`, conversion instead fails with `unsupported_cross_provider_followup`, making clear that reinstalling an initial handoff alone is not the immediate remedy. Neither error contacts Kimi. Responses assistant messages include only the transcript classification values `commentary` and `final_answer`; the bridge does not invent thread IDs or Desktop state.

## Release artifacts

Release downloads include SHA-256 checksums. The `0.4.1` macOS binaries are not Apple-notarized; verify the checksum before bypassing a Gatekeeper warning. The universal installation kit contains the matching Rust binary for Apple Silicon and Intel Macs.

## Reporting

When reporting an issue, include the bridge version and implementation from `/health`, macOS architecture, sanitized HTTP status, and stable error code. Never attach API keys, Authorization headers, private prompts, raw request bodies, or reasoning content.
