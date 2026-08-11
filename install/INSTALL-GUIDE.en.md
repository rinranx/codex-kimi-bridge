# Codex Kimi Bridge Clean Installation Guide (macOS)

[简体中文](INSTALL-GUIDE.zh-CN.md) | **English**

Applicable version: `codex-kimi-bridge 0.1.0`  
Configuration reviewed: 2026-08-11

This guide installs the following workflow on a new Mac:

```text
Codex Desktop primary agent
        │
        └── kimi_frontend subagent (Kimi K3 / read-only)
                    │
                    └── local bridge at 127.0.0.1:8787
                              │
                              └── Kimi Code HTTPS API
```

## Installation kit contents

The complete installation kit contains the following core files. Never write an API key into any of them:

- `codex-kimi-bridge-0.1.0.tgz`
- `SHA256SUMS`
- Chinese and English installation guides, README files, and Install-with-Codex prompt pages
- The `templates/` directory
- The `manage-codex-kimi-bridge` management skill
- A double-click launcher plus license, security, and provenance documents

Download the complete `codex-kimi-bridge-macos-install-kit-0.1.0.zip`, verify its SHA-256 checksum, and then extract it.

## Step 1: Install Codex Desktop and Node.js

1. Install the latest Codex Desktop and sign in.
2. Install Node.js 20 or later. If Homebrew is already installed, run:

   ```sh
   brew install node
   ```

   Otherwise, install a currently supported Node.js release from the official Node.js website.

3. Verify the installation:

   ```sh
   node --version
   npm --version
   ```

   `node --version` must report at least `v20`.

## Step 2: Verify and install the bridge package

Extract the installation kit into a directory such as `~/Documents/CodexKimiBridge`, then enter that directory:

```sh
cd "$HOME/Documents/CodexKimiBridge"
shasum -a 256 -c SHA256SUMS
```

Expected output:

```text
codex-kimi-bridge-0.1.0.tgz: OK
```

Install the global command:

```sh
npm install --global ./codex-kimi-bridge-0.1.0.tgz
command -v codex-kimi-bridge
codex-kimi-bridge --version
```

The expected version is `0.1.0`. Do not use `sudo npm install`. If a Homebrew Node.js global installation reports a permission error, keep the original error message and diagnose it first; do not change ownership or permissions across your entire home directory.

## Step 3: Store the Kimi Code API key in macOS Keychain

Create a dedicated Kimi Code API key for this bridge in the Kimi Code Console. Never share the key through chat, configuration files, or screenshots.

Run:

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

Keep `-w` as the final argument. Paste the key when the terminal prompts you; no characters are displayed while you type.

Verify only that the item exists, without printing the key:

```sh
/usr/bin/security find-generic-password \
  -s "codex-kimi-code-api-key" >/dev/null \
  && echo "Kimi Keychain item: OK"
```

## Step 4: Configure the custom Codex provider

The personal provider must be added to the user-level `~/.codex/config.toml`, not to a project-level `.codex/config.toml`.

First confirm that Codex Multi-agent v2 is enabled:

1. Open Codex Desktop settings.
2. Find **Multi-agent v2** under “Experimental Features” or “Features.”
3. If the option is present but disabled, enable it.
4. Newer Codex versions may enable it by default and mark it as stable. If no toggle appears but Codex can already display and schedule subagents, no additional action is required.

You can also check the effective state in a terminal:

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

Both `multi_agent` and `multi_agent_v2` should show `true`. The template below still sets `multi_agent_v2 = true` explicitly for compatibility with Codex versions that require manual activation.

Create the configuration file if necessary, then open it:

```sh
mkdir -p "$HOME/.codex"
touch "$HOME/.codex/config.toml"
open -e "$HOME/.codex/config.toml"
```

Merge the contents of `templates/config-kimi-provider.toml` into the file.

Important:

- If `[features]` already exists, add or update only `multi_agent_v2 = true`; do not create a second `[features]` table.
- If `[agents]` already exists, merge the corresponding keys into it; do not create a second `[agents]` table.
- If `[model_providers.codex_kimi_bridge]` already exists, update that table instead of pasting a duplicate.
- Never put the API key directly in TOML.

The merged content is:

