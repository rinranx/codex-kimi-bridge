import {
  createHash,
  createHmac,
  randomBytes,
  randomUUID,
  timingSafeEqual,
} from "node:crypto";
import {
  chmodSync,
  closeSync,
  existsSync,
  fsyncSync,
  mkdirSync,
  openSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

import { BridgeError } from "./errors.mjs";

export const ENVELOPE_PREFIX = "CKB1.";

const KEY_BYTES = 32;
const MAX_ENVELOPE_BYTES = 1024 * 1024;
const MAX_TASK_BYTES = 256 * 1024;
const DEFAULT_ENVELOPE_TTL_SECONDS = 6 * 60 * 60;
const MAX_ENVELOPE_TTL_SECONDS = 24 * 60 * 60;
const MAX_CLOCK_SKEW_SECONDS = 5 * 60;
const PROMPT_RETENTION_MS = 24 * 60 * 60 * 1000;
const TARGET_RETENTION_MS = 24 * 60 * 60 * 1000;
const TARGET_RECORD_VERSION = 1;
const TARGET_AGENT_TYPE = "kimi_frontend";
const TASK_OPEN = "[KIMI_TASK]";
const TASK_CLOSE = "[/KIMI_TASK]";

export class HandoffVerifier {
  constructor(key) {
    if (!Buffer.isBuffer(key) || key.length !== KEY_BYTES) {
      throw handoffError(
        "The local handoff signing key is malformed.",
        "invalid_handoff_key",
      );
    }
    this.key = Buffer.from(key);
  }

  verifyForRecipient(envelope, recipient, now = unixSeconds()) {
    if (typeof envelope !== "string" || Buffer.byteLength(envelope) > MAX_ENVELOPE_BYTES) {
      throw invalidEnvelope("The local handoff envelope is too large or invalid.");
    }
    if (!envelope.startsWith(ENVELOPE_PREFIX)) {
      throw invalidEnvelope("The local handoff envelope prefix is invalid.");
    }
    const encoded = envelope.slice(ENVELOPE_PREFIX.length);
    const separator = encoded.indexOf(".");
    if (separator <= 0 || encoded.indexOf(".", separator + 1) !== -1) {
      throw invalidEnvelope("The local handoff envelope structure is invalid.");
    }
    const payload = decodeHex(encoded.slice(0, separator));
    const suppliedSignature = decodeHex(encoded.slice(separator + 1));
    if (!payload || !suppliedSignature || suppliedSignature.length !== KEY_BYTES) {
      throw invalidEnvelope("The local handoff encoding or signature is invalid.");
    }
    const expectedSignature = createHmac("sha256", this.key).update(payload).digest();
    if (!timingSafeEqual(suppliedSignature, expectedSignature)) {
      throw invalidEnvelope("The local handoff signature could not be verified.");
    }

    let value;
    try {
      value = JSON.parse(payload.toString("utf8"));
    } catch {
      throw invalidEnvelope("The local handoff payload is not valid JSON.");
    }
    if (!value || typeof value !== "object" || Array.isArray(value) || value.version !== 1) {
      throw invalidEnvelope("The local handoff payload version or structure is invalid.");
    }
    for (const field of ["task_name", "session_id", "turn_id"]) {
      if (!safeIdentifier(value[field], field === "task_name" ? 256 : 128)) {
        throw invalidEnvelope("The local handoff routing metadata is invalid.");
      }
    }
    if (value.agent_type !== TARGET_AGENT_TYPE) {
      throw invalidEnvelope("The local handoff agent type is invalid.");
    }
    const recipientTaskName = String(recipient).split("/").at(-1);
    if (recipientTaskName !== value.task_name) {
      throw invalidEnvelope("The local handoff recipient does not match the spawned task.");
    }
    if (
      !Number.isSafeInteger(value.created_at) ||
      !Number.isSafeInteger(value.expires_at) ||
      value.created_at > now + MAX_CLOCK_SKEW_SECONDS ||
      value.expires_at < now ||
      value.expires_at <= value.created_at ||
      value.expires_at - value.created_at > MAX_ENVELOPE_TTL_SECONDS
    ) {
      throw invalidEnvelope("The local handoff envelope is expired or has invalid timing.");
    }
    if (
      typeof value.task !== "string" ||
      value.task.trim().length === 0 ||
      Buffer.byteLength(value.task) > MAX_TASK_BYTES
    ) {
      throw invalidEnvelope("The local handoff task is empty or too large.");
    }
    return value.task;
  }
}

export function defaultHandoffStateDir() {
  const home = process.env.HOME || homedir();
  if (!home) {
    throw handoffError(
      "HOME is unavailable, so the local handoff directory cannot be resolved.",
      "handoff_state_unavailable",
    );
  }
  if (process.platform === "darwin") {
    return join(home, "Library", "Caches", "codex-kimi-bridge", "handoff-v1");
  }
  return join(
    process.env.XDG_CACHE_HOME || join(home, ".cache"),
    "codex-kimi-bridge",
    "handoff-v1",
  );
}

export function loadHandoffVerifierIfPresent(stateDir = defaultHandoffStateDir()) {
  const keyPath = join(stateDir, "handoff.key");
  if (!existsSync(keyPath)) {
    return null;
  }
  try {
    return new HandoffVerifier(decodeKey(readFileSync(keyPath, "utf8").trim()));
  } catch (error) {
    if (error instanceof BridgeError) {
      throw error;
    }
    throw handoffError(
      "The local handoff signing key could not be read.",
      "handoff_key_unavailable",
    );
  }
}

export function captureUserPrompt(input, stateDir = defaultHandoffStateDir()) {
  const object = requireHookEvent(input, "UserPromptSubmit");
  const sessionId = hookIdentifier(object, "session_id");
  const turnId = hookIdentifier(object, "turn_id");
  const prompt = typeof object.prompt === "string" ? object.prompt : "";
  if (!prompt.trim()) {
    throw handoffError(
      "The UserPromptSubmit hook did not contain a visible prompt.",
      "missing_hook_prompt",
    );
  }
  if (Buffer.byteLength(prompt) > MAX_TASK_BYTES) {
    throw handoffError(
      "The visible user prompt is too large for a local Kimi handoff.",
      "hook_prompt_too_large",
    );
  }
  const promptsDir = join(stateDir, "prompts");
  ensurePrivateDirectory(stateDir);
  ensurePrivateDirectory(promptsDir);
  cleanupStalePrompts(promptsDir);
  writePrivateAtomic(promptPath(promptsDir, sessionId, turnId), prompt);
}

export function rewritePreToolUse(
  input,
  stateDir = defaultHandoffStateDir(),
  now = unixSeconds(),
) {
  const object = requireHookEvent(input, "PreToolUse");
  const toolInput = object.tool_input;
  if (!toolInput || typeof toolInput !== "object" || Array.isArray(toolInput)) {
    return null;
  }
  if (isFollowupTool(object.tool_name)) {
    if (typeof toolInput.target !== "string") {
      return null;
    }
    try {
      const sessionId = hookIdentifier(object, "session_id");
      if (isRegisteredKimiTarget(stateDir, sessionId, toolInput.target, now)) {
        return deniedCrossProviderFollowupOutput();
      }
      return null;
    } catch {
      return deniedFollowupGuardUnavailableOutput();
    }
  }
  if (!isSpawnTool(object.tool_name)) {
    return null;
  }
  if (toolInput.agent_type !== TARGET_AGENT_TYPE) {
    return null;
  }
  try {
    const updatedInput = rewriteKimiAgentCall(object, toolInput, stateDir, now);
    return {
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "allow",
        updatedInput,
      },
    };
  } catch {
    return {
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason:
          "Kimi handoff preparation failed. Send a new visible user task and retry; the provider was not contacted.",
      },
    };
  }
}

