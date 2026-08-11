# Codex Kimi Bridge 全新安装指南（macOS）

适用版本：`codex-kimi-bridge 0.1.0`
配置核对日期：2026-08-11

这份指南用于在一台 Mac 上首次安装以下工作流：

```text
Codex Desktop 主代理
        │
        └── kimi_frontend 子代理（Kimi K3 / 只读）
                    │
                    └── 本机桥接 127.0.0.1:8787
                              │
                              └── Kimi Code HTTPS API
```

## 安装包内容

完整安装包包含下面四项；不要把 API Key 写进任何文件：

- `codex-kimi-bridge-0.1.0.tgz`
- `SHA256SUMS`
- 本指南
- `templates/` 目录

推荐下载完整的 `codex-kimi-bridge-macos-install-kit-0.1.0.zip`，校验 SHA-256 后再解压安装。

## 第 1 步：安装 Codex Desktop 和 Node.js

1. 安装并登录最新版 Codex Desktop。
2. 安装 Node.js 20 或更高版本。已有 Homebrew 时可运行：

   ```sh
   brew install node
   ```

   没有 Homebrew 时，从 Node.js 官方网站安装当前受支持版本即可。

3. 验证：

   ```sh
   node --version
   npm --version
   ```

   `node --version` 必须至少为 `v20`。

## 第 2 步：验证并安装桥接包

把安装包解压到例如 `~/Documents/CodexKimiBridge`，然后进入该目录：

```sh
cd "$HOME/Documents/CodexKimiBridge"
shasum -a 256 -c SHA256SUMS
```

预期输出：

```text
codex-kimi-bridge-0.1.0.tgz: OK
```

安装为全局命令：

```sh
npm install --global ./codex-kimi-bridge-0.1.0.tgz
command -v codex-kimi-bridge
codex-kimi-bridge --version
```

预期版本为 `0.1.0`。不要使用 `sudo npm install`。若 Homebrew Node 的全局安装出现权限错误，先保留原始错误信息，不要修改整个主目录权限。

## 第 3 步：在 macOS Keychain 保存 Kimi Code API Key

建议在 Kimi Code 控制台创建一枚专用于此桥接的 Kimi Code API Key。不要通过聊天、配置文件或截图传递 Key。

运行：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

必须把 `-w` 放在最后。终端出现提示后粘贴 Key；输入过程不会显示字符。

只验证条目是否存在，不输出 Key：

```sh
/usr/bin/security find-generic-password \
  -s "codex-kimi-code-api-key" >/dev/null \
  && echo "Kimi Keychain item: OK"
```

## 第 4 步：配置 Codex 自定义 provider

个人 provider 必须写入用户级配置 `~/.codex/config.toml`，不要写进项目的 `.codex/config.toml`。

先确保文件存在，再打开：

```sh
mkdir -p "$HOME/.codex"
touch "$HOME/.codex/config.toml"
open -e "$HOME/.codex/config.toml"
```

把 `templates/config-kimi-provider.toml` 的内容合并到文件中。

注意：

- 如果已经有 `[agents]`，只把对应键加进现有表，不要创建第二个 `[agents]`。
- 如果已经有 `[model_providers.codex_kimi_bridge]`，更新现有表，不要重复粘贴。
- 不要把 API Key 直接写进 TOML。
- 不要求添加 `multi_agent_v2`。截至本指南日期，当前 Codex 版本默认启用子代理工作流；未来优先遵循当时的 Codex 设置界面和官方文档。

需要合并的内容是：

