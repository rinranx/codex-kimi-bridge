# Codex Kimi Bridge

[简体中文](README.md) | **English**

A local single-binary Rust bridge that converts OpenAI Responses requests from Codex Desktop into OpenAI-compatible Kimi Code or Kimi API Chat Completions requests.

```text
Codex Desktop ── Responses API ──> 127.0.0.1:8787
                                      │
                                      └── HTTPS ──> Kimi Chat Completions
```

Rust is the default implementation starting with `0.2.0-alpha.1`. Users do not need Rust, Node.js, or npm. The original Node.js implementation has been renamed to `codex-kimi-bridge-node` and is preserved in [`node/`](node/) as a fallback.

## Downloads

- Project: <https://github.com/rinranx/codex-kimi-bridge>
- Recommended universal macOS kit (Apple Silicon + Intel): <https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-install-kit-0.2.0-alpha.1.zip>
- Apple Silicon binary: <https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-arm64-0.2.0-alpha.1.tar.gz>
- Intel Mac binary: <https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-x86_64-0.2.0-alpha.1.tar.gz>
- SHA-256 checksums: <https://github.com/rinranx/codex-kimi-bridge/tree/main/downloads>

This is the first Rust alpha and is not Apple-notarized. macOS may require you to right-click a `.command` file and choose Open the first time. Verify SHA-256 before installation.

## Simplest installation

If you are not comfortable with the terminal, use [Install with Codex](INSTALL-WITH-CODEX.en.md). It contains a safe prompt you can paste directly into Codex Desktop.

For manual setup:

1. Download and extract the complete installation kit.
2. Double-click `install-codex-kimi-bridge.command` to install the universal Rust binary at `~/.local/bin/codex-kimi-bridge`.
3. Follow the [complete macOS guide](install/INSTALL-GUIDE.en.md) to store the key, merge the Codex configuration, and install the subagent.
4. Double-click `start-codex-kimi-bridge.command` to start the local bridge.

The installer does not modify `~/.codex/config.toml`, Keychain, or shell profiles, and it never uninstalls an old command automatically.

## Migrating from Node 0.1.0

The old npm package and the Rust default both use the command name `codex-kimi-bridge`. Stop the old bridge and remove the old global npm command before installing Rust:

```sh
npm uninstall --global codex-kimi-bridge
```

Then install the Rust binary. The provider URL, Keychain service, and `kimi_frontend.toml` remain unchanged. To fall back temporarily:

```sh
cd node
npm install --global .
codex-kimi-bridge-node serve
```

The Rust and Node fallback implementations cannot listen on port 8787 simultaneously.

## Implemented features

- Responses text, image, and video conversion
- Non-streaming and SSE streaming output
- Function tools and Responses custom tools
- In-memory preservation of Kimi `reasoning_content` across tool calls
- `low` / `high` / `max` reasoning-effort mapping
- JSON Object and JSON Schema output
- Kimi Code Plan `prompt_cache_key`
- Upstream error passthrough with stable local errors
- Loopback-only binding and HTTPS-only upstream defaults
- Rejection of upstream redirects and credential-bearing upstream URLs
- No logging of request bodies, API keys, or reasoning content
- A single macOS binary with no external runtime

## Enable Codex Multi-agent v2

The project schedules `kimi_frontend` as a custom Codex subagent:

1. Open Codex Desktop settings.
2. Enable **Multi-agent v2** under Experimental Features or Features.
3. Quit Codex Desktop completely, then reopen it.

Newer versions may enable it by default. If no toggle is visible but Codex can display and schedule subagents, no change is required.

You can also inspect the state:

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

When manual configuration is required, add the key to the existing table:

```toml
[features]
multi_agent_v2 = true
```

Do not create a duplicate `[features]` table.

## Codex provider configuration

Safely merge this into the user-level `~/.codex/config.toml`. Do not overwrite unrelated settings or put the key in TOML:

```toml
[agents]
enabled = true

[model_providers.codex_kimi_bridge]
name = "Kimi via Codex Kimi Bridge"
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
stream_idle_timeout_ms = 900000
request_max_retries = 1
stream_max_retries = 1

[model_providers.codex_kimi_bridge.auth]
command = "/usr/bin/security"
args = [
  "find-generic-password",
  "-s",
  "codex-kimi-code-api-key",
  "-w"
]
timeout_ms = 5000
refresh_interval_ms = 0
```

## Complete `kimi_frontend` example

This is the complete Allegretto + K3 1M template. The source file is [`install/templates/kimi_frontend.toml`](install/templates/kimi_frontend.toml):