function rewriteKimiAgentCall(hook, toolInput, stateDir, now) {
  const sessionId = hookIdentifier(hook, "session_id");
  const turnId = hookIdentifier(hook, "turn_id");
  if (!safeIdentifier(toolInput.task_name, 256)) {
    throw handoffError(
      "The Kimi spawn task_name is missing or invalid.",
      "invalid_hook_input",
    );
  }
  const explicitTask = typeof toolInput.message === "string"
    ? extractMarkedTask(toolInput.message)
    : null;
  let task = explicitTask;
  if (!task) {
    let prompt;
    try {
      prompt = readFileSync(
        promptPath(join(stateDir, "prompts"), sessionId, turnId),
        "utf8",
      );
    } catch {
      throw handoffError(
        "No captured user prompt or explicit marked task is available for this Kimi spawn.",
        "missing_handoff_prompt",
      );
    }
    task = extractTask(prompt);
  }
  if (!task || Buffer.byteLength(task) > MAX_TASK_BYTES) {
    throw handoffError(
      "The captured Kimi task is empty or too large.",
      "invalid_handoff_task",
    );
  }
  const key = loadOrCreateKey(stateDir);
  const payload = {
    version: 1,
    session_id: sessionId,
    turn_id: turnId,
    task_name: toolInput.task_name,
    agent_type: TARGET_AGENT_TYPE,
    created_at: now,
    expires_at: now + DEFAULT_ENVELOPE_TTL_SECONDS,
    task,
  };
  const envelope = signPayload(key, payload);
  registerKimiTarget(stateDir, sessionId, toolInput.task_name, now);
  return {
    ...toolInput,
    message: envelope,
    fork_turns: "none",
  };
}

