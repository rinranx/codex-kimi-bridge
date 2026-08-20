# Codex Kimi Bridge: Complete macOS Installation Guide

[简体中文](INSTALL-GUIDE.zh-CN.md) | **English**

This guide installs the default single-binary Rust implementation, `codex-kimi-bridge 0.4.1`. New users do not need Rust, Node.js, or npm. The Node implementation remains only as a fallback under [`node/`](../node/).

> The current stable release is `v0.4.1`. Download and verify only that version's GitHub Release; do not substitute `main/downloads`, a similarly named package, or an older build.

## 1. Requirements

You need macOS, Codex Desktop, a Kimi Code membership key or Kimi API Open Platform key, and Codex **Multi-agent v2**. Never paste the key into chat, screenshots, or TOML.

## 2. Download and verify

Download the recommended kit:

<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.1/codex-kimi-bridge-macos-install-kit-0.4.1.zip>

Download [`INSTALL-KIT-SHA256.txt`](https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.1/INSTALL-KIT-SHA256.txt) from the same Release and compare it:

```sh
shasum -a 256 codex-kimi-bridge-macos-install-kit-0.4.1.zip
```

This release is not Apple-notarized. After verifying the checksum, you may need to right-click a `.command` file and choose Open.

## 3. One-time migration from Node 0.1.0

New users can skip this section. Stop the old bridge with `Control+C`, then remove its conflicting command:

```sh
npm uninstall --global codex-kimi-bridge
command -v codex-kimi-bridge
```

The second command should no longer resolve to the old npm installation. Keep the Keychain item, provider configuration, and `kimi_frontend.toml`; all remain compatible.

## 4. Install the Rust binary

Extract the kit and double-click `install-codex-kimi-bridge.command`. It installs:

```text
~/.local/bin/codex-kimi-bridge
```

The installer verifies the version, refuses a conflicting command at another location, and asks before updating an existing target while preserving a backup. It never changes Codex configuration, Keychain, shell profiles, or uninstalls software.

Verify it:

```sh
$HOME/.local/bin/codex-kimi-bridge --version
```

Expected: `0.4.1`.

Optionally add this line to `~/.zprofile` and open a new terminal:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

The bundled double-click launcher does not require that PATH change.

## 5. Enable Multi-agent v2

In Codex Desktop, open Settings → Experimental Features or Features, enable **Multi-agent v2**, then quit and reopen Codex Desktop.

You may also inspect the state:

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

When manual configuration is required, merge this key into the existing `[features]` table in `~/.codex/config.toml`; never create a duplicate table:

```toml
[features]
multi_agent_v2 = true
```

If the toggle is absent but Codex can already display and schedule custom subagents, no extra entry is needed.

## 6. Choose the key, model, and upstream

### Kimi Code membership keys

All membership keys use:

```text
https://api.kimi.com/coding/v1/chat/completions
```

Set these three fields in `~/.codex/agents/kimi_frontend.toml` according to actual access:

| Membership | `model` | `model_context_window` | `model_auto_compact_token_limit` |
| --- | --- | ---: | ---: |
| Andante / all members | `kimi-for-coding` | `262144` | `230000` |
| Moderato, quota-saving | `k3-256k` | `262144` | `230000` |
| Moderato, K3 | `k3` | `262144` | `230000` |
| Allegretto and above | `k3` | `1048576` | `900000` |
| Allegretto and above, high speed | `kimi-for-coding-highspeed` | `262144` | `230000` |

The bundled template defaults to Allegretto `k3` with 1M context. Change all three fields if your access differs.

### Kimi API Open Platform keys

Open Platform and membership keys are different products; do not mix their keys, models, or endpoints. For example:

```sh
$HOME/.local/bin/codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

For the mainland China platform, use `https://api.moonshot.cn/v1/chat/completions`. Change the agent model to one enabled for your account. Open Platform is an advanced route; follow the current model list in your official Kimi console.

## 7. Store the key in macOS Keychain

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

Keep `-w` last and enter the key only at the terminal's secure prompt. The service name remains `codex-kimi-code-api-key` for both implementations. Run the same command to replace the key, then restart Codex Desktop or create a new task.

## 8. Merge the Codex provider configuration

If the configuration already exists, back it up first:

