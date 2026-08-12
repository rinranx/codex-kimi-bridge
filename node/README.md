# Codex Kimi Bridge Node 回退版

**简体中文** | [English](README.en.md)

这是 `codex-kimi-bridge` 原始 Node.js 实现的保留版本。项目的默认实现和默认下载已经改为 Rust；只有在 Rust 版遇到兼容问题时，才建议使用这里的回退版。

## 名称和共存规则

- npm 包名：`codex-kimi-bridge-node`
- 命令：`codex-kimi-bridge-node`
- 版本：`0.2.0`
- 默认地址仍为 `127.0.0.1:8787`
- Keychain 项目仍为 `codex-kimi-code-api-key`

Rust 版和 Node 版不能同时监听 8787。切换实现前，先在当前桥接终端按 `Control-C`。

## 从源码运行

需要 Node.js 20 或更高版本。本目录没有第三方运行时 npm 依赖。

```sh
node ./bin/codex-kimi-bridge-node.mjs --version
node ./bin/codex-kimi-bridge-node.mjs serve
```

## 安装回退命令

在本目录运行：

```sh
npm install --global .
codex-kimi-bridge-node doctor --json
codex-kimi-bridge-node serve
```

安装回退版不会修改 `~/.codex/config.toml`。由于本地地址和 Responses 协议保持不变，原有 provider 与 `kimi_frontend` 配置可以继续使用。

也可以在项目根目录安装已打包的回退文件：

```sh
npm install --global https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.3.0/codex-kimi-bridge-node-0.2.0.tgz
```

## 功能

- Responses 文本、图片和视频输入转换
- 非流式与 SSE 流式输出
- 顶层及 `namespace` 内的 function 与 custom tools
- 多轮工具调用所需的 Kimi `reasoning_content` 保留
- JSON Object 与 JSON Schema 输出
- 只监听 loopback、只允许安全上游、拒绝重定向
- 不记录请求正文、API Key 或推理内容

## 验证

```sh
npm run check
npm test
npm run smoke
```

完整配置、模型等级与 Keychain 说明请回到 [项目主 README](../README.md)。