function deniedCrossProviderFollowupOutput() {
  return {
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason:
        "[unsupported_cross_provider_followup] codex-kimi-bridge blocked a follow-up to a running Kimi subagent because Codex would wrap it in provider-private encrypted state. Wait for automatic completion. For new instructions, submit a new visible [KIMI_TASK] and create a new Kimi subagent. The target was not contacted.",
    },
  };
}

function deniedFollowupGuardUnavailableOutput() {
  return {
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason:
        "[kimi_followup_guard_unavailable] codex-kimi-bridge could not verify whether this target is a Kimi subagent, so the follow-up was blocked safely. The target was not contacted.",
    },
  };
}

function isSpawnTool(toolName) {
  return ["agent", "spawnagent", "collaborationspawnagent"]
    .includes(normalizedToolName(toolName));
}

function isFollowupTool(toolName) {
  return [
    "sendmessage",
    "followuptask",
    "collaborationsendmessage",
    "collaborationfollowuptask",
  ].includes(normalizedToolName(toolName));
}

function normalizedToolName(toolName) {
  return typeof toolName === "string"
    ? toolName.replace(/[^A-Za-z0-9]/g, "").toLowerCase()
    : "";
}

function registerKimiTarget(stateDir, sessionId, taskName, now) {
  const targetsDir = join(stateDir, "targets");
  ensurePrivateDirectory(stateDir);
  ensurePrivateDirectory(targetsDir);
  cleanupStaleTargets(targetsDir);
  const record = {
    version: TARGET_RECORD_VERSION,
    session_id: sessionId,
    task_name: taskName,
    created_at: now,
    expires_at: now + DEFAULT_ENVELOPE_TTL_SECONDS,
  };
  writePrivateAtomic(
    targetRecordPath(targetsDir, sessionId, taskName),
    JSON.stringify(record),
  );
}

function isRegisteredKimiTarget(stateDir, sessionId, target, now) {
  const taskName = target.split("/").at(-1);
  if (!safeIdentifier(taskName, 256)) {
    return false;
  }
  const path = targetRecordPath(join(stateDir, "targets"), sessionId, taskName);
  if (!existsSync(path)) {
    return false;
  }
  let record;
  try {
    record = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw handoffError(
      "The local Kimi target record could not be read.",
      "handoff_target_unavailable",
    );
  }
  const matchesTarget = record &&
    typeof record === "object" &&
    !Array.isArray(record) &&
    record.version === TARGET_RECORD_VERSION &&
    record.session_id === sessionId &&
    record.task_name === taskName &&
    Number.isSafeInteger(record.created_at) &&
    Number.isSafeInteger(record.expires_at);
  if (!matchesTarget) {
    throw handoffError(
      "The local Kimi target record is invalid.",
      "handoff_target_unavailable",
    );
  }
  if (record.expires_at < now) {
    rmSync(path, { force: true });
    return false;
  }
  if (
    record.created_at > now + MAX_CLOCK_SKEW_SECONDS ||
    record.expires_at <= record.created_at ||
    record.expires_at - record.created_at > MAX_ENVELOPE_TTL_SECONDS
  ) {
    throw handoffError(
      "The local Kimi target record timing is invalid.",
      "handoff_target_unavailable",
    );
  }
  return true;
}

function targetRecordPath(targetsDir, sessionId, taskName) {
  const digest = createHash("sha256")
    .update(sessionId)
    .update("\0")
    .update(taskName)
    .digest("hex");
  return join(targetsDir, `${digest}.json`);
}

function signPayload(key, payload) {
  const bytes = Buffer.from(JSON.stringify(payload), "utf8");
  const signature = createHmac("sha256", key).update(bytes).digest();
  return `${ENVELOPE_PREFIX}${bytes.toString("hex")}.${signature.toString("hex")}`;
}