```sh
if test -f "$HOME/.codex/config.toml"; then
  cp "$HOME/.codex/config.toml" "$HOME/.codex/config.toml.backup.$(date +%Y%m%d-%H%M%S)"
fi
```

Read the existing file, then merge `templates/config-kimi-provider.toml`. Do not overwrite unrelated settings or duplicate `[features]`, `[agents]`, or provider tables. The core provider is:

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

The auth command gives the key to Codex without putting it in TOML.

## 9. Install the `kimi_frontend` subagent

```sh
mkdir -p "$HOME/.codex/agents"
cp templates/kimi_frontend.toml "$HOME/.codex/agents/kimi_frontend.toml"
```

Apply the three model settings from section 6. The full template is read-only by default and returns actionable frontend visual, interaction, responsive-layout, accessibility, and implementation-cost guidance to the primary agent.

## 10. Install the management skill (optional)

```sh
mkdir -p "$HOME/.codex/skills"
cp -R companion-skill/manage-codex-kimi-bridge "$HOME/.codex/skills/"
```

The skill manages and diagnoses the bridge later; it is not the bridge executable.

## 11. Install and trust the task-handoff hooks

Run:

```sh
$HOME/.local/bin/codex-kimi-bridge hooks install
$HOME/.local/bin/codex-kimi-bridge hooks status --json
```

The installer merges only two marked hook entries into `~/.codex/hooks.json`, preserves unrelated hooks, and creates a timestamped backup before changing an existing file. Quit and reopen Codex Desktop, enter `/hooks`, then review and trust:

- `UserPromptSubmit` → `codex-kimi-bridge hook user-prompt-submit`
- `PreToolUse`, strictly matching ordinary, flattened, and separator-based names for initial spawn plus `send_message` / `followup_task` → `codex-kimi-bridge hook pre-tool-use`

Do not bypass hook trust. Visible tasks and hash-named Kimi-target registrations are cached with mode `0600` under the mode-`0700` directory `~/Library/Caches/codex-kimi-bridge/handoff-v1`; registrations expire after six hours and stale files are cleaned after 24 hours. A `CKB1` envelope uses HMAC-SHA256 to authenticate source, integrity, recipient, and lifetime; it is not confidential encryption. Never put an API key, password, or other secret in a task.

Run `codex-kimi-bridge hooks uninstall` to remove only this project's two marked commands.

## 12. Start and verify

Choose one startup method after installation:

| Method | Action | Characteristics |
| --- | --- | --- |
| Codex on demand | Tell Codex, “Use `$manage-codex-kimi-bridge` to check and start the Rust bridge; do not run `doctor --live`.” | Can use zero bridge memory while stopped, but usually depends on the current task or terminal session |
| Visible Terminal | Double-click `start-codex-kimi-bridge.command` | Visible logs; stop with `Control+C` |
| Login-time LaunchAgent | Double-click `install-launchagent.command` | Background service with recovery after an unexpected exit; best for daily use |

All three run the same Rust binary; never start competing listeners. A LaunchAgent is configuration for the existing macOS `launchd`, not another resident manager process.

### Method A: start from Codex

Send this in Codex Desktop:

```text
Use $manage-codex-kimi-bridge to check and start the Rust bridge. Run local health checks only; do not run doctor --live.
```

### Method B: double-click the launcher

For the default Kimi Code setup, double-click `start-codex-kimi-bridge.command`. For another Kimi Code model, start from a terminal, for example:

```sh
$HOME/.local/bin/codex-kimi-bridge serve --model k3-256k
```

The window should show `implementation: rust`. Keep it open; press `Control+C` to stop.

### Method C: install the macOS LaunchAgent

Double-click `install-launchagent.command`. It creates and loads:

```text
~/Library/LaunchAgents/io.github.rinranx.codex-kimi-bridge.plist
```

It starts after login and is recovered by `launchd` after an unexpected exit. Standard logs are written to `~/Library/Logs/codex-kimi-bridge.log`; sanitized errors go to `~/Library/Logs/codex-kimi-bridge.error.log`. Request bodies and credentials are not logged.

Inspect it with:

```sh
launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"
```

The bundled template uses the default Kimi Code upstream. Kimi API Open Platform users must first add the correct `--upstream` and `--model` to `ProgramArguments`, or choose one of the first two methods.

