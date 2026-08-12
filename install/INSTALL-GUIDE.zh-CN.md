# Codex Kimi Bridge：macOS 完整安装指南

**简体中文** | [English](INSTALL-GUIDE.en.md)

本指南安装默认的 Rust 单文件版 `codex-kimi-bridge 0.2.0-alpha.2`。新用户不需要安装 Rust、Node.js 或 npm。Node 版仅作为 [`node/`](../node/) 中的备用实现。

## 1. 准备

需要：

- macOS 与 Codex Desktop
- 可用的 Kimi Code 会员 Key，或 Kimi API 开放平台 Key
- 已启用的 Codex **Multi-agent v2**

不要把 API Key 发到聊天、截图或 TOML 文件中。

## 2. 下载并核对安装包

推荐下载：

<https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-macos-install-kit-0.2.0-alpha.2.zip>

同时下载同一 Release 中的 [`INSTALL-KIT-SHA256.txt`](https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/INSTALL-KIT-SHA256.txt)，然后在终端进入下载目录运行：

```sh
shasum -a 256 codex-kimi-bridge-macos-install-kit-0.2.0-alpha.2.zip
```

输出应与校验文件完全一致。本 alpha 尚未做 Apple 公证；首次打开 `.command` 时，可能需要右键文件并选择“打开”。

## 3. 第一次从 Node 0.1.0 迁移时清理命令冲突

全新安装者跳过本节。

旧 npm 版与新 Rust 版都曾使用 `codex-kimi-bridge` 命令。先在旧桥终端按 `Control+C`，再运行：

```sh
npm uninstall --global codex-kimi-bridge
command -v codex-kimi-bridge
```

第二条命令不应再指向旧 npm 位置。不要删除 Keychain 项目、provider 配置或 `kimi_frontend.toml`；它们可以继续使用。

## 4. 安装 Rust 二进制

解压完整安装包，然后双击：

```text
install-codex-kimi-bridge.command
```

安装位置固定为：

```text
~/.local/bin/codex-kimi-bridge
```

安装器会验证版本。若发现其他位置的同名命令，会停止并提示，不会自动卸载；更新已有的 `~/.local/bin` 版本前会询问并保留备份。安装器不会修改 Codex 配置、Keychain 或 shell PATH。

验证：

```sh
$HOME/.local/bin/codex-kimi-bridge --version
```

预期输出：

```text
0.2.0-alpha.2
```

如果希望在任意终端直接输入命令，可把下面一行加入 `~/.zprofile`，再新开终端：

```sh
export PATH="$HOME/.local/bin:$PATH"
```

这一步不是双击启动器的必要条件。

## 5. 启用 Multi-agent v2

在 Codex Desktop 的“设置 → 实验功能／功能”中启用 **Multi-agent v2**，然后完全退出并重新打开 Codex Desktop。

也可以检查：

```sh
codex features list | grep -E '^multi_agent(_v2)?[[:space:]]'
```

若版本需要手动配置，请把这一项合并到 `~/.codex/config.toml` 中现有的 `[features]` 表；不要创建重复表：

```toml
[features]
multi_agent_v2 = true
```

如果设置中没有开关，但 Codex 已能显示和调度自定义子代理，则无需重复添加。

## 6. 选择 Key、模型与上游

### Kimi Code 会员 Key

所有会员 Key 使用同一个上游：

```text
https://api.kimi.com/coding/v1/chat/completions
```

按实际权限设置 `~/.codex/agents/kimi_frontend.toml`：

| 会员 | `model` | `model_context_window` | `model_auto_compact_token_limit` |
| --- | --- | ---: | ---: |
| Andante／所有会员 | `kimi-for-coding` | `262144` | `230000` |
| Moderato（推荐节省配额） | `k3-256k` | `262144` | `230000` |
| Moderato（K3） | `k3` | `262144` | `230000` |
| Allegretto 及以上 | `k3` | `1048576` | `900000` |
| Allegretto 及以上（高速） | `kimi-for-coding-highspeed` | `262144` | `230000` |

安装包模板默认使用 Allegretto 的 `k3` 1M 配置。没有相应权限时必须同时修改上表三项。

### Kimi API 开放平台 Key

开放平台与会员 Key 是不同产品，Key、模型和上游不能混用。启动桥接时需要显式指定上游，例如：

```sh
$HOME/.local/bin/codex-kimi-bridge serve \
  --upstream https://api.moonshot.ai/v1/chat/completions \
  --model kimi-k3
```

中国大陆开放平台把上游换为 `https://api.moonshot.cn/v1/chat/completions`。同时把子代理里的 `model` 改为账号实际可用的开放平台模型。开放平台路线属于进阶配置；请以 Kimi 官方控制台显示的模型权限为准。

## 7. 把 Key 保存到 macOS Keychain

运行：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

保持 `-w` 在最后。终端出现提示后直接输入 Key；输入不会回显。服务名仍是 `codex-kimi-code-api-key`，不需要因为项目使用 Rust 而改名。

以后更换 Key，重新运行同一命令即可，然后重启 Codex Desktop 或新建任务。

## 8. 合并 Codex provider 配置

若配置文件已经存在，先备份：

```sh
if test -f "$HOME/.codex/config.toml"; then
  cp "$HOME/.codex/config.toml" "$HOME/.codex/config.toml.backup.$(date +%Y%m%d-%H%M%S)"
fi
```

读取现有文件，把安装包 `templates/config-kimi-provider.toml` 的内容合并进去。不要整文件覆盖，也不要创建重复的 `[features]`、`[agents]` 或 provider 表。核心配置是：

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

Keychain 命令只把 Key 交给 Codex provider，不会把 Key 写进 TOML。

