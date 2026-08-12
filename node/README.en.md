# Codex Kimi Bridge Node Fallback

[简体中文](README.md) | **English**

This directory preserves the original Node.js implementation of `codex-kimi-bridge`. Rust is now the default implementation and download. Use this fallback only when the Rust build has a compatibility problem.

## Naming and coexistence

- npm package: `codex-kimi-bridge-node`
- Command: `codex-kimi-bridge-node`
- Version: `0.1.0`
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

You can also install the packaged fallback from the repository root:

```sh
npm install --global https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-node-0.1.0.tgz
```

## Features

- Responses text, image, and video conversion
- Non-streaming and SSE streaming output
- Function and custom tools
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