Double-click `uninstall-launchagent.command` to stop it and move only the plist to Trash. The binary, Codex configuration, Keychain item, and logs remain.

### Health checks

In another terminal, run local-only checks:

```sh
curl -s http://127.0.0.1:8787/health
$HOME/.local/bin/codex-kimi-bridge doctor --json
```

Health should identify `service: codex-kimi-bridge`, `implementation: rust`, and version `0.4.1`. The default doctor command does not contact Kimi. Only with explicit consent to consume a small amount of quota, run:

```sh
$HOME/.local/bin/codex-kimi-bridge doctor --live --json --model k3
```

Restart Codex Desktop. Use this signed handoff for a native Kimi child thread:

1. The current user request or recent completed history must already contain the full task. Prefer sending:

```text
[KIMI_TASK]
Put the complete task, input locations, output requirements, and stop condition here.
[/KIMI_TASK]
```

2. Spawn `kimi_frontend` with `fork_turns = "none"`. The trusted `PreToolUse` hook signs the marked task and binds it to that child's `task_name`.
3. After the child starts, use only `wait_agent` and let Kimi return an ordinary final answer automatically. Do not call `send_message` or `followup_task` on the running Kimi child; version `0.4.1` denies those calls for a registered Kimi target before delivery.
4. To add instructions, submit a new complete visible `[KIMI_TASK]` and create a newly named `kimi_frontend`; do not follow up on the old child.

You can send: “`[KIMI_TASK]` (put the complete task here) `[/KIMI_TASK]`; spawn `kimi_frontend` with `fork_turns=none` and wait for its ordinary final answer.”

If hooks are unavailable, an explicit `[KIMI_TASK]` in completed history plus the smallest positive `fork_turns` remains a less reliable compatibility fallback. When Kimi creates another `kimi_frontend` descendant, it should put the exact descendant task in `[KIMI_TASK]` markers inside `spawn_agent.message`.

## 13. Troubleshooting

- **Port 8787 is occupied:** identify the owner first; never terminate an unknown process. Stop an old bridge window with `Control+C`. Rust and Node cannot listen together.
- **Version 0.1.0 still starts:** inspect `command -v codex-kimi-bridge`, uninstall the old npm package, and use the explicit `~/.local/bin` path.
- **Command not found:** use the full path or add `~/.local/bin` to PATH as described above.
- **macOS blocks the launcher:** verify SHA-256, then right-click and choose Open.
- **401 / missing_api_key:** inspect the Keychain service name and provider auth command without displaying the secret.
- **Model access error:** update all three agent model fields for your membership and create a new task.
- **The child runs but Kimi progress is not visible:** confirm `0.4.1` Responses assistant messages include `phase`; terminal text must be `final_answer` and tool-progress text `commentary`. Older builds omitted this field. Do not patch the Desktop app.
- **Initial `missing_handoff_envelope` / empty payload:** require version `0.4.1`, run `hooks status --json`, and confirm both commands are trusted in Desktop `/hooks`. Never bypass verification by forwarding raw provider ciphertext.
- **`unsupported_cross_provider_followup`:** the parent sent an opaque provider-private follow-up to an existing Kimi child. Stop retrying and wait for automatic completion. If new content is necessary, submit a new visible `[KIMI_TASK]` and create a new child. If the bridge emitted this instead of the hook denying the tool, rerun `hooks install`, restart Desktop, and review/trust the updated hook.
- **LaunchAgent did not start:** run `launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"` and inspect `~/Library/Logs/codex-kimi-bridge.error.log`.
- **LaunchAgent restarts repeatedly:** port 8787 is usually occupied. Identify the owner first; never terminate an unknown process.
- **Rust compatibility problem:** stop Rust and follow [`node/install/INSTALL-GUIDE.en.md`](../node/install/INSTALL-GUIDE.en.md) for the separately named Node fallback.

## 14. Uninstall

If a LaunchAgent is installed, double-click `uninstall-launchagent.command` first. Then stop any manually started bridge and move the binary to Trash:

```sh
mv "$HOME/.local/bin/codex-kimi-bridge" "$HOME/.Trash/codex-kimi-bridge"
```

Before removing the binary, you may run `codex-kimi-bridge hooks uninstall` to remove only this project's hooks. The provider, subagent, skill, and Keychain item are separate. Remove them individually only if you no longer need them.