## 9. 安装 `kimi_frontend` 子代理

```sh
mkdir -p "$HOME/.codex/agents"
cp templates/kimi_frontend.toml "$HOME/.codex/agents/kimi_frontend.toml"
```

然后按第 6 节修改模型、上下文窗口和自动压缩值。完整角色模板默认只读，并会向主代理返回前端视觉、交互、响应式、可访问性和实施成本方面的可执行建议。

## 10. 安装管理 Skill（可选）

```sh
mkdir -p "$HOME/.codex/skills"
cp -R companion-skill/manage-codex-kimi-bridge "$HOME/.codex/skills/"
```

这个 Skill 用于以后启动、诊断、切换模型和排查 8787 端口；它不是桥接程序本身。

## 11. 启动与检查

安装后任选一种启动方式：

| 方式 | 操作 | 特点 |
| --- | --- | --- |
| Codex 按需启动 | 对 Codex 说“使用 `$manage-codex-kimi-bridge` 检查并启动 Rust 桥接；不要运行 `doctor --live`” | 不用时可完全退出，但通常依赖当前任务／终端会话 |
| 可见终端启动 | 双击 `start-codex-kimi-bridge.command` | 可看日志，按 `Control+C` 停止 |
| 登录后自动启动 | 双击 `install-launchagent.command` | 后台常驻，异常退出自动恢复，适合每天使用 |

三种方式启动的是同一个 Rust 二进制，不要同时重复运行。LaunchAgent 本身只是 macOS `launchd` 的配置，不增加独立管理进程；占用内存的是同一个桥接进程。

### 方式 A：让 Codex 按需启动

在 Codex Desktop 中发送：

```text
使用 $manage-codex-kimi-bridge 检查并启动 Rust 桥接。只做本机健康检查，不要运行 doctor --live。
```

### 方式 B：双击启动器

Kimi Code 默认配置可直接双击：

```text
start-codex-kimi-bridge.command
```

其他 Kimi Code 模型从终端启动，例如：

```sh
$HOME/.local/bin/codex-kimi-bridge serve --model k3-256k
```

窗口显示 `implementation: rust` 即为新默认版。保持窗口打开；停止时按 `Control+C`。

### 方式 C：安装 macOS LaunchAgent

双击：

```text
install-launchagent.command
```

它会生成并加载：

```text
~/Library/LaunchAgents/io.github.rinranx.codex-kimi-bridge.plist
```

登录后会自动启动，异常退出时由 `launchd` 恢复。标准日志位于 `~/Library/Logs/codex-kimi-bridge.log`，错误日志位于 `~/Library/Logs/codex-kimi-bridge.error.log`；日志不包含请求正文或凭据。

查看 LaunchAgent：

```sh
launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"
```

随附模板使用默认 Kimi Code 上游。Kimi API 开放平台用户需要先修改 plist 的 `ProgramArguments`，加入对应的 `--upstream` 和 `--model`，或者选择前两种启动方式。

不再需要自动启动时，双击 `uninstall-launchagent.command`。它会停止该 LaunchAgent，并把 plist 移到废纸篓；不会删除二进制、Codex 配置、Keychain 或日志。

### 健康检查

另开终端执行离线／本机检查：

```sh
curl -s http://127.0.0.1:8787/health
$HOME/.local/bin/codex-kimi-bridge doctor --json
```

健康信息应包含：

```json
{"service":"codex-kimi-bridge","implementation":"rust","version":"0.2.0-alpha.2"}
```

`doctor --json` 不会联系 Kimi。只有明确愿意消耗少量额度时才运行：

```sh
$HOME/.local/bin/codex-kimi-bridge doctor --live --json --model k3
```

最后完全重启 Codex Desktop，在任务中要求主代理“使用 `kimi_frontend` 子代理审查这个前端”。

## 12. 常见问题

- **8787 已占用**：先确认占用者，不要直接结束未知进程。旧桥窗口可按 `Control+C` 停止。Rust 和 Node 版不能同时监听 8787。
- **仍启动 0.1.0**：运行 `command -v codex-kimi-bridge`；若指向全局 npm 位置，卸载旧包，再使用 `~/.local/bin/codex-kimi-bridge`。
- **`command not found`**：使用完整路径，或按第 4 节把 `~/.local/bin` 加入 PATH。
- **macOS 阻止打开**：先核对 SHA-256，再右键 `.command` 选择“打开”。
- **401／missing_api_key**：检查 Keychain 服务名和 provider 的 auth 命令，不要输出 Key 本身。
- **模型无权限**：按会员等级同时修改子代理的三项模型配置；新建任务后重试。
- **LaunchAgent 未启动**：运行 `launchctl print "gui/$(id -u)/io.github.rinranx.codex-kimi-bridge"`，并查看 `~/Library/Logs/codex-kimi-bridge.error.log`。
- **LaunchAgent 反复重启**：通常是 8787 被其他程序占用。先识别占用者，不要直接结束未知进程。
- **Rust 兼容问题**：停止 Rust 版后，按 [`node/install/INSTALL-GUIDE.zh-CN.md`](../node/install/INSTALL-GUIDE.zh-CN.md) 安装不同命令名的 Node 回退版。

## 13. 卸载

如果安装过 LaunchAgent，先双击 `uninstall-launchagent.command`。然后停止其他手动桥接，把二进制移到废纸篓：

```sh
mv "$HOME/.local/bin/codex-kimi-bridge" "$HOME/.Trash/codex-kimi-bridge"
```

Codex provider、子代理、Skill 和 Keychain 项目是独立配置，只有确定不再使用时才分别删除。卸载 Rust 二进制不会自动删除这些内容。