```toml
[features]
multi_agent_v2 = true

[agents]
enabled = true
max_concurrent_threads_per_session = 4

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

Save the file and close TextEdit.

## Step 5: Install the custom `kimi_frontend` subagent

Codex automatically loads personal custom agents from `~/.codex/agents/*.toml`.

Run:

```sh
mkdir -p "$HOME/.codex/agents"
cp ./templates/kimi_frontend.toml "$HOME/.codex/agents/kimi_frontend.toml"
```

Inspect the file:

```sh
sed -n '1,220p' "$HOME/.codex/agents/kimi_frontend.toml"
```

The key settings are:

- `model_provider = "codex_kimi_bridge"`
- `model = "k3"`
- `model_context_window = 1048576`
- `model_reasoning_effort = "xhigh"`, which the bridge maps to Kimi `max`
- `sandbox_mode = "read-only"`

The bundled template is the tested Allegretto configuration using K3 with a 1M-token context window. If your membership tier differs, select a permitted model and context window from the [model table in the English README](../README.en.md#choose-a-kimi-code-model-for-your-membership-tier), update `kimi_frontend.toml`, and use the same model ID when starting or diagnosing the bridge.

## Step 6: Start the bridge

In a dedicated terminal, run:

```sh
codex-kimi-bridge serve
```

The service is running when output similar to the following appears:

```text
codex-kimi-bridge 0.1.0 listening on http://127.0.0.1:8787
upstream: https://api.kimi.com
privacy: request bodies and credentials are not logged
```

Keep this terminal open. Start the bridge again after every Mac restart. You can also double-click `start-installed-codex-kimi-bridge.command` in the installation kit.

If you selected a model other than the default `k3`, start the bridge with the matching model ID, for example:

```sh
codex-kimi-bridge serve --model k3-256k
```

The actual model sent by Codex still comes from `kimi_frontend.toml`; keeping both settings synchronized makes health and diagnostic output accurate.

## Step 7: Verify the local service, key, and a real Kimi request

Open another terminal.

Check the local service:

```sh
curl -s http://127.0.0.1:8787/health
codex-kimi-bridge doctor --json
```

Run one small live request. This consumes a small amount of Kimi quota:

```sh
codex-kimi-bridge doctor --live --json
```

If you selected a model other than `k3`, pass that model to the live diagnostic as well:

```sh
codex-kimi-bridge doctor --live --json --model k3-256k
```

You can also test through the local Responses bridge:

```sh
codex-kimi-bridge request "Reply with OK only."
```

## Step 8: Restart Codex Desktop and test the subagent

1. Quit Codex Desktop completely; closing only the window is not enough.
2. Keep the bridge terminal running.
3. Reopen Codex Desktop and start a new task.
4. Enter:

   ```text
   Use the kimi_frontend subagent to perform a read-only review of the current project's frontend visuals, interactions, and responsive layout. Wait for it to finish, then summarize its findings.
   ```

5. Confirm that a `kimi_frontend` worker appears in the Desktop subagent activity area.

If Multi-agent v2 is disabled, or if the current Codex version still restricts third-party-provider subagents, the bridge health check may pass while Desktop refuses to schedule the agent. Confirm the feature state first, then determine whether the behavior is a Codex client restriction. It does not necessarily indicate a broken API key or bridge.

## Optional: install the Codex management skill

The installation kit also includes the `manage-codex-kimi-bridge` skill, which lets Codex diagnose, start, and inspect the bridge. It is not required for the bridge itself to run.

```sh
mkdir -p "$HOME/.codex/skills"
cp -R ./companion-skill/manage-codex-kimi-bridge \
  "$HOME/.codex/skills/manage-codex-kimi-bridge"
```

Quit and reopen Codex Desktop after installation. You can invoke it with:

```text
Use $manage-codex-kimi-bridge to inspect and safely start the local Kimi bridge.
```

## Replace the API key later

No bridge or TOML change is required. Run the Keychain command again:

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

Then quit and reopen Codex Desktop, or start a new task. The bridge itself does not cache the API key.

## Upgrade the bridge later

1. Stop the running bridge by pressing `Control-C` in its terminal.
2. Verify the new package’s SHA-256 checksum.
3. Install the new package:

   ```sh
   npm install --global ./new-codex-kimi-bridge.tgz
   codex-kimi-bridge --version
   ```

4. Start `codex-kimi-bridge serve` again.

Do not restart the bridge in the middle of an unfinished tool call. Preserved Kimi reasoning state exists only in the current bridge process memory.

## Troubleshooting

### Port 8787 is already in use

```sh
lsof -nP -iTCP:8787 -sTCP:LISTEN
```

Identify the process before stopping it:

```sh
kill -INT <PID>
```

### `codex-kimi-bridge: command not found`

```sh
npm list --global --depth=0
npm bin --global 2>/dev/null || npm prefix --global
```

Open a new terminal and try again. Do not substitute an npm package with a similar name.

### HTTP 401 or `missing_api_key`

```sh
/usr/bin/security find-generic-password \
  -s "codex-kimi-code-api-key" >/dev/null \
  && echo OK
```

If `OK` is not printed, repeat Step 3. Do not run a Keychain lookup that prints the secret and paste its output into chat.

### Cannot connect to `/health`

The bridge is not running, or another process is using port 8787. Return to Step 6 and start the service after inspecting the port.

### Codex cannot find `kimi_frontend`

Confirm the file and required fields:

```sh
test -f "$HOME/.codex/agents/kimi_frontend.toml" && echo "agent file: OK"
rg -n '^(name|description|developer_instructions|model_provider|model)\s*=' \
  "$HOME/.codex/agents/kimi_frontend.toml"
```

The custom-agent file must contain `name`, `description`, and `developer_instructions`. Fix the file, then quit and reopen Codex Desktop.

### Kimi reports a model permission or quota error

Confirm that you are using a Kimi Code API key rather than a key from another platform. Confirm that your membership permits the model configured in `kimi_frontend.toml`, then check the key status and remaining quota in the Kimi Code Console.

## Security checklist

- [ ] The installation kit contains no API key.
- [ ] The key exists only in macOS Keychain.
- [ ] The provider URL is `http://127.0.0.1:8787/v1`.
- [ ] The bridge upstream is `https://api.kimi.com`.
- [ ] Multi-agent v2 is enabled, or `codex features list` reports `true`.
- [ ] `--allow-non-loopback` is not enabled.
- [ ] `kimi_frontend` remains configured with `sandbox_mode = "read-only"`.
- [ ] No complete response or API key from a live test was posted publicly.

## Configuration update reminder

Codex and Kimi Code both evolve. If you install this package long after 2026-08-11, verify the current documentation first:

- OpenAI Codex configuration reference: `https://learn.chatgpt.com/docs/config-file/config-reference`
- OpenAI Codex subagent documentation: `https://learn.chatgpt.com/docs/agent-configuration/subagents`
- Kimi Code documentation: `https://www.kimi.com/code/docs/en/`

Do not blindly retain feature fields that a future release has removed or renamed. Recheck the provider `base_url`, `wire_api`, authentication-command fields, and required custom-agent fields in particular.
