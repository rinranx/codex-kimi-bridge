# Codex Kimi Bridge

[简体中文](README.md) | **English**

An independently implemented local bridge with zero third-party runtime dependencies. It converts the OpenAI Responses requests used by Codex Desktop into OpenAI-compatible Chat Completions requests for Kimi Code.

```text
Codex Desktop ── Responses API ──> 127.0.0.1:8787
                                      │
                                      └── HTTPS ──> api.kimi.com/coding/v1/chat/completions
```

The runtime uses only the Node.js standard library.

For a first-time installation, follow the [complete macOS installation guide](install/INSTALL-GUIDE.en.md) and use the included TOML templates. The installation package never contains an API key.

## Downloads and one-click startup

- Project: <https://github.com/rinranx/codex-kimi-bridge>
- Complete macOS installation kit: <https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-install-kit-0.1.0.zip>
- npm package: <https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-0.1.0.tgz>
- SHA-256 checksums: <https://github.com/rinranx/codex-kimi-bridge/tree/main/downloads>

After extracting the installation kit and completing the configuration, double-click `start-installed-codex-kimi-bridge.command` to start the local bridge. It invokes the globally installed `codex-kimi-bridge` command. The source tree also includes `start-codex-kimi-bridge.command`, which can run the development copy without a global installation.

## Beginner setup: let Codex install it

If you would rather not merge TOML or run every command manually, open [Install with Codex](INSTALL-WITH-CODEX.en.md) and paste its complete prompt into Codex Desktop. Codex will read the project documentation first, then configure the installation for your key type and membership tier.

This is a secure assisted installation, not a fully unattended one. You still approve narrowly scoped writes and personally type the API key into the macOS Keychain prompt. Never send the key through chat.

## Implemented features

- Responses text, image, and video input conversion
- Non-streaming and SSE streaming output
- Function tools and Responses custom tools
- Preservation of Kimi `reasoning_content` across multi-turn tool calls
- `low` / `high` / `max` reasoning-effort mapping
- JSON Object and JSON Schema output formats
- Kimi Code Plan `prompt_cache_key`
- Upstream API error passthrough with a stable local error format
- Loopback-only binding by default and HTTPS-only upstreams by default
- No logging of request bodies, API keys, or reasoning content

## Requirements

- macOS, Linux, or Windows
- Node.js 20 or later; the current development and test environment uses Node.js 26
- A Kimi Code API key. Multiple membership tiers are supported; select a model available to your plan using the table below.

No `npm install` is required inside the source directory. The project has zero runtime dependencies.

## Confirm that Codex Multi-agent v2 is enabled

This project uses Codex multi-agent support to schedule `kimi_frontend` as a custom subagent. Before continuing, confirm that Multi-agent v2 is enabled:

1. Open Codex Desktop settings.
2. Find **Multi-agent v2** under “Experimental Features” or “Features.”
3. If the option is present but disabled, enable it.
4. Quit Codex Desktop completely, then reopen it.

Newer Codex versions may enable Multi-agent v2 by default and mark it as stable. If no toggle is shown but Codex can already display and schedule subagents, no additional change is required.

You can also check the effective state in a terminal:

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

Both `multi_agent` and `multi_agent_v2` should show `true`. With an older Codex version or a manually managed configuration, add the following to `~/.codex/config.toml`:

```toml
[features]
multi_agent_v2 = true
```

If `[features]` already exists, add only `multi_agent_v2 = true`; do not create a second `[features]` table. The `[agents] enabled = true` setting shown later is also required. The two settings serve different purposes.

## Optional: install as a global command

Run the following from the project directory:

```sh
npm install --global .
codex-kimi-bridge doctor --json
codex-kimi-bridge serve
```

You can then use `codex-kimi-bridge` from any directory. If you prefer not to install it globally, continue using the `.command` launcher in the source directory.

## Codex configuration

Use the following Codex provider structure:

