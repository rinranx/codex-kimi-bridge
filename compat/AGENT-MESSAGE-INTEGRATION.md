# `agent_message` integration check

This is the manual release gate for native Codex Desktop subagent visibility. It complements the automated Rust and Node conversion tests, which use a fake upstream and consume no Kimi quota.

## v0.4.1 status

Offline guard and protocol tests pass. The live Desktop long-task/follow-up gate below has not yet been run and must not be attempted without the user's separate explicit approval.

## v0.4.0 result

Passed on 2026-08-12 with Codex Desktop Multi-agent v2 and the Rust `0.4.0` bridge. Both handoff hooks were reported as trusted and enabled, one native `kimi_frontend` task was created with `fork_turns = "none"`, and its ordinary final-result channel returned `KIMI_BRIDGE_V2_FINAL_OK` exactly. No retry or child tool call was used.

Run this check only with the user's explicit consent because it sends real task text to Kimi and consumes account quota.

## Preconditions

1. Build and install the Rust binary under test.
2. Restart the bridge and verify `/health` reports version `0.4.1` and `implementation: rust`.
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

### v0.4.1 long-task follow-up guard

Run this case separately and only once. Give the child a small task that performs at least one ordinary read-only tool call before its final answer. As soon as the child is visibly working, the parent intentionally calls `send_message` once using the canonical task path returned by `spawn_agent`.

Require all of the following:

- `PreToolUse` denies the call with `unsupported_cross_provider_followup` before delivery.
- The follow-up text does not enter the child request or bridge log, and the denied call creates no additional Kimi request.
- The existing Kimi child remains active, completes its original task, and returns through ordinary final-result delivery.
- Offline tests confirm that an unregistered non-Kimi target remains untouched by the guard.
- Do not retry either the Kimi spawn or the follow-up. If the guard misses, stop the child and report only the sanitized hook name and error path.

---

# `agent_message` 集成验收

这是验证 Codex Desktop 原生子智能体可见性的手动发布门槛。自动化 Rust 与 Node 测试使用假上游，不消耗 Kimi 额度；两者互为补充。

## v0.4.1 状态

离线保护与协议测试已通过。下面的 Desktop 长任务／追发保护真实验收尚未运行；没有用户另行明确授权时不得执行。

## v0.4.0 结果

已于 2026-08-12 使用 Codex Desktop Multi-agent v2 与 Rust `0.4.0` 桥接通过。两条交接 Hook 均由 Desktop 报告为已信任并启用；只创建了一次 `fork_turns = "none"` 的原生 `kimi_frontend` 任务，普通最终结果通道原样返回 `KIMI_BRIDGE_V2_FINAL_OK`。没有重试，也没有调用子代理工具。

真实测试会把任务文本发送给 Kimi 并消耗账户额度，因此只能在用户明确同意后运行。

## 前置条件

1. 构建并安装待验收的 Rust 二进制。
2. 重启桥接，确认 `/health` 返回版本 `0.4.1` 和 `implementation: rust`。
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

### v0.4.1 长任务追发保护

本项单独运行且只能运行一次。给子代理一个至少会调用一次普通只读工具、随后返回最终答案的小任务。确认子代理正在工作后，主代理使用 `spawn_agent` 返回的规范任务路径，故意调用一次 `send_message`。

必须同时满足：

- `PreToolUse` 在投递前以 `unsupported_cross_provider_followup` 拒绝调用。
- 追发正文不进入子代理请求或桥接日志，被拒绝的调用不会产生额外 Kimi 请求。
- 原 Kimi 子代理保持运行，完成初始任务，并通过普通最终结果通道返回。
- 离线测试确认未登记的非 Kimi 目标不受保护逻辑影响。
- Kimi 创建和追发都不得重试。保护未命中时立即停止子代理，只报告净化后的 Hook 名称与错误路径。