```toml
[agents]
enabled = true
max_concurrent_threads_per_session = 4

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

保存并关闭 TextEdit。

## 第 5 步：安装 `kimi_frontend` 自定义子代理

Codex 会自动读取 `~/.codex/agents/*.toml` 中的个人自定义代理。

执行：

```sh
mkdir -p "$HOME/.codex/agents"
cp ./templates/kimi_frontend.toml "$HOME/.codex/agents/kimi_frontend.toml"
```

检查文件：

```sh
sed -n '1,220p' "$HOME/.codex/agents/kimi_frontend.toml"
```

这个代理的关键设置是：

- `model_provider = "codex_kimi_bridge"`
- `model = "k3"`
- `model_context_window = 1048576`
- `model_reasoning_effort = "xhigh"`，桥接器会映射为 Kimi 的 `max`
- `sandbox_mode = "read-only"`

## 第 6 步：启动桥接

在一个独立终端运行：

```sh
codex-kimi-bridge serve
```

看到下面几类信息即表示服务已启动：

```text
codex-kimi-bridge 0.1.0 listening on http://127.0.0.1:8787
upstream: https://api.kimi.com
privacy: request bodies and credentials are not logged
```

保留这个终端窗口。每次重启 Mac 后都要重新启动桥接。也可以双击安装包中的 `start-installed-codex-kimi-bridge.command`。

## 第 7 步：验证本机、Key 和真实 Kimi 请求

另开一个终端。

检查本机服务：

```sh
curl -s http://127.0.0.1:8787/health
codex-kimi-bridge doctor --json
```

执行一次小型真实请求；这一步会消耗少量 Kimi 额度：

```sh
codex-kimi-bridge doctor --live --json
```

也可以经过本机 Responses 桥接测试：

```sh
codex-kimi-bridge request "只回复 OK"
```

## 第 8 步：重启 Codex Desktop 并测试子代理

1. 完全退出 Codex Desktop，不只是关闭窗口。
2. 保持桥接终端运行。
3. 重新打开 Codex Desktop，新建对话。
4. 输入：

   ```text
   请使用 kimi_frontend 子代理，只读审查当前项目的前端视觉、交互和响应式布局，等它完成后汇总结论。
   ```

5. 在 Desktop 的子代理活动区确认出现 `kimi_frontend` 工作线程。

如果当前 Codex 版本仍限制实验性第三方 provider 子代理，桥接健康检查仍可能成功，但 Desktop 会拒绝调度。这属于 Codex 客户端限制，不代表 API Key 或桥接损坏。

## 可选：安装 Codex 管理 Skill

安装包还附带 `manage-codex-kimi-bridge` Skill，可以让 Codex 帮你诊断、启动和检查桥接。它不是运行桥接所必需的。

```sh
mkdir -p "$HOME/.codex/skills"
cp -R ./companion-skill/manage-codex-kimi-bridge \
  "$HOME/.codex/skills/manage-codex-kimi-bridge"
```

安装后完全重启 Codex Desktop。可以这样调用：

```text
请使用 $manage-codex-kimi-bridge 检查并安全启动本地 Kimi 桥接。
```

## 以后更换 API Key

不需要改桥接或 TOML。再次执行：

```sh
/usr/bin/security add-generic-password \
  -U \
  -a "$USER" \
  -s "codex-kimi-code-api-key" \
  -w
```

然后完全重启 Codex Desktop 或新建对话。桥接本身不会缓存 API Key。

## 以后升级桥接

1. 停止当前正在运行的桥接终端：按 `Control-C`。
2. 验证新安装包的 SHA-256。
3. 安装新包：

   ```sh
   npm install --global ./新的-codex-kimi-bridge.tgz
   codex-kimi-bridge --version
   ```

4. 重新运行 `codex-kimi-bridge serve`。

不要在未完成的工具调用中途重启桥接；Kimi 的 preserved reasoning 状态只保存在当前桥接进程内存中。

## 故障排查

### 8787 端口被占用

```sh
lsof -nP -iTCP:8787 -sTCP:LISTEN
```

确认 PID 后再停止对应进程：

```sh
kill -INT <PID>
```

### `codex-kimi-bridge: command not found`

```sh
npm list --global --depth=0
npm bin --global 2>/dev/null || npm prefix --global
```

重新打开终端后再试。不要改用名字相似的 npm 包。

### HTTP 401 或 `missing_api_key`

```sh
/usr/bin/security find-generic-password \
  -s "codex-kimi-code-api-key" >/dev/null \
  && echo OK
```

如果没有输出 `OK`，重新执行第 3 步。不要运行带 `-w` 的查找命令并把结果贴到聊天中，因为那会显示 Key。

### `/health` 无法连接

桥接没有运行，或 8787 被别的进程占用。回到第 6 步启动服务。

### Codex 看不到 `kimi_frontend`

确认：

```sh
test -f "$HOME/.codex/agents/kimi_frontend.toml" && echo "agent file: OK"
rg -n '^(name|description|developer_instructions|model_provider|model)\s*=' \
  "$HOME/.codex/agents/kimi_frontend.toml"
```

自定义代理文件必须包含 `name`、`description` 和 `developer_instructions`。修复后完全重启 Codex Desktop。

### Kimi 返回模型权限或额度错误

确认使用 Kimi Code API Key，而不是其他平台的 Key；确认会员仍允许调用 `k3`，并检查 Kimi Code 控制台中的额度和 Key 状态。

## 安全检查清单

- [ ] 安装包里没有 API Key
- [ ] Key 只保存在 macOS Keychain
- [ ] provider URL 是 `http://127.0.0.1:8787/v1`
- [ ] 桥接上游是 `https://api.kimi.com`
- [ ] 没有使用 `--allow-non-loopback`
- [ ] `kimi_frontend` 保持 `sandbox_mode = "read-only"`
- [ ] 真实测试结束后没有把完整响应或 Key 发到公开位置

## 配置更新提醒

Codex 和 Kimi Code 都会更新。如果安装日期距离 2026-08-11 较久，先核对：

- OpenAI Codex 配置参考：`https://learn.chatgpt.com/docs/config-file/config-reference`
- OpenAI Codex 子代理文档：`https://learn.chatgpt.com/docs/agent-configuration/subagents`
- Kimi Code 文档：`https://www.kimi.com/code/docs/en/`

不要直接沿用未来已被官方删除或改名的实验字段；provider 的 `base_url`、`wire_api`、认证命令字段和自定义代理必填字段尤其需要复核。
