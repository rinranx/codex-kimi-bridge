# Node 回退版安装说明

**简体中文** | [English](INSTALL-GUIDE.en.md)

项目默认安装的是 Rust 版 `codex-kimi-bridge`。本说明仅用于安装 Node 回退命令 `codex-kimi-bridge-node 0.1.0`。

## 安装

需要 Node.js 20 或更高版本。进入 `node/` 目录后运行：

```sh
npm install --global .
command -v codex-kimi-bridge-node
codex-kimi-bridge-node --version
```

也可以从项目根目录安装项目提供的回退 npm 包：

```sh
npm install --global https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-node-0.1.0.tgz
```

不要使用 `sudo npm install`，也不要修改整个主目录或 npm 缓存的权限。

## 使用

先停止正在运行的 Rust 版或其他 8787 服务，再运行：

```sh
codex-kimi-bridge-node doctor --json
codex-kimi-bridge-node serve
```

它仍使用 `http://127.0.0.1:8787/v1`、相同的 Codex provider 和相同的 Keychain 项目，因此无需修改 `kimi_frontend.toml`。

真实诊断会连接 Kimi 并消耗少量额度，只能在明确需要时运行：

```sh
codex-kimi-bridge-node doctor --live --json
```

## 卸载

```sh
npm uninstall --global codex-kimi-bridge-node
```

完整配置和安全说明见项目根目录的中文 README 与安装指南。
