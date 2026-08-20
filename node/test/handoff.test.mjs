import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  ENVELOPE_PREFIX,
  captureUserPrompt,
  loadHandoffVerifierIfPresent,
  rewritePreToolUse,
} from "../src/handoff.mjs";
import { translateResponsesRequest } from "../src/protocol.mjs";

test("signs, verifies, and translates a local Kimi handoff", () => {
  const stateDir = mkdtempSync(join(tmpdir(), "codex-kimi-node-handoff-"));
  const now = Math.floor(Date.now() / 1000);
  try {
    captureUserPrompt({
      hook_event_name: "UserPromptSubmit",
      session_id: "session_node",
      turn_id: "turn_node",
      prompt: "[KIMI_TASK]\nReturn KIMI_NODE_SIGNED_OK.\n[/KIMI_TASK]",
    }, stateDir);
    const hookOutput = rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "session_node",
      turn_id: "turn_node",
      tool_name: "spawn_agent",
      tool_input: {
        agent_type: "kimi_frontend",
        task_name: "node_signed_protocol",
        message: "gAAAA_ORIGINAL_PROVIDER_STATE",
        fork_turns: "4",
      },
    }, stateDir, now);
    assert.equal(hookOutput.hookSpecificOutput.permissionDecision, "allow");
    assert.equal(hookOutput.hookSpecificOutput.updatedInput.fork_turns, "none");
    const envelope = hookOutput.hookSpecificOutput.updatedInput.message;
    assert.match(envelope, /^CKB1\./);
    assert.equal(envelope.includes("KIMI_NODE_SIGNED_OK"), false);
    assert.equal(envelope.includes("gAAAA"), false);

    const verifier = loadHandoffVerifierIfPresent(stateDir);
    assert.equal(
      verifier.verifyForRecipient(envelope, "/root/node_signed_protocol", now + 1),
      "Return KIMI_NODE_SIGNED_OK.",
    );
    const translated = translateResponsesRequest({
      model: "k3",
      input: [{
        type: "agent_message",
        author: "/root",
        recipient: "/root/node_signed_protocol",
        content: [
          { type: "input_text", text: "Delegated task\n\nPayload:\n" },
          { type: "encrypted_content", encrypted_content: envelope },
        ],
      }],
      stream: false,
    }, { handoffVerifier: verifier });
    const upstream = JSON.stringify(translated.body);
    assert.equal(upstream.includes("KIMI_NODE_SIGNED_OK"), true);
    assert.equal(upstream.includes(ENVELOPE_PREFIX), false);
    assert.equal(upstream.includes("encrypted_content"), false);
    assert.equal(statSync(join(stateDir, "handoff.key")).mode & 0o777, 0o600);
    const targetRecord = readdirSync(join(stateDir, "targets"))[0];
    assert.equal(statSync(join(stateDir, "targets", targetRecord)).mode & 0o777, 0o600);

    const tampered = `${envelope.slice(0, -1)}${envelope.endsWith("0") ? "1" : "0"}`;
    assert.throws(
      () => verifier.verifyForRecipient(tampered, "/root/node_signed_protocol", now + 1),
      (error) => error.code === "invalid_handoff_envelope",
    );
  } finally {
    rmSync(stateDir, { recursive: true, force: true });
  }
});

test("denies a Kimi spawn when no visible prompt was captured", () => {
  const stateDir = mkdtempSync(join(tmpdir(), "codex-kimi-node-handoff-"));
  try {
    const output = rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "session_node",
      turn_id: "turn_node",
      tool_name: "spawn_agent",
      tool_input: {
        agent_type: "kimi_frontend",
        task_name: "missing_prompt",
        message: "gAAAA",
      },
    }, stateDir, 1_000);
    assert.equal(output.hookSpecificOutput.permissionDecision, "deny");
    assert.match(
      output.hookSpecificOutput.permissionDecisionReason,
      /provider was not contacted/,
    );
  } finally {
    rmSync(stateDir, { recursive: true, force: true });
  }
});

test("does not rewrite non-Kimi Agent calls", () => {
  const output = rewritePreToolUse({
    hook_event_name: "PreToolUse",
    session_id: "session_node",
    turn_id: "turn_node",
    tool_name: "spawn_agent",
    tool_input: {
      agent_type: "worker",
      task_name: "ordinary_worker",
      message: "visible",
    },
  }, join(tmpdir(), "unused-codex-kimi-node-state"), 1_000);
  assert.equal(output, null);
});

test("an explicit marked tool task supports recursive handoff without a prompt cache", () => {
  const stateDir = mkdtempSync(join(tmpdir(), "codex-kimi-node-handoff-"));
  try {
    const output = rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "session_recursive",
      turn_id: "turn_recursive",
      tool_name: "spawn_agent",
      tool_input: {
        agent_type: "kimi_frontend",
        task_name: "recursive_kimi",
        message: "[KIMI_TASK]\nReturn KIMI_NODE_RECURSIVE_OK.\n[/KIMI_TASK]",
      },
    }, stateDir, 1_000);
    assert.equal(output.hookSpecificOutput.permissionDecision, "allow");
    const verifier = loadHandoffVerifierIfPresent(stateDir);
    assert.equal(
      verifier.verifyForRecipient(
        output.hookSpecificOutput.updatedInput.message,
        "/root/recursive_kimi",
        1_001,
      ),
      "Return KIMI_NODE_RECURSIVE_OK.",
    );
  } finally {
    rmSync(stateDir, { recursive: true, force: true });
  }
});

test("blocks followups to registered Kimi targets only", () => {
  const stateDir = mkdtempSync(join(tmpdir(), "codex-kimi-node-handoff-"));
  try {
    captureUserPrompt({
      hook_event_name: "UserPromptSubmit",
      session_id: "session_followup",
      turn_id: "turn_spawn",
      prompt: "[KIMI_TASK]\nReview once.\n[/KIMI_TASK]",
    }, stateDir);
    rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "session_followup",
      turn_id: "turn_spawn",
      tool_name: "spawn_agent",
      tool_input: {
        agent_type: "kimi_frontend",
        task_name: "registered_kimi",
        message: "gAAAA",
      },
    }, stateDir, 1_000);

    for (const toolName of [
      "send_message",
      "followup_task",
      "collaborationsend_message",
      "collaboration.followup_task",
    ]) {
      const output = rewritePreToolUse({
        hook_event_name: "PreToolUse",
        session_id: "session_followup",
        turn_id: "turn_later",
        tool_name: toolName,
        tool_input: {
          target: "/root/registered_kimi",
          message: "gAAAA_OPAQUE_PROVIDER_STATE",
        },
      }, stateDir, 1_001);
      assert.equal(output.hookSpecificOutput.permissionDecision, "deny");
      assert.match(
        output.hookSpecificOutput.permissionDecisionReason,
        /unsupported_cross_provider_followup/,
      );
    }

    const ordinary = rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "session_followup",
      turn_id: "turn_later",
      tool_name: "send_message",
      tool_input: { target: "/root/ordinary_worker", message: "visible" },
    }, stateDir, 1_001);
    assert.equal(ordinary, null);

    const otherSession = rewritePreToolUse({
      hook_event_name: "PreToolUse",
      session_id: "different_session",
      turn_id: "turn_later",
      tool_name: "followup_task",
      tool_input: { target: "/root/registered_kimi", message: "visible" },
    }, stateDir, 1_001);
    assert.equal(otherSession, null);
  } finally {
    rmSync(stateDir, { recursive: true, force: true });
  }
});
