# 把仓库交给 Codex 安装

**简体中文** | [English](INSTALL-WITH-CODEX.en.md)

不熟悉终端或 TOML 时，可以把项目地址交给 Codex Desktop，让它安装默认的 Rust 单文件版、配置 provider、子代理和管理 Skill。

这接近一键安装，但为了安全仍需要你：

1. 审核对 `~/.local/bin`、`~/.codex` 等明确路径的写入授权。
2. 只在 macOS Keychain 的终端安全提示中输入 API Key，绝不把 Key 发到聊天。
3. 决定是否运行会实际连接 Kimi 并消耗少量额度的测试。

安装者不需要 Rust、Node.js 或 npm。只有默认 Rust 版遇到兼容问题时，才使用 `node/` 中独立命名的 `codex-kimi-bridge-node`。

## 使用方法

在 Codex Desktop 新建任务，先填写这一行：

```text
我的 Key 类型／会员等级：Kimi Code Andante／Moderato／Allegretto 及以上／Kimi API 开放平台（选择一项）
```

然后把下面整段复制发送：

```text
请从这个项目安装并配置默认的 Rust 版 Codex Kimi Bridge：

https://github.com/rinranx/codex-kimi-bridge

请严格遵守以下要求：

1. 先完整阅读 README.md、INSTALL-WITH-CODEX.md、install/INSTALL-GUIDE.zh-CN.md、SECURITY.md，以及 GitHub Release v0.2.0-alpha.2 随附的 SHA-256 校验文件。
2. 默认安装 Rust 单文件版 codex-kimi-bridge 0.2.0-alpha.2。不要安装名字相似的 npm 包，也不要把 node/ 回退版当成默认版。新安装不要求 Rust、Node.js 或 npm。
3. 只从本仓库固定版本的 GitHub Release v0.2.0-alpha.2 下载 macOS 安装包，先核对 SHA-256，再安装到 ~/.local/bin/codex-kimi-bridge。不要使用 sudo，也不要从 main/downloads 获取构建产物。
4. 安装前只读检查 command -v codex-kimi-bridge 和 8787 端口。如果发现旧的 npm 0.1.0 同名命令或未知占用，先报告具体路径／进程并征得我的确认；不要擅自卸载、覆盖或结束进程。得到确认后，旧官方 npm 包可用 npm uninstall --global codex-kimi-bridge 移除。
5. 如果我还没说明 Key 类型和会员等级，请先问清。根据 README 的表格选择正确的模型、上游、上下文窗口和自动压缩值；不要默认所有人都有 Allegretto 1M 权限。Kimi Code 会员 Key 与 Kimi API 开放平台 Key、模型和地址不能混用。
6. 不要让我把 API Key 粘贴到聊天。使用 macOS Keychain 命令，让我只在终端安全提示中输入；服务名保持 codex-kimi-code-api-key。不要显示、记录或复述 Key。
7. 检查 Codex Multi-agent v2。若尚未启用，优先指导我在 Desktop 实验功能中开启；只有当前版本需要时才安全合并 multi_agent_v2 = true，不能创建重复的 [features] 表。
8. 修改 ~/.codex/config.toml 前读取现有内容并创建带时间戳备份。只合并 [agents] 和 model_providers.codex_kimi_bridge 等所需键，不覆盖无关配置，不创建重复 TOML 表，不把 Key 写进 TOML。
9. 从仓库模板安装 ~/.codex/agents/kimi_frontend.toml，按我的等级修改 model、model_context_window 和 model_auto_compact_token_limit，并保持 sandbox_mode = "read-only"。
10. 安装 companion-skill/manage-codex-kimi-bridge 到 ~/.codex/skills/。桥接只监听 127.0.0.1，上游使用 HTTPS；不要开启 --allow-non-loopback 或 --allow-insecure-upstream。
11. 让我从三种启动方式中选择一项，并说明差异：A）在 Codex 中调用 manage-codex-kimi-bridge 按需启动；B）双击 start-codex-kimi-bridge.command，在可见终端运行；C）安装 macOS LaunchAgent，登录后后台自动启动并在异常退出后恢复。三种方式不能同时重复监听 8787。LaunchAgent 配置本身不增加独立常驻进程，内存来自同一个 Rust 桥接。
12. 只有我选择 C 时，才安装仓库随附的 install-launchagent.command。确认 plist 使用绝对的 ~/.local/bin/codex-kimi-bridge 路径，只监听 loopback，并说明怎样用 uninstall-launchagent.command 停止自动启动。Kimi API 开放平台不能直接套用默认 Kimi Code LaunchAgent 参数。
13. 启动时必须确认输出包含 implementation: rust。先运行版本、SHA-256、health、doctor --json 和离线 translate-request 等不会调用 Kimi 的检查。
14. doctor --live 或 request 会真实调用 Kimi 并消耗额度，必须在运行前单独征得我的明确同意。没有同意就跳过，并在结果中说明未运行真实请求。
15. 只有 Rust 版经确认存在兼容问题时，才提出 node/ 回退方案。回退命令必须是 codex-kimi-bridge-node，并且不能与 Rust 版同时监听 8787。
16. 完成后列出：下载文件及 SHA-256、安装路径与版本、Key 类型、模型、上游、改动和备份的配置文件、Multi-agent v2 状态、我选择的启动方式、桥接是否启动、下次怎样启动、是否发出过真实 Kimi 请求。
```

## Codex 应完成的结果

- `~/.local/bin/codex-kimi-bridge --version` 输出 `0.2.0-alpha.2`
- `/health` 显示 `implementation: rust`
- API Key 只在 Keychain 中
- `~/.codex/config.toml` 保留原配置且没有重复表
- `kimi_frontend` 的模型权限与实际会员／平台一致
- Multi-agent v2 已可调度自定义子代理
- 已明确选择 Codex 按需、双击终端或 LaunchAgent 三种启动方式之一
- 未经确认没有发出真实 Kimi 请求

若 Codex 无法读取 GitHub，可先手动下载[完整 macOS 安装包](https://github.com/rinranx/codex-kimi-bridge/releases/download/v0.2.0-alpha.2/codex-kimi-bridge-macos-install-kit-0.2.0-alpha.2.zip)，把解压后的文件夹交给它，并把提示词第一句改为“请从我提供的本地安装包安装”。

更细的人工步骤见[完整安装指南](install/INSTALL-GUIDE.zh-CN.md)。
