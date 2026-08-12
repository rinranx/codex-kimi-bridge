# Node Fallback Installation

[简体中文](INSTALL-GUIDE.zh-CN.md) | **English**

The default installation is the Rust `codex-kimi-bridge` binary. This guide is only for the Node fallback command, `codex-kimi-bridge-node 0.1.0`.

## Install

Node.js 20 or later is required. From the `node/` directory, run:

```sh
npm install --global .
command -v codex-kimi-bridge-node
codex-kimi-bridge-node --version
```

From the repository root, you can also install the packaged fallback:

```sh
npm install --global ./downloads/node/codex-kimi-bridge-node-0.1.0.tgz
```

Do not use `sudo npm install`, and do not change permissions across your home directory or npm cache.

## Use

Stop the running Rust bridge or any other service using port 8787, then run:

```sh
codex-kimi-bridge-node doctor --json
codex-kimi-bridge-node serve
```

The fallback still uses `http://127.0.0.1:8787/v1`, the same Codex provider, and the same Keychain service, so no change to `kimi_frontend.toml` is required.

A live diagnostic contacts Kimi and consumes a small amount of quota. Run it only when explicitly needed:

```sh
codex-kimi-bridge-node doctor --live --json
```

## Uninstall

```sh
npm uninstall --global codex-kimi-bridge-node
```

See the root English README and installation guide for complete configuration and security guidance.