```toml
[agents]
enabled = true

[model_providers.codex_kimi_bridge]
name = "Kimi Code K3 via Codex Kimi Bridge"
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

Subagent configuration—the following is the tested example for an Allegretto membership using K3 with a 1M-token context window:

```toml
model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
sandbox_mode = "read-only"
```

The bridge maps `xhigh` to Kimi K3 `max`.

## Choose a Kimi Code model for your membership tier

The `k3` 1M configuration above is not a requirement for installing the bridge. It is the Allegretto configuration that has been tested end to end by this project. All Kimi Code membership tiers use the same upstream endpoint and the same type of Kimi Code API key:

```text
https://api.kimi.com/coding/v1/chat/completions
```

The differences are the models and context windows available to each membership tier:

| Kimi membership tier | Recommended model ID | Context window | Best for |
| --- | --- | ---: | --- |
| Andante / all Kimi Code members | `kimi-for-coding` | `262144` | Routine development and code completion |
| Moderato | `k3-256k` | `262144` | Recommended; K3-equivalent results within 256K with lower quota usage |
| Moderato | `k3` | `262144` | K3 without 1M-context access |
| Allegretto and above | `k3` | `1048576` | Large repositories and long-context tasks |
| Allegretto and above | `kimi-for-coding-highspeed` | `262144` | Faster model output |

Refer to the [official Kimi Code model configuration](https://www.kimi.com/code/docs/en/kimi-code/models.html) for current model permissions and context limits. The official documentation states that `k3-256k` produces the same results as `k3` within a 256K context, while the 1M-context `k3` consumes roughly twice as much quota.

After selecting a model, change the corresponding lines in `~/.codex/agents/kimi_frontend.toml`.

### Andante / general membership configuration

```toml
model = "kimi-for-coding"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model kimi-for-coding
```

### Recommended Moderato configuration

```toml
model = "k3-256k"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model k3-256k
```

### Allegretto and above: K3 1M

```toml
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
```

```sh
codex-kimi-bridge serve --model k3
```

### Allegretto and above: K2.7 HighSpeed

```toml
model = "kimi-for-coding-highspeed"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model kimi-for-coding-highspeed
```

`serve --model` sets the bridge’s default model and the model displayed by its health endpoint. The model actually sent by the Codex subagent comes from `model` in `kimi_frontend.toml`. Keep both values synchronized when switching models. Then restart the bridge and either restart Codex Desktop or open a new task to avoid reusing the previous session’s model cache. When running a live diagnostic, also pass the same model to `doctor --live --model <model-id>`.

You can switch among the models permitted by your current Kimi Code membership using the same Kimi Code key. Switching models does not require a new key.

## No Kimi Code membership: pay-as-you-go API (advanced)

Kimi Code membership keys and Kimi API Open Platform keys belong to separate products. Their keys, model IDs, and endpoints are not interchangeable. See the [official Kimi API troubleshooting guide](https://www.kimi.com/help/kimi-api/api-troubleshooting).

For an international Open Platform key:

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

For a mainland China Open Platform key:

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.cn/v1/chat/completions \
  --model kimi-k3
```

Use the following `kimi_frontend.toml` settings:

```toml
model = "kimi-k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
```

Store the Open Platform key in the same local Keychain item. `codex-kimi-code-api-key` is only the local item name; it does not determine the key type:

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

> [!IMPORTANT]
> The complete test and release-validation path for `codex-kimi-bridge 0.1.0` uses a Kimi Code membership key. Kimi Open Platform provides a compatible Chat Completions interface, and the bridge supports custom upstreams and models, but this route should be treated as an advanced and experimental configuration in version 0.1.0. Before relying on it, run a small test with the same key, regional endpoint, and model you intend to use.

## Replace the API key

The bridge does not store the key. It only forwards the Bearer token supplied with each Codex request. Update Keychain without changing the source code:

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

Keep `-w` as the final argument. The terminal will securely prompt for the new key, so the key is not written to shell history. Restart Codex Desktop or start a new task after the update.

## Commands

```sh
codex-kimi-bridge serve
codex-kimi-bridge doctor --json
codex-kimi-bridge doctor --live --json
codex-kimi-bridge translate-request --file fixtures/responses-request.json
codex-kimi-bridge request "Reply with OK only."
codex-kimi-bridge --help
```

`translate-request` is fully offline and can be used to inspect the converted request. `request` sends a request through the running local bridge and reads the test key from `KIMI_CODE_API_KEY` or the macOS Keychain.

## Security design

- Binds to `127.0.0.1` by default; binding to a non-loopback interface requires the explicit `--allow-non-loopback` flag
- Allows HTTPS upstreams by default; plain HTTP is accepted only for an explicitly configured loopback test server
- Refuses upstream redirects to reduce the risk of credential forwarding
- Logs only sanitized status and error codes
- Keeps Kimi reasoning state in memory only, capped at 512 entries or 64 MiB and evicted after two hours; all state is lost when the process exits

See [SECURITY.md](SECURITY.md) for details.

## Verification

```sh
npm run check
npm test
npm run smoke
```

Tests cover request conversion, SSE, function/custom tools, authentication, privacy logging, and a two-turn tool-call flow. The development sandbox cannot bind a local port, so automated end-to-end tests directly invoke the same request handler used by the HTTP server and supply a simulated Kimi response. On a normal machine, use `/health` and `doctor --live` for additional local-port and upstream checks.

## Known limitations

- Safely converts only Responses `function` and `custom` tools. OpenAI-hosted tools such as `web_search` and `file_search` produce explicit errors instead of being silently removed.
- Does not support `previous_response_id`; callers must send the complete conversation items, as Codex does.
- Does not forward `parallel_tool_calls` because the Kimi Chat API documentation does not declare that request field.
- Multi-turn tool-call reasoning state is process-local. If the bridge restarts during an unfinished tool chain, start a new Kimi subagent task.
- Whether Codex Desktop schedules a third-party provider as a Multi-agent v2 subagent is controlled by Codex itself. The bridge handles protocol compatibility only and cannot bypass Desktop scheduling restrictions.

## Optional companion skill

The project includes the [`manage-codex-kimi-bridge`](companion-skill/manage-codex-kimi-bridge/SKILL.md) skill. After installing the global command, copy that skill directory to `~/.codex/skills/` to let Codex diagnose and start the bridge. The skill is not required for the bridge to run.

## Uninstall or roll back

Press `Control-C` in the bridge terminal to stop it. If you installed the global command, run:

```sh
npm uninstall --global codex-kimi-bridge
```

This does not delete the Keychain item or modify the Codex configuration.
