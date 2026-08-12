# Security: Node Fallback

This file applies to the separately named `codex-kimi-bridge-node 0.3.0` fallback source. The repository default is the Rust `codex-kimi-bridge` implementation described by the root security policy.

## Secret handling

The bridge does not persist API keys. Codex supplies a Bearer token per request and the bridge forwards it to the configured HTTPS upstream. Request bodies, credentials, and Kimi reasoning content are never logged.

The `request` and `doctor --live` commands may read `KIMI_CODE_API_KEY` or the macOS Keychain service `codex-kimi-code-api-key`. They never print the secret.

## Network boundaries

- Default bind: `127.0.0.1:8787`
- Default upstream: `https://api.kimi.com/coding/v1/chat/completions`
- Non-loopback binding requires `--allow-non-loopback`
- Plain HTTP requires both a loopback upstream and `--allow-insecure-upstream`
- Upstream redirects are rejected
- Browser CORS access is not enabled

Do not expose the bridge through a public reverse proxy. It authenticates callers only by requiring a Bearer token and is designed for a single local Codex process.

## In-memory reasoning state

Kimi requires `reasoning_content` to be preserved across multi-step tool calls. The bridge keeps this state only in process memory, keyed by tool call ID. The cache is bounded to 512 entries and 64 MiB, expires after two hours, and is cleared when the process exits.

## Inter-agent metadata

The `0.3.0` fallback source converts Codex `agent_message` items to upstream `user` messages. Only strictly validated `author` and `recipient` routes are retained in a fixed metadata prefix. The Responses item ID and `internal_chat_message_metadata_passthrough`, including internal turn IDs, are deliberately omitted from the upstream request. Invalid route metadata is rejected rather than sanitized into prompt text.

Unknown `agent_message.content[].encrypted_content` is opaque OpenAI provider state. Version `0.3.0` omits it and never attempts to decrypt, reinterpret, or forward it. The only accepted exception is a `CKB1` envelope that passes HMAC-SHA256, recipient, agent-type, and expiry checks using the same local key format as Rust `0.4.0`. Prompt caches and the key use private filesystem permissions, but HMAC is not encryption; do not put secrets in a task. An empty payload without a verified envelope or explicit visible-history fallback fails locally. Terminal assistant text is labeled `final_answer`; tool-progress text is labeled `commentary` for native transcript classification.

## Reporting

When reporting an issue, include `codex-kimi-bridge-node` version, Node.js version, sanitized HTTP status, and error code. Never attach API keys, full Authorization headers, private prompts, or raw reasoning content.
