# `agent_message` integration check

This is the manual release gate for native Codex Desktop subagent visibility. It complements the automated Rust and Node conversion tests, which use a fake upstream and consume no Kimi quota.

## v0.4.0 result

Passed on 2026-08-12 with Codex Desktop Multi-agent v2 and the Rust `0.4.0` bridge. Both handoff hooks were reported as trusted and enabled, one native `kimi_frontend` task was created with `fork_turns = "none"`, and its ordinary final-result channel returned `KIMI_BRIDGE_V2_FINAL_OK` exactly. No retry or child tool call was used.

Run this check only with the user's explicit consent because it sends real task text to Kimi and consumes account quota.

## Preconditions

1. Build and install the Rust binary under test.
2. Restart the bridge and verify `/health` reports version `0.4.0` and `implementation: rust`.
3. Run `codex-kimi-bridge hooks status --json` and require both managed hooks. If installation is needed, run `hooks install`, restart Desktop, open `/hooks`, review both commands, and trust them.
4. Confirm Codex Desktop Multi-agent v2 is enabled and `kimi_frontend` uses `model_provider = "codex_kimi_bridge"`.
5. Do not enable request-body or credential logging.

## Cases

From a Codex Desktop test thread, send this as the user's visible request:

```text
[KIMI_TASK]
Return KIMI_SIGNED_HANDOFF_OK and briefly state that the task arrived as readable delegated text.
[/KIMI_TASK]
```

The primary agent must then invoke the real `spawn_agent` tool once in that turn, selecting `kimi_frontend`, a unique `task_name`, and `fork_turns = "none"`. Do not use `send_message` or `followup_task` in this check.

For each case, verify all of the following:

- A native subagent task appears in the Desktop subagent panel.
- Its status advances beyond creation and reaches completion.
- No `unsupported_input_item`, `unsupported_content_part`, or `invalid_agent_message` error appears.
- The child receives `Return KIMI_SIGNED_HANDOFF_OK...` as readable task text and does not report an empty payload.
- At least one saved assistant message carries a valid `phase`; terminal text must be `final_answer`, while tool-progress text must be `commentary`.
- `KIMI_SIGNED_HANDOFF_OK` is returned exactly to the parent through the normal final-result channel; `Payload 为空` is a failure.
- Automated offline tests confirm that neither the `CKB1` envelope nor any unknown `encrypted_content` value appears in the translated upstream request. Do not add body logging for the live check.
- The bridge log contains no request body, API key, Authorization header, or internal turn ID.

Do not replace this check with a direct plain-text bridge request: that path does not validate Codex subagent UI or `agent_message` transport.

---

# `agent_message` 集成验收

这是验证 Codex Desktop 原生子智能体可见性的手动发布门槛。自动化 Rust 与 Node 测试使用假上游，不消耗 Kimi 额度；两者互为补充。

## v0.4.0 结果

已于 2026-08-12 使用 Codex Desktop Multi-agent v2 与 Rust `0.4.0` 桥接通过。两条交接 Hook 均由 Desktop 报告为已信任并启用；只创建了一次 `fork_turns = "none"` 的原生 `kimi_frontend` 任务，普通最终结果通道原样返回 `KIMI_BRIDGE_V2_FINAL_OK`。没有重试，也没有调用子代理工具。

真实测试会把任务文本发送给 Kimi 并消耗账户额度，因此只能在用户明确同意后运行。

## 前置条件

1. 构建并安装待验收的 Rust 二进制。
2. 重启桥接，确认 `/health` 返回版本 `0.4.0` 和 `implementation: rust`。
3. 运行 `codex-kimi-bridge hooks status --json`，确认两条托管 Hook 都存在。需要安装时运行 `hooks install`，重启 Desktop，打开 `/hooks`，逐条检查并信任。
4. 确认 Codex Desktop 已启用 Multi-agent v2，且 `kimi_frontend` 使用 `model_provider = "codex_kimi_bridge"`。
5. 不得开启请求正文或凭据日志。

## 用例

在 Codex Desktop 测试会话中，把下面内容作为用户的可见请求发送：

```text
[KIMI_TASK]
请原样返回 KIMI_SIGNED_HANDOFF_OK，并简短说明任务以可读的委派文本到达。
[/KIMI_TASK]
```

主代理随后在这一轮只调用一次真实 `spawn_agent`，选择 `kimi_frontend`、唯一的 `task_name` 和 `fork_turns = "none"`。本项验收不要使用 `send_message` 或 `followup_task`。

每次都必须确认：

- Desktop 子智能体面板出现原生任务。
- 状态成功进入运行并最终完成。
- 没有 `unsupported_input_item`、`unsupported_content_part` 或 `invalid_agent_message`。
- 子代理收到“请原样返回 KIMI_SIGNED_HANDOFF_OK……”这段可读任务，并且不报告空 Payload。
- 保存的助手消息至少有一条带合法 `phase`；终止文本必须为 `final_answer`，工具过程文本必须为 `commentary`。
- `KIMI_SIGNED_HANDOFF_OK` 通过普通最终结果通道原样返回主代理；返回“Payload 为空”即失败。
- 自动化离线测试必须确认转换后的上游请求既没有 `CKB1` 信封，也没有未知 `encrypted_content` 值；实时验收不得为此增加正文日志。
- 桥接日志没有请求正文、API Key、Authorization header 或内部 turn ID。

不能用普通纯文本直连代替这项验收，因为直连无法验证 Codex 子智能体 UI 与 `agent_message` 传输。
