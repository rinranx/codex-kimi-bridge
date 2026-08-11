# 把仓库交给 Codex 安装

**简体中文** | [English](INSTALL-WITH-CODEX.en.md)

如果你不熟悉终端或 TOML，可以把本项目仓库交给 Codex，让它按官方文档完成大部分安装和配置工作。

这是一种接近一键的**辅助安装**，不是完全无人值守安装。为了保护你的电脑和 API Key，仍有两件事需要你亲自完成：

1. 确认 Codex 对 `~/.codex` 等明确位置的写入操作。
2. 在 macOS 钥匙串的安全输入提示中直接输入 API Key；不要把 Key 发到聊天里。

## 开始前准备

- 一台 Mac，并已安装和登录 Codex Desktop。
- Node.js 20 或更高版本；若尚未安装，可以让 Codex 先检查并指导你安装。
- 一枚可用的 Kimi Code 会员 API Key，或 Kimi API 开放平台按量付费 Key。
- 如果使用 Kimi Code，最好先确认会员等级：Andante、Moderato、Allegretto 或更高。

## 最简单的安装方法

1. 在 Codex Desktop 新建一个任务。
2. 复制下面的完整提示词并发送。
3. 如果提示词中没有填写 Key 类型或会员等级，只回答 Codex 对这一项的询问。
4. 审核并允许范围明确的安装操作。
5. Codex 要求保存 API Key 时，在它打开或指定的终端提示中输入；不要粘贴到对话框。
6. 只有你同意时，Codex 才应运行会实际连接 Kimi、可能消耗额度的测试。

可以先把这一行填写好，和提示词一起发送：

```text
我的 Key 类型／会员等级：________________
```

## 可直接复制给 Codex 的提示词

```text
请帮我从下面这个官方项目仓库安装并配置 Codex Kimi Bridge：

https://github.com/rinranx/codex-kimi-bridge

请严格遵守以下要求：

1. 先阅读仓库中的 README.md、INSTALL-WITH-CODEX.md 和 install/INSTALL-GUIDE.zh-CN.md，再执行安装。
2. 只使用这个仓库提供的 codex-kimi-bridge，不要使用 npm 上名字相似的第三方桥接包。
3. 检查 Node.js、Codex Desktop 和 Multi-agent v2 状态；不要使用 sudo npm install，也不要修改整个主目录或 npm 缓存的权限。
4. 如果我还没有说明 Key 类型和会员等级，请先问我是：
   - Kimi Code Andante
   - Kimi Code Moderato
   - Kimi Code Allegretto 或以上
   - Kimi API 开放平台按量付费 Key
5. 根据我的等级选择正确的模型、调用地址、上下文窗口和自动压缩值，不要默认所有人都有 Allegretto 权限。Kimi Code 与开放平台的 Key、模型和调用地址不能混用。
6. 不要让我把 API Key 粘贴到聊天中。请使用 macOS Keychain 的安全输入命令，并让我直接在终端提示中输入。
7. 修改 ~/.codex/config.toml 前先读取现有内容并创建备份。合并 [features]、[agents] 和 provider 配置，不要覆盖无关配置，也不要创建重复的 TOML 表。
8. 安装 kimi_frontend 子代理和仓库附带的 manage-codex-kimi-bridge 管理 Skill，并保持 sandbox_mode = "read-only"。
9. 桥接只允许监听 127.0.0.1，上游必须使用 HTTPS；不要启用 --allow-non-loopback 或 --allow-insecure-upstream。
10. 先运行不消耗 Kimi 额度的检查。任何真实 Kimi 测试前都要先征得我的确认。
11. 完成后告诉我：
    - 安装了哪些文件
    - 使用的模型和上游地址
    - 桥接是否已启动
    - 下次怎样启动
    - 是否运行过真实 Kimi 请求

不要显示、记录或复述我的 API Key。
```

## Codex 应该替你完成什么

在你授权的范围内，Codex 应该：

- 阅读项目文档，确认下载内容和 SHA-256。
- 检查 Node.js、Codex Desktop、Multi-agent v2 和现有 Codex 配置。
- 备份并安全合并 `~/.codex/config.toml`，不覆盖无关设置。
- 安装桥接命令、`kimi_frontend` 子代理和管理 Skill。
- 按 Key 类型与会员等级选择匹配的模型、上游地址和上下文设置。
- 先做语法、版本、健康状态等不调用 Kimi 的检查。
- 清楚报告它修改了什么，以及是否曾发出真实 Kimi 请求。

Codex 不应该：

- 要求你把 API Key 发到聊天中，或把它写进普通配置文件。
- 使用名称相似但并非来自本仓库的包。
- 用 `sudo npm install`，或为了修复 npm 而改动整个主目录的权限。
- 未经确认执行真实 Kimi 测试。
- 把本地服务暴露到 `0.0.0.0`，或关闭上游 HTTPS 限制。

## 如果 Codex 无法直接读取仓库

你可以手动下载 [macOS 完整安装包](https://raw.githubusercontent.com/rinranx/codex-kimi-bridge/main/downloads/codex-kimi-bridge-macos-install-kit-0.1.0.zip)，解压后把文件夹交给 Codex，并发送同一段提示词。把第一句话改成：

```text
请从我提供的本地安装包文件夹安装并配置 Codex Kimi Bridge。
```

如果 Codex 无法写入 `~/.codex`，只批准它对安装指南明确列出的文件进行写入，不必开放整个主目录。

仓库附带的 `manage-codex-kimi-bridge` 是安装完成后的管理 Skill，用于启动、停止、诊断和切换模型；它本身不是首次安装器。

需要了解每一步时，请阅读 [完整 macOS 安装指南](install/INSTALL-GUIDE.zh-CN.md)。模型和会员等级说明见 [中文 README](README.md#按会员等级选择-kimi-code-模型)。
