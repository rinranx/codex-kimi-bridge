# Install with Codex

[简体中文](INSTALL-WITH-CODEX.md) | **English**

If you are not comfortable with the terminal or TOML, you can give this repository to Codex and let it perform most of the installation and configuration by following the project documentation.

This is an **assisted installation** that is close to one click, not a fully unattended installer. Two actions remain yours for security:

1. Approve narrowly scoped writes to locations such as `~/.codex`.
2. Enter the API key directly into the secure macOS Keychain prompt. Never send the key through chat.

## Before you start

- A Mac with Codex Desktop installed and signed in.
- Node.js 20 or later. If it is missing, Codex can check and guide you through installing it.
- Either a Kimi Code membership API key or a pay-as-you-go Kimi API Open Platform key.
- For Kimi Code, it helps to know whether your membership is Andante, Moderato, Allegretto, or higher.

## The simplest setup

1. Create a new task in Codex Desktop.
2. Copy and send the complete prompt below.
3. If you did not fill in the key type or membership tier, answer Codex's question about that item only.
4. Review and approve installation actions with a clear, limited scope.
5. When Codex is ready to store the API key, type it into the terminal prompt it opens or identifies. Do not paste it into chat.
6. Codex should run a real Kimi request, which may consume quota, only after you explicitly approve it.

You may fill in this line and send it with the prompt:

```text
My key type / membership tier: __________________
```

## Prompt to copy into Codex

```text
Install and configure Codex Kimi Bridge from this official repository:

https://github.com/rinranx/codex-kimi-bridge

Follow these requirements strictly:

1. Read README.en.md, INSTALL-WITH-CODEX.en.md, and install/INSTALL-GUIDE.en.md before installing anything.
2. Use only codex-kimi-bridge supplied by this repository. Do not substitute a similarly named third-party package from npm.
3. Check Node.js, Codex Desktop, and Multi-agent v2 first. Do not use sudo npm install, and do not change permissions for my entire home directory or npm cache.
4. If I have not provided my key type and membership tier, first ask whether I use:
   - Kimi Code Andante
   - Kimi Code Moderato
   - Kimi Code Allegretto or higher
   - A pay-as-you-go Kimi API Open Platform key
5. Select the correct model, upstream endpoint, context window, and automatic compaction value for my tier. Do not assume that everyone has Allegretto access. Kimi Code and Open Platform keys, models, and endpoints are not interchangeable.
6. Never ask me to paste an API key into chat. Use a secure macOS Keychain input command and have me type the key directly into its terminal prompt.
7. Read and back up ~/.codex/config.toml before changing it. Merge the [features], [agents], and provider settings without overwriting unrelated configuration or creating duplicate TOML tables.
8. Install the kimi_frontend subagent and the bundled manage-codex-kimi-bridge management skill. Keep sandbox_mode = "read-only".
9. Bind the bridge only to 127.0.0.1 and require HTTPS for the upstream. Do not enable --allow-non-loopback or --allow-insecure-upstream.
10. Run checks that do not consume Kimi quota first. Ask for my explicit approval before any real Kimi request.
11. When finished, report:
    - Which files were installed
    - The selected model and upstream endpoint
    - Whether the bridge is running
    - How to start it next time
    - Whether any real Kimi request was made

Never display, log, or repeat my API key.
```

## What Codex should do for you

Within the scope you approve, Codex should:

- Read the project documentation and verify downloads and SHA-256 checksums.
- Check Node.js, Codex Desktop, Multi-agent v2, and your existing Codex configuration.
- Back up and safely merge `~/.codex/config.toml` without removing unrelated settings.
- Install the bridge command, the `kimi_frontend` subagent, and the management skill.
- Match the model, upstream endpoint, and context settings to your key type and membership tier.
- Run syntax, version, and health checks that do not call Kimi first.
- Clearly report every change and whether it made a real Kimi request.

Codex should not:

- Ask you to send an API key through chat or store it in a plain configuration file.
- Use a similarly named package that did not come from this repository.
- Run `sudo npm install` or change permissions across your home directory to fix npm.
- Make a real Kimi request without confirmation.
- Expose the local service on `0.0.0.0` or disable the HTTPS requirement for the upstream.

## If Codex cannot read the repository directly

Download the [complete macOS installation kit](https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-install-kit-0.1.0.zip), extract it, give the folder to Codex, and send the same prompt. Replace its first sentence with:

```text
Install and configure Codex Kimi Bridge from the local installation-kit folder I provided.
```

If Codex cannot write to `~/.codex`, approve only the specific files listed in the installation guide. You do not need to grant access to your entire home directory.

The bundled `manage-codex-kimi-bridge` skill is for starting, stopping, diagnosing, and switching models after setup. It is not the first-time installer itself.

For every manual step, read the [complete macOS installation guide](install/INSTALL-GUIDE.en.md). See the [English README](README.en.md#choose-a-kimi-code-model-for-your-membership-tier) for membership-tier and model details.
