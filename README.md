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
- Kimi Code API Key。不同会员等级均可使用，请按下方表格选择有权限的模型

不需要 `npm install`，项目的运行依赖数量为 0。

## 确认启用 Codex Multi-agent v2

这个项目通过 Codex 的多代理功能把 `kimi_frontend` 作为自定义子代理调度。继续配置前，请先确认 Multi-agent v2 已启用：

1. 打开 Codex Desktop 设置。
2. 在“实验功能”或“功能”页面找到 **Multi-agent v2**。
3. 如果它存在但尚未开启，请将其开启。
4. 完全退出并重新打开 Codex Desktop。

较新的 Codex 版本可能已经默认启用 Multi-agent v2，并将其标记为稳定功能；如果设置中没有开关，但 Codex 已能显示和调度子代理，则不需要额外修改。

也可以在终端检查当前状态：

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

输出中的 `multi_agent` 和 `multi_agent_v2` 应为 `true`。旧版 Codex 或手动管理配置时，可以在 `~/.codex/config.toml` 中加入：

```toml
[features]
multi_agent_v2 = true
```

如果文件中已经有 `[features]`，只添加 `multi_agent_v2 = true`，不要创建第二个 `[features]`。后文的 `[agents] enabled = true` 也必须保留；两项用途不同。

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

子代理配置（下面是 Allegretto 会员调用 K3 1M 上下文的实测示例）：

```toml
model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
sandbox_mode = "read-only"
```

这里的 `xhigh` 会转换为 Kimi K3 的 `max`。

## 按会员等级选择 Kimi Code 模型

上面的 `k3` 1M 配置不是安装桥接的硬性要求，只是本项目已经实际验证过的 Allegretto 配置。所有 Kimi Code 会员使用相同的上游地址和 Kimi Code API Key 类型：

```text
https://api.kimi.com/coding/v1/chat/completions
```

区别只在于会员等级允许调用的模型和上下文窗口：

| Kimi 会员等级 | 推荐模型 ID | 上下文窗口 | 适用场景 |
| --- | --- | ---: | --- |
| Andante／所有 Kimi Code 会员 | `kimi-for-coding` | `262144` | 日常开发、代码补全 |
| Moderato | `k3-256k` | `262144` | 推荐；256K 内与 K3 效果相同，更节省配额 |
| Moderato | `k3` | `262144` | 使用 K3，但没有 1M 权限 |
| Allegretto 及以上 | `k3` | `1048576` | 大型代码库、长上下文任务 |
| Allegretto 及以上 | `kimi-for-coding-highspeed` | `262144` | 优先输出速度 |

模型权限和上下文以 [Kimi Code 官方模型配置](https://www.kimi.com/code/docs/en/kimi-code/models.html) 为准。官方说明中，`k3-256k` 在 256K 范围内与 `k3` 效果相同，而 `k3` 1M 的配额消耗约为其两倍。

选择模型后，修改 `~/.codex/agents/kimi_frontend.toml` 中对应的几行。

### Andante／通用会员

```toml
model = "kimi-for-coding"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model kimi-for-coding
```

### Moderato 推荐配置

```toml
model = "k3-256k"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model k3-256k
```

### Allegretto 及以上：K3 1M

```toml
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
```

```sh
codex-kimi-bridge serve --model k3
```

### Allegretto 及以上：K2.7 HighSpeed

```toml
model = "kimi-for-coding-highspeed"
model_context_window = 262144
model_auto_compact_token_limit = 230000
model_reasoning_effort = "high"
```

```sh
codex-kimi-bridge serve --model kimi-for-coding-highspeed
```

`serve --model` 设置桥接器的默认模型，并显示在健康检查中；Codex 子代理实际发送的模型以 `kimi_frontend.toml` 中的 `model` 为准。因此切换模型时应同步修改两处。然后重新启动桥接，并重启 Codex Desktop 或新建任务，避免沿用旧会话的模型缓存。运行实时诊断时，也应把同一个模型传给 `doctor --live --model <模型 ID>`。

同一个 Kimi Code Key 可以在当前会员等级允许的模型之间切换，不需要因为更换模型而重新生成 Key。

## 非 Kimi Code 会员：按量付费 API（进阶）

Kimi Code 会员 Key 与 Kimi API 开放平台 Key 是两个独立产品，Key、模型 ID 和调用地址不能混用。详见 [Kimi API 官方排错说明](https://www.kimi.com/help/kimi-api/api-troubleshooting)。

国际开放平台 Key 使用：

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

中国大陆开放平台 Key 使用：

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.cn/v1/chat/completions \
  --model kimi-k3
```

对应的 `kimi_frontend.toml` 配置为：

```toml
model = "kimi-k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
```

再把开放平台 Key 写入同一个本机钥匙串项目。`codex-kimi-code-api-key` 只是本地项目名称，不决定 Key 的类型：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

> [!IMPORTANT]
> `codex-kimi-bridge 0.1.0` 的完整实测和发布验收使用的是 Kimi Code 会员路线。开放平台提供兼容的 Chat Completions 接口，桥接器也允许自定义上游和模型，但这条路线在 0.1.0 中应视为进阶／实验性配置；正式使用前请先用相同的 Key、地区地址和模型完成小额测试。

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
- Codex Desktop 是否允许把第三方 provider 调度为 Multi-agent v2 子代理仍由 Codex 本身决定；桥接器只负责协议兼容，不能绕过 Desktop 的调度限制

## 可选伴随技能

项目附带 [`manage-codex-kimi-bridge`](companion-skill/manage-codex-kimi-bridge/SKILL.md) 技能。全局安装命令后，可把该技能目录复制到 `~/.codex/skills/`，让 Codex 自动诊断和启动桥接。它不是桥接运行所必需的。

## 删除或回退

在桥接终端按 `Control-C` 即可停止。若安装了全局命令：

```sh
npm uninstall --global codex-kimi-bridge
```

这不会删除 Keychain 中的 Key，也不会修改 Codex 配置。
