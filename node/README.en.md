# Codex Kimi Bridge Node Fallback

[简体中文](README.md) | **English**

This directory preserves the original Node.js implementation of `codex-kimi-bridge`. Rust is now the default implementation and download. Use this fallback only when the Rust build has a compatibility problem.

## Naming and coexistence

- npm package: `codex-kimi-bridge-node`
- Command: `codex-kimi-bridge-node`
- Source version: `0.4.1` (shares `CKB1` locally signed handoff and active-Kimi follow-up protection with the Rust default)
- Default address remains `127.0.0.1:8787`
- The Keychain service remains `codex-kimi-code-api-key`

The Rust and Node implementations cannot listen on port 8787 at the same time. Press `Control-C` in the active bridge terminal before switching.

## Run from source

Node.js 20 or later is required. This directory has zero third-party runtime npm dependencies.

```sh
node ./bin/codex-kimi-bridge-node.mjs --version
node ./bin/codex-kimi-bridge-node.mjs serve
```

## Install the fallback command

Run from this directory:

```sh
npm install --global .
codex-kimi-bridge-node doctor --json
codex-kimi-bridge-node serve
```

Installing the fallback does not modify `~/.codex/config.toml`. Because the local endpoint and Responses protocol are unchanged, the existing provider and `kimi_frontend` configuration continue to work.

You can also install the separately named fallback package from Release `v0.4.1`:

```sh
npm install --global https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.4.1/codex-kimi-bridge-node-0.4.1.tgz
```

## Features

- Responses text, image, and video conversion
- Codex Desktop `agent_message` conversion that omits undecryptable provider state, verifies local `CKB1` task envelopes, and supplies `commentary` / `final_answer` assistant phases
- `hook user-prompt-submit` and `hook pre-tool-use` commands interoperable with the Rust default's private cache, signing key, and target registry, including pre-delivery denial of `send_message` / `followup_task` to registered Kimi children
- Non-streaming and SSE streaming output
- Function and custom tools at the top level and inside `namespace`
- Preservation of Kimi `reasoning_content` for multi-step tool calls
- JSON Object and JSON Schema output
- Loopback-only defaults, secure upstream rules, and redirect rejection
- No logging of request bodies, API keys, or reasoning content

## Validation

```sh
npm run check
npm test
npm run smoke
```

Return to the [main project README](../README.en.md) for complete configuration, membership-tier, and Keychain guidance.