function loadOrCreateKey(stateDir) {
  ensurePrivateDirectory(stateDir);
  const keyPath = join(stateDir, "handoff.key");
  if (existsSync(keyPath)) {
    return decodeKey(readFileSync(keyPath, "utf8").trim());
  }
  const key = randomBytes(KEY_BYTES);
  try {
    const descriptor = openSync(keyPath, "wx", 0o600);
    try {
      writeFileSync(descriptor, `${key.toString("hex")}\n`);
      fsyncSync(descriptor);
    } finally {
      closeSync(descriptor);
    }
    chmodSync(keyPath, 0o600);
    return key;
  } catch (error) {
    if (error?.code === "EEXIST") {
      return decodeKey(readFileSync(keyPath, "utf8").trim());
    }
    throw handoffError(
      "The local handoff signing key could not be created.",
      "handoff_key_unavailable",
    );
  }
}

function decodeKey(encoded) {
  const key = decodeHex(encoded);
  if (!key || key.length !== KEY_BYTES) {
    throw handoffError(
      "The local handoff signing key is malformed.",
      "invalid_handoff_key",
    );
  }
  return key;
}

function decodeHex(value) {
  if (typeof value !== "string" || value.length % 2 !== 0 || !/^[0-9a-f]*$/i.test(value)) {
    return null;
  }
  return Buffer.from(value, "hex");
}

function extractTask(prompt) {
  return extractMarkedTask(prompt) ?? prompt.trim();
}

function extractMarkedTask(prompt) {
  const open = prompt.lastIndexOf(TASK_OPEN);
  if (open !== -1) {
    const tail = prompt.slice(open + TASK_OPEN.length);
    const close = tail.indexOf(TASK_CLOSE);
    if (close !== -1 && tail.slice(0, close).trim()) {
      return tail.slice(0, close).trim();
    }
  }
  return null;
}

function promptPath(promptsDir, sessionId, turnId) {
  return join(promptsDir, `${sessionId}--${turnId}.txt`);
}

function ensurePrivateDirectory(path) {
  try {
    mkdirSync(path, { recursive: true, mode: 0o700 });
    chmodSync(path, 0o700);
  } catch {
    throw handoffError(
      "The local handoff directory could not be secured.",
      "handoff_state_unavailable",
    );
  }
}

function writePrivateAtomic(path, text) {
  ensurePrivateDirectory(dirname(path));
  const temporary = join(dirname(path), `.handoff-${randomUUID()}.tmp`);
  try {
    writeFileSync(temporary, text, { encoding: "utf8", mode: 0o600, flag: "wx" });
    chmodSync(temporary, 0o600);
    renameSync(temporary, path);
    chmodSync(path, 0o600);
  } catch {
    rmSync(temporary, { force: true });
    throw handoffError(
      "The visible user prompt could not be stored for the local handoff.",
      "handoff_state_unavailable",
    );
  }
}

function cleanupStalePrompts(promptsDir) {
  const now = Date.now();
  try {
    for (const name of readdirSync(promptsDir)) {
      if (!name.endsWith(".txt")) {
        continue;
      }
      const path = join(promptsDir, name);
      if (now - statSync(path).mtimeMs > PROMPT_RETENTION_MS) {
        rmSync(path, { force: true });
      }
    }
  } catch {
    // Cleanup is best-effort and must not block a valid handoff.
  }
}

function cleanupStaleTargets(targetsDir) {
  const now = Date.now();
  try {
    for (const name of readdirSync(targetsDir)) {
      if (!name.endsWith(".json")) {
        continue;
      }
      const path = join(targetsDir, name);
      if (now - statSync(path).mtimeMs > TARGET_RETENTION_MS) {
        rmSync(path, { force: true });
      }
    }
  } catch {
    // Cleanup is best-effort and must not block a valid handoff.
  }
}

function requireHookEvent(input, expected) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw handoffError("The Codex hook input must be a JSON object.", "invalid_hook_input");
  }
  if (input.hook_event_name !== expected) {
    throw handoffError("The Codex hook event name is invalid.", "invalid_hook_input");
  }
  return input;
}

function hookIdentifier(object, field) {
  if (!safeIdentifier(object[field], 128)) {
    throw handoffError(
      `The Codex hook ${field} is missing or invalid.`,
      "invalid_hook_input",
    );
  }
  return object[field];
}

function safeIdentifier(value, maxLength) {
  return typeof value === "string" &&
    value.length > 0 &&
    value.length <= maxLength &&
    /^[A-Za-z0-9_.:@-]+$/.test(value);
}

function invalidEnvelope(message) {
  return new BridgeError(message, {
    type: "invalid_request_error",
    code: "invalid_handoff_envelope",
    param: "input",
  });
}

function handoffError(message, code) {
  return new BridgeError(message, { code });
}

function unixSeconds() {
  return Math.floor(Date.now() / 1000);
}
