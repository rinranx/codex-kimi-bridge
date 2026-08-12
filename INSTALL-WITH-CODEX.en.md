# Install by Giving the Repository to Codex

[简体中文](INSTALL-WITH-CODEX.md) | **English**

If terminal commands and TOML are unfamiliar, give the project URL to Codex Desktop. It can install the default Rust single binary and configure the provider, subagent, and management skill.

This is close to one-click setup, but you should still review scoped writes to `~/.local/bin` and `~/.codex`, enter the API key only in a macOS Keychain terminal prompt, and decide whether a quota-consuming live test may run.

Users do not need Rust, Node.js, or npm. The separately named `codex-kimi-bridge-node` under `node/` is only a fallback after a confirmed Rust compatibility problem.

## How to use it

Create a Codex Desktop task and fill in this line:

```text
My key type / membership: Kimi Code Andante / Moderato / Allegretto or above / Kimi API Open Platform (choose one)
```

Then paste the complete prompt below:

```text
Install and configure the default Rust Codex Kimi Bridge from:

https://github.com/rinranx/codex-kimi-bridge

Follow these requirements exactly:

1. Read README.en.md, INSTALL-WITH-CODEX.en.md, install/INSTALL-GUIDE.en.md, SECURITY.md, and the SHA-256 files attached to GitHub Release v0.3.0 before installing.
2. Install the default Rust single-binary codex-kimi-bridge 0.3.0. Do not install similarly named npm packages or treat the node/ fallback as the default. A new installation requires no Rust, Node.js, or npm.
3. Download the macOS kit only from this repository's versioned GitHub Release v0.3.0, verify SHA-256, and install it at ~/.local/bin/codex-kimi-bridge without sudo. Do not obtain build artifacts from main/downloads.
4. Before installation, inspect command -v codex-kimi-bridge and port 8787 without changing state. If an old npm 0.1.0 command or unknown listener exists, report its exact path/process and ask before uninstalling, overwriting, or stopping anything. After approval, the old official npm package may be removed with npm uninstall --global codex-kimi-bridge.
5. Ask for my key type and membership if I did not provide them. Use the README table to select the correct model, upstream, context window, and auto-compact limit. Do not assume Allegretto 1M access. Kimi Code membership and Kimi API Open Platform keys, models, and endpoints are not interchangeable.
6. Never ask me to paste the API key into chat. Use the macOS Keychain command and let me enter it only at the secure terminal prompt. Keep the service name codex-kimi-code-api-key; never display, log, or repeat the secret.
7. Check Codex Multi-agent v2. Prefer enabling it in Desktop Experimental Features; only merge multi_agent_v2 = true when this Codex version needs it, and never create a duplicate [features] table.
8. Read ~/.codex/config.toml and make a timestamped backup before editing. Merge only required [agents] and model_providers.codex_kimi_bridge keys; preserve unrelated settings, avoid duplicate TOML tables, and never place the key in TOML.
9. Install ~/.codex/agents/kimi_frontend.toml from the repository template. Adjust model, model_context_window, and model_auto_compact_token_limit for my access, and keep sandbox_mode = "read-only".
10. Install companion-skill/manage-codex-kimi-bridge under ~/.codex/skills/. Keep the bridge on 127.0.0.1 and the upstream on HTTPS; never enable --allow-non-loopback or --allow-insecure-upstream.
11. Let me choose one of three startup methods and explain the tradeoffs: A) call manage-codex-kimi-bridge from Codex for on-demand startup; B) double-click start-codex-kimi-bridge.command in a visible Terminal; C) install the macOS LaunchAgent for login-time background startup and recovery after an unexpected exit. Never run competing listeners on 8787. The LaunchAgent configuration adds no separate resident manager process; memory belongs to the same Rust bridge.
12. Install the bundled install-launchagent.command only if I choose C. Confirm that its plist uses the absolute ~/.local/bin/codex-kimi-bridge path, remains loopback-only, and explain uninstall-launchagent.command. A Kimi API Open Platform setup must not reuse the default Kimi Code LaunchAgent arguments unchanged.
13. On startup, require output containing implementation: rust. First run version, checksum, health, doctor --json, and offline translate-request checks that do not contact Kimi.
14. doctor --live and request make real Kimi calls and consume quota. Ask for my separate explicit approval immediately before either command. If I do not approve, skip them and state that no live call was made.
15. Suggest node/ only after confirming a Rust compatibility problem. Its command must be codex-kimi-bridge-node, and it cannot listen on 8787 at the same time as Rust.
16. At completion report: downloaded file and SHA-256, installed path and version, key type, model, upstream, configuration files changed and backed up, Multi-agent v2 status, my selected startup method, whether the bridge is running, how to start it next time, and whether any real Kimi request was sent.
```

## Expected result

- `~/.local/bin/codex-kimi-bridge --version` prints `0.3.0`
- `/health` identifies `implementation: rust`
- The API key exists only in Keychain
- Existing Codex settings remain intact with no duplicate TOML tables
- `kimi_frontend` matches the user's membership or platform access
- Multi-agent v2 can schedule the custom subagent
- One of Codex on demand, visible Terminal, or LaunchAgent has been explicitly selected
- No real Kimi request was sent without approval

If Codex cannot read GitHub, manually download the [complete macOS kit](https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.3.0/codex-kimi-bridge-macos-install-kit-0.3.0.zip), provide the extracted folder, and change the prompt's first line to “Install from the local installation-kit folder I provided.”

See the [complete installation guide](install/INSTALL-GUIDE.en.md) for every manual step.
