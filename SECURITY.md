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

## Release artifacts

Release downloads include SHA-256 checksums. The `0.3.0` macOS binaries are not Apple-notarized; verify the checksum before bypassing a Gatekeeper warning. The universal installation kit contains the same Rust binary for Apple Silicon and Intel Macs.

## Reporting

When reporting an issue, include the bridge version and implementation from `/health`, macOS architecture, sanitized HTTP status, and stable error code. Never attach API keys, Authorization headers, private prompts, raw request bodies, or reasoning content.
