# Codex Kimi Bridge

**简体中文** | [English](README.en.md)

一个本机 Rust 单文件桥接器，把 Codex Desktop 使用的 OpenAI Responses 请求转换为 Kimi Code 或 Kimi API 的 OpenAI-compatible Chat Completions 请求。

```text
Codex Desktop ── Responses API ──> 127.0.0.1:8787
                                      │
                                      └── HTTPS ──> Kimi Chat Completions
```

默认实现从 `0.2.0-alpha.1` 起改为 Rust。安装者不需要 Rust、Node.js 或 npm。原始 Node.js 实现已更名为 `codex-kimi-bridge-node`，完整保留在 [`node/`](node/) 作为回退。

## 下载

- 项目主页：<https://github.com/rinranx/codex-kimi-bridge>
- 当前版本：[`v0.2.0-alpha.2`](https://github.com/rinranx/codex-kimi-bridge/releases/tag/v0.2.0-alpha.2)
- 推荐：macOS 通用安装包（Apple Silicon + Intel）：<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-macos-install-kit-0.2.0-alpha.2.zip>
- Apple Silicon 二进制包：<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-macos-arm64-0.2.0-alpha.2.tar.gz>
- Intel Mac 二进制包：<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-macos-x86_64-0.2.0-alpha.2.tar.gz>
- SHA-256：<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/SHA256SUMS.txt>

构建产物只发布到带版本号的 GitHub Release，不再从 `main/downloads` 覆盖同名文件。当前仍为 alpha，尚未做 Apple 公证。首次打开 `.command` 时，macOS 可能要求右键选择“打开”。请先核对 SHA-256。

## 最简单的安装方式

不熟悉终端时，请使用 [把仓库交给 Codex 安装](INSTALL-WITH-CODEX.md)。其中有一段可以直接复制到 Codex Desktop 的安全安装提示词。

手动安装时：

1. 下载并解压完整安装包。
2. 双击 `install-codex-kimi-bridge.command`，把通用 Rust 二进制安装到 `~/.local/bin/codex-kimi-bridge`。
3. 按 [完整 macOS 安装指南](install/INSTALL-GUIDE.zh-CN.md) 保存 Key、合并 Codex 配置并安装子代理。
4. 从下一节的三种方式中任选一种启动本机桥接；第一次使用可先双击 `start-codex-kimi-bridge.command`。

安装器不会修改 `~/.codex/config.toml`、Keychain 或 shell 配置，也不会自动卸载旧命令。

## 三种启动方式

安装完成后可以任选一种；三种方式运行的是同一个 Rust 二进制，不能同时重复监听 8787。

| 方式 | 如何启动 | 是否常驻 | 适合场景 |
| --- | --- | --- | --- |
| 在 Codex 中启动 | 说“使用 `$manage-codex-kimi-bridge` 检查并启动 Rust 桥接；不要运行 `doctor --live`” | 通常随当前任务或终端会话结束 | 偶尔使用、希望不用时占用为 0 |
| 双击启动器 | 双击 `start-codex-kimi-bridge.command` | 保持终端窗口打开；`Control+C` 停止 | 希望看到日志并手动控制 |
| macOS LaunchAgent | 双击 `install-launchagent.command` | 登录后后台自动启动，异常退出自动恢复 | 每天使用，最省事 |

LaunchAgent 只是一份由 macOS `launchd` 读取的配置，不会再创建一层常驻管理程序；内存占用来自同一个 Rust 桥接进程。退出 Codex Desktop 后，LaunchAgent 管理的桥接仍会运行。

查看状态：

```sh
launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"
curl -s http://127.0.0.1:8787/health
```

不再需要自动启动时，双击 `uninstall-launchagent.command`。它只卸载自动启动并把 plist 移到废纸篓，不删除桥接二进制、Codex 配置、Keychain 或日志。随附 LaunchAgent 使用默认 Kimi Code HTTPS 上游；Kimi API 开放平台用户应先自定义 `ProgramArguments`，不要直接套用默认模板。

## 从 Node 0.1.0 迁移

旧 npm 包和 Rust 默认版都曾使用 `codex-kimi-bridge` 这个命令名。安装 Rust 版前先停止旧桥，并移除旧的全局 npm 命令：

```sh
npm uninstall --global codex-kimi-bridge
```

然后安装 Rust 二进制。Provider URL、Keychain 项目和 `kimi_frontend.toml` 都不需要改变。需要临时回退时，可安装名称不同的 Node 版：

```sh
cd node
npm install --global .
codex-kimi-bridge-node serve
```

Rust 版和 Node 回退版不能同时监听 8787。

## 已实现

- Responses 文本、图片和视频输入转换
- 非流式与 SSE 流式输出
- function tools 与 Responses custom tools
- 多轮工具调用所需的 Kimi `reasoning_content` 内存保留
- `low` / `high` / `max` 推理强度映射
- JSON Object 与 JSON Schema 输出格式
- Kimi Code Plan `prompt_cache_key`
- API 错误透传与稳定的本地错误格式
- 默认只监听 `127.0.0.1`，上游默认只允许 HTTPS
- 拒绝上游重定向和含凭据的上游 URL
- 不记录请求正文、API Key 或推理内容
- macOS 单文件二进制，不需要外部运行时

## 确认启用 Codex Multi-agent v2

本项目通过 Codex 多代理功能调度 `kimi_frontend` 自定义子代理：

1. 打开 Codex Desktop 设置。
2. 在“实验功能”或“功能”中找到 **Multi-agent v2** 并启用。
3. 完全退出并重新打开 Codex Desktop。

较新版本可能已默认启用。如果设置中没有开关，但 Codex 已能显示和调度子代理，则无需额外修改。

也可以检查：

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

需要手动配置时，在现有 `[features]` 表中加入：

```toml
[features]
multi_agent_v2 = true
```

不要创建重复的 `[features]` 表。

## Codex Provider 配置

把下面内容安全合并到用户级 `~/.codex/config.toml`；不要覆盖无关配置，也不要把 Key 写入 TOML：

```toml
[agents]
enabled = true

[model_providers.codex_kimi_bridge]
name = "Kimi via Codex Kimi Bridge"
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

## `kimi_frontend` 子代理示例

下面是 Allegretto 会员调用 K3 1M 上下文的完整模板。实际文件位于 [`install/templates/kimi_frontend.toml`](install/templates/kimi_frontend.toml)：

```toml
name = "kimi_frontend"
description = "使用 Kimi K3 审查和优化前端视觉、交互、响应式布局与实现方案。"

model_provider = "codex_kimi_bridge"
model = "k3"
model_context_window = 1048576
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
model_supports_reasoning_summaries = false

sandbox_mode = "read-only"

developer_instructions = """
你是一名专注于前端体验与视觉质量的高级设计工程师。

工作重点：
- 检查视觉层级、排版、间距、配色与信息密度
- 检查组件一致性和设计系统
- 检查桌面端与移动端响应式布局
- 检查交互反馈、动效、可访问性和操作流程
- 结合现有代码判断建议的实现成本与维护风险
- 如果有页面截图，优先结合截图与代码进行判断

输出结构：
1. 必须修复的问题
2. 影响体验的主要问题
3. 推荐的具体优化方案
4. 可直接交给主代理实施的修改清单

默认只读，不直接修改文件。
结论必须具体，避免只有“更现代”“更美观”等抽象描述。
完成后向主代理返回简明、可执行的总结。
"""
```

`xhigh` 会转换为 Kimi K3 的 `max`。角色指令可以按任务修改；若只希望 Kimi 提供建议，请保留 `sandbox_mode = "read-only"` 和只读指令。

## 按会员等级选择 Kimi Code 模型

所有 Kimi Code 会员使用同一种 Kimi Code Key 和相同上游：

```text
https://api.kimi.com/coding/v1/chat/completions
```

区别在于模型和上下文权限：

| Kimi 会员等级 | 推荐模型 ID | 上下文窗口 | 自动压缩值 | 适用场景 |
| --- | --- | ---: | ---: | --- |
| Andante／所有会员 | `kimi-for-coding` | `262144` | `230000` | 日常开发与代码补全 |
| Moderato | `k3-256k` | `262144` | `230000` | 推荐；节省配额 |
| Moderato | `k3` | `262144` | `230000` | K3、无 1M 权限 |
| Allegretto 及以上 | `k3` | `1048576` | `900000` | 大型仓库与长上下文 |
| Allegretto 及以上 | `kimi-for-coding-highspeed` | `262144` | `230000` | 优先输出速度 |

模型权限以 [Kimi Code 官方模型配置](https://www.kimi.com/code/docs/en/kimi-code/models.html) 为准。

切换模型时同时修改 `~/.codex/agents/kimi_frontend.toml` 并用同一模型启动桥接：

```sh
codex-kimi-bridge serve --model k3-256k
```

重启桥接，再重新打开 Codex Desktop 或新建任务。运行实时诊断时也使用相同模型：

```sh
codex-kimi-bridge doctor --live --json --model k3-256k
```

这条命令会调用 Kimi 并消耗少量额度。

## Kimi API 开放平台 Key（进阶）

Kimi Code 会员 Key 与 Kimi API 开放平台 Key 属于不同产品，Key、模型 ID 和地址不能混用。

国际开放平台：

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

中国大陆开放平台：

```sh
codex-kimi-bridge serve \
  --upstream https://api.moonshot.cn/v1/chat/completions \
  --model kimi-k3
```

开放平台路线在 `0.2.0-alpha.2` 中仍属于进阶配置。发布验收的主要路线是 Kimi Code 会员 Key。

## 保存或更换 API Key

Key 只保存在 macOS Keychain：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

必须把 `-w` 放在最后，在终端提示中直接输入。不要把 Key 发到聊天、截图或配置文件中。以后换 Key 只需重新执行同一命令，然后重启 Codex Desktop 或新建任务。

## CLI

```sh
codex-kimi-bridge --version
codex-kimi-bridge serve
codex-kimi-bridge doctor --json
codex-kimi-bridge translate-request --file compat/responses-request.json
codex-kimi-bridge request "只回复 OK"
```

- `translate-request` 完全离线。
- `doctor --json` 默认不调用 Kimi。
- `doctor --live` 和 `request` 会读取本机 Key 并产生真实请求。

## 安全设计

- 默认绑定 `127.0.0.1`；外部地址必须显式使用 `--allow-non-loopback`
- 默认只允许 HTTPS 上游
- 明文 HTTP 只允许显式指定的 loopback 测试服务
- 上游 URL 禁止嵌入用户名或密码
- 禁止跟随重定向，避免凭据转发
- 请求正文、Authorization、Key 和 reasoning 不写入日志
- reasoning 缓存只驻内存，最多 512 项或 64 MiB，两小时过期
- `request` 命令只会把 Key 发送到 loopback 地址

详见 [SECURITY.md](SECURITY.md)。

## 开发与验证

Rust 默认版：

```sh
cd rust
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo build --release
```

Node 回退版：

```sh
cd node
npm run check
npm test
npm run smoke
```

共享兼容 fixture 位于 [`compat/responses-request.json`](compat/responses-request.json)。发布验收会比较 Node 与 Rust 的离线请求转换结果。

## 已知边界

- 只安全转换 Responses 的 `function` 和 `custom` tools；托管型工具会明确报错
- 不支持 `previous_response_id`；调用方需要发送完整对话 items
- `parallel_tool_calls` 不转发
- reasoning 状态只在当前桥接进程内存中；工具链中途重启后应新开子代理任务
- Desktop 是否允许调度第三方 provider 子代理仍由 Codex 客户端决定
- Rust alpha 二进制尚未做 Apple 公证

## 许可证

[MIT](LICENSE)
