# Codex Kimi Bridge

一个独立实现、零第三方运行时依赖的本地桥接器，把 Codex Desktop 使用的 OpenAI Responses 请求转换为 Kimi Code 的 OpenAI-compatible Chat Completions 请求。

```text
Codex Desktop ── Responses API ──> 127.0.0.1:8787
                                      │
                                      └── HTTPS ──> api.kimi.com/coding/v1/chat/completions
```

运行时只使用 Node.js 标准库。

首次安装请使用 [完整 macOS 安装指南](install/INSTALL-GUIDE.zh-CN.md) 和随附的 TOML 模板；安装包不会包含 API Key。

## 下载与一键启动

- 项目主页：<https://github.com/rinranx/codex-kimi-bridge>
- macOS 完整安装包：<https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-install-kit-0.1.0.zip>
- npm 安装包：<https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-0.1.0.tgz>
- SHA-256 校验：<https://github.com/rinranx/codex-kimi-bridge/tree/main/downloads>

安装包解压并完成配置后，双击 `start-installed-codex-kimi-bridge.command` 即可启动本机桥接；它会调用已经全局安装的 `codex-kimi-bridge`。源码目录中的 `start-codex-kimi-bridge.command` 则可以直接启动未全局安装的开发版本。

## 已实现

- Responses 文本、图片和视频输入转换
- 非流式与 SSE 流式输出
- function tools 与 Responses custom tools
- 多轮工具调用所需的 Kimi `reasoning_content` 保留
- `low` / `high` / `max` 推理强度映射
- JSON Object 与 JSON Schema 输出格式
- Kimi Code Plan 的 `prompt_cache_key`
- API 错误透传和稳定的本地错误格式
- 默认仅监听 `127.0.0.1`，上游默认只允许 HTTPS
- 不记录请求正文、API Key 或推理内容

## 环境要求

- macOS、Linux 或 Windows
- Node.js 20 或更高版本；当前开发和测试环境为 Node.js 26
- Kimi Code API Key。Allegretto 的 Kimi Code Key 可直接使用

不需要 `npm install`，项目的运行依赖数量为 0。

## 可选：安装成全局命令

在项目目录运行：

```sh
npm install --global .
codex-kimi-bridge doctor --json
codex-kimi-bridge serve
```

之后可以在任意目录使用 `codex-kimi-bridge`。不想全局安装时，继续使用项目内的 `.command` 启动器即可。

## Codex 配置

Codex Provider 应使用下面的结构：

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

子代理配置：

```toml
model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_reasoning_effort = "xhigh"
sandbox_mode = "read-only"
```

这里的 `xhigh` 会转换为 Kimi K3 的 `max`。

## 更换 API Key

桥接器不保存 Key；它只转发 Codex 每次请求中的 Bearer token。更新 Keychain 即可，不需要改源码：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

把 `-w` 保持为最后一个参数，终端会安全提示输入新 Key，不会把 Key 写进命令历史。更新后重启 Codex Desktop 或新开对话。

## 命令

```sh
codex-kimi-bridge serve
codex-kimi-bridge doctor --json
codex-kimi-bridge doctor --live --json
codex-kimi-bridge translate-request --file fixtures/responses-request.json
codex-kimi-bridge request "只回复 OK"
codex-kimi-bridge --help
```

`translate-request` 完全离线，可用于检查协议转换结果。`request` 会向正在运行的本地桥接发送请求，并从 `KIMI_CODE_API_KEY` 或 macOS Keychain 读取测试用 Key。

## 安全设计

- 默认绑定 `127.0.0.1`；绑定外网接口必须显式使用 `--allow-non-loopback`
- 默认上游为 HTTPS；明文 HTTP 只允许显式指定的 loopback 测试服务
- 禁止跟随上游重定向，避免凭据意外转发
- 日志只包含清洗后的状态与错误码
- Kimi 推理状态仅驻内存，最多 512 项或 64 MiB，两小时自动过期；进程退出立即消失

详见 [SECURITY.md](SECURITY.md)。

## 验证

```sh
npm run check
npm test
npm run smoke
```

测试覆盖请求转换、SSE、function/custom tools、认证、隐私日志和两轮工具调用。开发沙箱不允许监听本地端口，因此自动端到端测试直接调用与 HTTP Server 完全相同的请求处理器，并使用本地模拟 Kimi Response；在普通终端可通过 `/health` 和 `doctor --live` 补做真实端口与上游验证。

## 已知边界

- 只安全转换 Responses 的 `function` 和 `custom` tools；`web_search`、`file_search` 等 OpenAI 托管型工具会明确报错，不会静默删除
- 不支持 `previous_response_id`；调用方需要像 Codex 一样发送完整对话 items
- `parallel_tool_calls` 不转发，因为 Kimi Chat API 文档未声明该请求字段
- 多轮工具调用的推理缓存是进程内状态；若桥接在一次未完成的工具链中途重启，请重开该 Kimi 子代理任务
- Codex Desktop 的实验性 Multi-agent v2 是否允许第三方 provider 仍由 Codex 本身决定；桥接器只负责协议兼容，不能绕过 Desktop 的调度限制

## 可选伴随技能

项目附带 [`manage-codex-kimi-bridge`](companion-skill/manage-codex-kimi-bridge/SKILL.md) 技能。全局安装命令后，可把该技能目录复制到 `~/.codex/skills/`，让 Codex 自动诊断和启动桥接。它不是桥接运行所必需的。

## 删除或回退

在桥接终端按 `Control-C` 即可停止。若安装了全局命令：

```sh
npm uninstall --global codex-kimi-bridge
```

这不会删除 Keychain 中的 Key，也不会修改 Codex 配置。
