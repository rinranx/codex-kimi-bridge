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
- Current release: [`v0.4.0`](https://github.com/rinranx/codex-kimi-bridge/releases/tag/v0.4.0)
- Version `0.4.0` adds user-trusted Codex hooks, locally signed task envelopes, fail-closed verification, and native subagent message phases
- Recommended universal macOS kit (Apple Silicon + Intel): <https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.0/codex-kimi-bridge-macos-install-kit-0.4.0.zip>
- Apple Silicon binary: <https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.0/codex-kimi-bridge-macos-arm64-0.4.0.tar.gz>
- Intel Mac binary: <https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.0/codex-kimi-bridge-macos-x86_64-0.4.0.tar.gz>
- SHA-256 checksums: <https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.0/SHA256SUMS.txt>

Build artifacts are published only to versioned GitHub Releases; files under `main/downloads` are no longer overwritten. The current macOS binaries are not Apple-notarized. macOS may require you to right-click a `.command` file and choose Open the first time. Verify SHA-256 before installation.

## Simplest installation

If you are not comfortable with the terminal, use [Install with Codex](INSTALL-WITH-CODEX.en.md). It contains a safe prompt you can paste directly into Codex Desktop.

For manual setup:

1. Download and extract the complete installation kit.
2. Double-click `install-codex-kimi-bridge.command` to install the universal Rust binary at `~/.local/bin/codex-kimi-bridge`.
3. Follow the [complete macOS guide](install/INSTALL-GUIDE.en.md) to store the key, merge the Codex configuration, and install the subagent.
4. Choose one of the three startup methods below. For a first run, you can start with `start-codex-kimi-bridge.command`.

The installer does not modify `~/.codex/config.toml`, Keychain, or shell profiles, and it never uninstalls an old command automatically.

## Three startup methods

Choose one after installation. All three run the same Rust binary and must not compete for port 8787.

| Method | How to start | Persistence | Best for |
| --- | --- | --- | --- |
| Start from Codex | Say “Use `$manage-codex-kimi-bridge` to check and start the Rust bridge; do not run `doctor --live`.” | Usually tied to the current task or terminal session | Occasional use and zero bridge memory while stopped |
| Double-click launcher | Open `start-codex-kimi-bridge.command` | Keep the Terminal window open; stop with `Control+C` | Visible logs and manual control |
| macOS LaunchAgent | Open `install-launchagent.command` | Starts after login and recovers after an unexpected exit | Daily use and least friction |

The LaunchAgent is only configuration read by the existing macOS `launchd`; it does not add another resident manager process. Memory belongs to the same Rust bridge process. A LaunchAgent-managed bridge continues after Codex Desktop quits.

Inspect it with:

```sh
launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"
curl -s http://127.0.0.1:8787/health
```

Double-click `uninstall-launchagent.command` to remove automatic startup. It moves only the plist to Trash and preserves the binary, Codex configuration, Keychain item, and logs. The bundled LaunchAgent uses the default Kimi Code HTTPS upstream; Kimi API Open Platform users must customize `ProgramArguments` before using it.

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
- Native Codex Desktop `agent_message` conversion, opaque provider-state filtering, locally signed task handoff, and assistant message phases
- Non-streaming and SSE streaming output
- Function tools and Responses custom tools at the top level and inside `namespace`
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

### Recursive subagents and namespace tools

Version `0.3.0` supports `namespace` tools in the Codex Responses protocol. Inner `function` and `custom` tools are sent to Kimi under collision-safe upstream names, then restored to their original `namespace + name` in non-streaming responses, streaming events, and later history replay. When the current Codex version exposes the `collaboration` namespace to `kimi_frontend`, Kimi can therefore call `spawn_agent` as a native subagent, and descendant agents use the same translation path.

The bridge translates protocol messages; it does not replace the Codex scheduler:

- No additional recursion-depth limit is imposed by the bridge
- No additional child or grandchild concurrency cap is imposed by the bridge
- The bridge does not require every task to declare a stop condition
- Scheduling, UI, permissions, sandboxing, and the global thread limit remain owned by Codex Desktop

If `[agents]` defines `max_concurrent_threads_per_session`, that Codex-wide limit still applies. A `namespace` may currently contain only `function` and `custom` tools. Other hosted tool types that cannot be translated safely are rejected explicitly instead of being dropped silently.

### Native Desktop subagents and `agent_message`

Version `0.4.0` supports the Responses `agent_message` item emitted by Codex Desktop without attempting to break OpenAI provider-private state. Version `0.3.4` safely filters ciphertext but still depends on unreliable visible history. Version `0.4.0` instead uses the official Codex hook lifecycle to create a verifiable local handoff before the task enters cross-provider private wrapping.

The mapping is explicit and minimal:

- The upstream Chat Completions role is normalized to `user`
- Ordinary visible text, image, and video content remains supported; unknown `encrypted_content` is treated as opaque OpenAI provider state and omitted
- The bridge has neither OpenAI's keys nor provider context, so it never decrypts, guesses, repeats, or forwards OpenAI ciphertext
- Only a local envelope beginning with `CKB1` that passes HMAC-SHA256 verification, recipient binding, and expiry checks is restored as a Kimi-readable task; lookalikes fail explicitly
- Strictly validated `author` and `recipient` agent routes are retained in a fixed JSON metadata prefix
- The Responses item `id` and `internal_chat_message_metadata_passthrough` are not sent to Kimi, so the internal `turn_id` remains local
- Invalid agent routes that could inject extra prompt text fail explicitly with `invalid_agent_message`
- Terminal Kimi text is returned with `phase = "final_answer"`; text produced during tool work uses `phase = "commentary"` so Desktop can classify it like native transcript messages

#### Install and trust the local handoff hooks

After installing the `0.4.0` binary, run:

```sh
codex-kimi-bridge hooks install
codex-kimi-bridge hooks status --json
```

The installer merges into `~/.codex/hooks.json`, preserves unrelated hooks, and creates a timestamped backup before changing an existing file. This follows the [official Codex Hooks interface](https://learn.chatgpt.com/docs/hooks): a supported `PreToolUse` hook can rewrite the call through `updatedInput`. Current Codex Multi-Agent V2 hook input normalizes the namespaced tool to `collaborationspawn_agent` (see [OpenAI Codex #33284](https://github.com/openai/codex/issues/33284)); other paths may report `Agent`, `spawn_agent`, or `collaboration.spawn_agent`. The installer therefore uses the strict allowlist matcher `^(Agent|spawn_agent|collaborationspawn_agent|collaboration[.:_]+spawn_agent)$`. Quit and reopen Codex Desktop, enter `/hooks`, then review and trust both commands:

- `UserPromptSubmit` temporarily stores the current visible user request in a user-private cache
- `PreToolUse`, matching `Agent`, `spawn_agent`, V2's `collaborationspawn_agent`, and separator-based compatibility names, rewrites only `agent_type = "kimi_frontend"` calls into a locally signed envelope and sets `fork_turns` to `"none"`

Wrap the exact Kimi task in `[KIMI_TASK]` and `[/KIMI_TASK]` when precision matters. Without markers, the whole visible user request is used. Other agent calls are untouched. Remove only this project's managed hooks with:

```sh
codex-kimi-bridge hooks uninstall
```

If the hooks are absent or untrusted, the prompt cache is missing, the signature fails, the envelope expires, or the recipient does not match, version `0.4.0` fails before contacting Kimi. A positive `fork_turns` value with an explicit `[KIMI_TASK]` in completed history remains a compatibility fallback, not the default path.

The local envelope authenticates integrity and source; it does not encrypt for confidentiality. Visible prompts are temporarily stored with mode `0600` under `~/Library/Caches/codex-kimi-bridge/handoff-v1` and cleaned after 24 hours. The signing key is also readable only by the current macOS user. Never put an API key, password, or other secret in a handoff task.

The request remains on the native Codex `spawn_agent` path. Desktop still owns thread IDs, status, result delivery, and the panel; the bridge only converts protocol items and supplies standard message classification. Version `0.4.0` passed one user-authorized real Desktop acceptance check documented in [`compat/AGENT-MESSAGE-INTEGRATION.md`](compat/AGENT-MESSAGE-INTEGRATION.md).

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
description = "Use Kimi K3 for frontend review. A trusted local Codex hook signs the task handoff; use fork_turns=none by default."

model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"

sandbox_mode = "read-only"

developer_instructions = """
You are a senior design engineer focused on frontend experience and visual quality.

Native subagent handoff rules:
- Use the visible task body supplied in the current agent message after bridge verification
- If the task is absent or only an empty Payload shell remains, report a handoff failure immediately; do not scan the workspace and guess
- Never interpret, repeat, or guess provider-private state
- Do not use send_message or followup_task to return content to the primary agent; use ordinary commentary and a final answer
- When creating another kimi_frontend descendant, wrap the exact task in [KIMI_TASK] and [/KIMI_TASK] inside spawn_agent.message

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

web_search = "disabled"
include_apps_instructions = false

[features]
apps = false
plugins = false
remote_plugin = false
tool_search = false
image_generation = false
computer_use = false
browser_use = false
in_app_browser = false
multi_agent = true
multi_agent_v2 = true
goals = false

[mcp_servers.node_repl]
enabled = false
command = "/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node_repl"
args = []
```

The bridge maps `xhigh` to Kimi K3 `max`. The template retains `multi_agent_v2`; a trusted hook hands the task off by default, and ordinary final-result delivery returns it automatically. Cross-provider `send_message` and `followup_task` remain unreliable body channels, and recursive subagents remain experimental. You may customize the role, but keep `sandbox_mode = "read-only"` and the read-only instruction when Kimi should only advise.

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

Open Platform remains an advanced route in `0.4.0`. Release validation primarily uses a Kimi Code membership key.

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
codex-kimi-bridge hooks status --json
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
- Local handoffs use HMAC-SHA256, recipient binding, and a maximum six-hour lifetime; prompt cache and signing key files use mode `0600`
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

The shared compatibility fixture is [`compat/responses-request.json`](compat/responses-request.json). Release validation compares offline Node and Rust translations. The real-quota Desktop `spawn_agent` release gate is documented separately in [`compat/AGENT-MESSAGE-INTEGRATION.md`](compat/AGENT-MESSAGE-INTEGRATION.md) and may run only with explicit user consent.

## Known boundaries

- Responses `function` and `custom` tools are translated safely at the top level and inside `namespace`; other hosted tools fail explicitly
- Version `0.4.0` requires the user to review, trust, and enable two Codex hooks; without effective hooks, an empty task fails before Kimi is contacted
- Hooks can capture only visible user requests and cannot decrypt OpenAI provider-private state; `[KIMI_TASK]` precisely limits the handoff body
- `previous_response_id` is unsupported; callers must send full conversation items
- `parallel_tool_calls` is not forwarded
- Reasoning state exists only in the current process; start a new subagent task after a mid-chain restart
- Third-party-provider subagent scheduling remains controlled by Codex Desktop
- The macOS binaries are not yet Apple-notarized

## License

[MIT](LICENSE)