```toml
name = "kimi_frontend"
description = "Use Kimi K3 to review and improve frontend visuals, interactions, responsive layouts, and implementation plans."

model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
model_supports_reasoning_summaries = false

sandbox_mode = "read-only"

developer_instructions = """
You are a senior design engineer focused on frontend experience and visual quality.

Focus areas:
- Review visual hierarchy, typography, spacing, color, and information density
- Review component consistency and the design system
- Review responsive layouts on desktop and mobile
- Review interaction feedback, motion, accessibility, and user flows
- Use the existing code to assess implementation cost and maintenance risk
- When screenshots are available, evaluate them together with the code

Output structure:
1. Issues that must be fixed
2. Major issues affecting the experience
3. Specific recommended improvements
4. An implementation checklist for the primary agent

Remain read-only by default and do not modify files directly.
Be specific; avoid abstract recommendations such as only saying “more modern” or “more polished.”
Return a concise, actionable summary to the primary agent when finished.
"""
```

The bridge maps `xhigh` to Kimi K3 `max`. You may customize the role, but keep `sandbox_mode = "read-only"` and the read-only instruction when Kimi should only advise.

## Choose a Kimi Code model by membership tier

All Kimi Code memberships use the same key type and upstream:

```text
https://api.kimi.com/coding/v1/chat/completions
```

| Kimi membership | Recommended model | Context | Auto compact | Best for |
| --- | --- | ---: | ---: | --- |
| Andante / all members | `kimi-for-coding` | `262144` | `230000` | Routine development |
| Moderato | `k3-256k` | `262144` | `230000` | Recommended; lower quota usage |
| Moderato | `k3` | `262144` | `230000` | K3 without 1M access |
| Allegretto and above | `k3` | `1048576` | `900000` | Large repositories and long context |
| Allegretto and above | `kimi-for-coding-highspeed` | `262144` | `230000` | Faster output |

Refer to the [official Kimi Code model configuration](https://www.kimi.com/code/docs/en/kimi-code/models.html) for current permissions.

When switching models, update `~/.codex/agents/kimi_frontend.toml` and start the bridge with the same model:

```sh
codex-kimi-bridge serve --model k3-256k
```

Restart the bridge and reopen Codex Desktop or create a new task. A live diagnostic must use the same model:

```sh
codex-kimi-bridge doctor --live --json --model k3-256k
```

That command contacts Kimi and consumes a small amount of quota.

## Kimi API Open Platform key (advanced)

Kimi Code membership keys and Kimi API Open Platform keys are separate products. Their keys, model IDs, and endpoints are not interchangeable.

International Open Platform:

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

Mainland China Open Platform:

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.cn/v1/chat/completions \
  --model kimi-k3
```

Open Platform remains an advanced route in `0.2.0-alpha.1`. Release validation primarily uses a Kimi Code membership key.

## Store or replace the API key

Store the key only in macOS Keychain:

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

Keep `-w` last and type the key directly into the terminal prompt. Never send it through chat, screenshots, or configuration files. Run the same command to replace the key, then restart Codex Desktop or create a new task.

## CLI

```sh
codex-kimi-bridge --version
codex-kimi-bridge serve
codex-kimi-bridge doctor --json
codex-kimi-bridge translate-request --file compat/responses-request.json
codex-kimi-bridge request "Reply with OK only."
```

- `translate-request` is fully offline.
- `doctor --json` does not contact Kimi by default.
- `doctor --live` and `request` read the local key and make real requests.

## Security design

- Default bind is `127.0.0.1`; external binding requires `--allow-non-loopback`
- HTTPS upstreams are required by default
- Plain HTTP is limited to explicit loopback test servers
- Upstream URLs cannot contain embedded credentials
- Redirects are rejected to prevent credential forwarding
- Request bodies, Authorization, keys, and reasoning are never logged
- The reasoning cache stays in memory, is limited to 512 entries or 64 MiB, and expires after two hours
- The `request` command sends credentials only to loopback URLs

See [SECURITY.md](SECURITY.md).

## Development and validation

Rust default:

```sh
cd rust
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo build --release
```

Node fallback:

```sh
cd node
npm run check
npm test
npm run smoke
```

The shared compatibility fixture is [`compat/responses-request.json`](compat/responses-request.json). Release validation compares offline Node and Rust translations.

## Known boundaries

- Only Responses `function` and `custom` tools are translated safely; hosted tools fail explicitly
- `previous_response_id` is unsupported; callers must send full conversation items
- `parallel_tool_calls` is not forwarded
- Reasoning state exists only in the current process; start a new subagent task after a mid-chain restart
- Third-party-provider subagent scheduling remains controlled by Codex Desktop
- The Rust alpha binary is not yet Apple-notarized

## License

[MIT](LICENSE)

