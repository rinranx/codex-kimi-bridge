import { createHash, randomUUID } from "node:crypto";

import { BridgeError } from "./errors.mjs";
import { ENVELOPE_PREFIX } from "./handoff.mjs";
import { parseSse } from "./sse.mjs";

const TEXT_PART_TYPES = new Set(["input_text", "output_text", "text"]);
const IGNORED_INPUT_TYPES = new Set(["reasoning", "item_reference"]);

export function translateResponsesRequest(input, options = {}) {
  assertPlainObject(input, "request body");

  if (input.previous_response_id) {
    throw new BridgeError(
      "previous_response_id is not supported because Kimi Chat Completions is stateless. Send the conversation items in input instead.",
      { param: "previous_response_id", code: "unsupported_parameter" },
    );
  }

  const model = nonEmptyString(input.model) ?? options.defaultModel ?? "k3";
  const { chatTools, toolMap } = translateTools(input.tools ?? []);
  const messages = [];

  if (input.instructions !== undefined && input.instructions !== null) {
    messages.push({
      role: "system",
      content: coerceInstructionText(input.instructions),
    });
  }

  appendResponsesInput(
    messages,
    input.input,
    toolMap,
    options.reasoningStore,
    options.handoffVerifier,
    inputHasVisibleHandoff(input.input),
  );

  if (messages.length === 0) {
    throw new BridgeError("input must contain at least one message.", {
      param: "input",
      code: "missing_required_parameter",
    });
  }

  const stream = input.stream !== false;
  const body = {
    model,
    messages,
    stream,
    prompt_cache_key: derivePromptCacheKey(input, model, messages),
  };

  if (stream) {
    body.stream_options = { include_usage: true };
  }
  if (chatTools.length > 0) {
    body.tools = chatTools;
  }

  const toolChoice = translateToolChoice(input.tool_choice, toolMap);
  if (toolChoice !== undefined) {
    body.tool_choice = toolChoice;
  }
  const effort = translateReasoningEffort(input.reasoning?.effort);
  if (effort) {
    body.reasoning_effort = effort;
  }

  if (Number.isSafeInteger(input.max_output_tokens) && input.max_output_tokens > 0) {
    body.max_completion_tokens = input.max_output_tokens;
  }

  const responseFormat = translateResponseFormat(input.text?.format);
  if (responseFormat) {
    body.response_format = responseFormat;
  }

  for (const field of ["temperature", "top_p", "seed", "stop"]) {
    if (input[field] !== undefined && input[field] !== null) {
      body[field] = input[field];
    }
  }

  if (nonEmptyString(input.safety_identifier)) {
    body.safety_identifier = input.safety_identifier;
  }

  return {
    body,
    context: {
      model,
      originalRequest: input,
      toolMap,
      promptCacheKey: body.prompt_cache_key,
      reasoningStore: options.reasoningStore,
    },
  };
}

export function translateChatCompletion(chat, context = {}) {
  assertPlainObject(chat, "upstream response");
  const choice = chat.choices?.[0];
  if (!choice || typeof choice !== "object") {
    throw new BridgeError("The upstream response did not contain choices[0].", {
      status: 502,
      type: "upstream_protocol_error",
      code: "invalid_upstream_response",
    });
  }

  const output = [];
  const message = choice.message ?? {};
  rememberReasoningForToolCalls(
    context.reasoningStore,
    message.tool_calls,
    message.reasoning_content,
  );
  const text = normalizeAssistantText(message.content);
  const phase = assistantPhase(
    Array.isArray(message.tool_calls) && message.tool_calls.length > 0,
    choice.finish_reason,
  );
  if (text) {
    output.push(makeCompletedMessage(text, makeId("msg"), phase));
  }

  for (const toolCall of message.tool_calls ?? []) {
    output.push(makeCompletedToolCall(toolCall, context.toolMap));
  }

  const incomplete = choice.finish_reason === "length";
  return makeResponseObject({
    id: responseIdFrom(chat.id),
    createdAt: chat.created,
    model: chat.model ?? context.model ?? "k3",
    status: incomplete ? "incomplete" : "completed",
    output,
    usage: normalizeUsage(chat.usage),
    incompleteReason: incomplete ? "max_output_tokens" : null,
    originalRequest: context.originalRequest,
  });
}

export async function* translateChatCompletionStream(readable, context = {}) {
  const state = {
    responseId: null,
    chatId: null,
    createdAt: null,
    model: context.model ?? "k3",
    sequence: 0,
    output: [],
    nextOutputIndex: 0,
    message: null,
    toolCalls: new Map(),
    usage: normalizeUsage(null),
    finishReason: null,
    createdEmitted: false,
    reasoningContent: "",
  };

  for await (const frame of parseSse(readable)) {
    if (frame.data === "[DONE]") {
      break;
    }

    let chunk;
    try {
      chunk = JSON.parse(frame.data);
    } catch (error) {
      throw new BridgeError("The upstream SSE stream contained invalid JSON.", {
        status: 502,
        type: "upstream_protocol_error",
        code: "invalid_upstream_sse",
        cause: error,
      });
    }

    if (chunk.error) {
      throw Object.assign(
        new BridgeError(
          chunk.error.message ?? "The upstream provider returned a streaming error.",
          {
            status: 502,
            type: chunk.error.type ?? "upstream_provider_error",
            code: chunk.error.code ?? "upstream_provider_error",
            param: chunk.error.param ?? null,
          },
        ),
        { upstreamError: chunk.error },
      );
    }

    initializeStreamState(state, chunk, context);
    if (!state.createdEmitted) {
      state.createdEmitted = true;
      yield event(state, "response.created", {
        response: makeResponseObject({
          id: state.responseId,
          createdAt: state.createdAt,
          model: state.model,
          status: "in_progress",
          output: [],
          usage: null,
          originalRequest: context.originalRequest,
        }),
      });
      yield event(state, "response.in_progress", {
        response: makeResponseObject({
          id: state.responseId,
          createdAt: state.createdAt,
          model: state.model,
          status: "in_progress",
          output: [],
          usage: null,
          originalRequest: context.originalRequest,
        }),
      });
    }

    if (chunk.usage) {
      state.usage = normalizeUsage(chunk.usage);
    }

    const choice = chunk.choices?.[0];
    if (!choice) {
      continue;
    }
    if (choice.usage) {
      state.usage = normalizeUsage(choice.usage);
    }

    const delta = choice.delta ?? {};
    if (typeof delta.reasoning_content === "string") {
      state.reasoningContent += delta.reasoning_content;
    }
    if (typeof delta.content === "string" && delta.content.length > 0) {
      if (!state.message) {
        state.message = startMessage(state);
        yield event(state, "response.output_item.added", {
          output_index: state.message.outputIndex,
          item: state.message.inProgressItem,
        });
        yield event(state, "response.content_part.added", {
          item_id: state.message.itemId,
          output_index: state.message.outputIndex,
          content_index: 0,
          part: { type: "output_text", text: "", annotations: [], logprobs: [] },
        });
      }
      state.message.text += delta.content;
      yield event(state, "response.output_text.delta", {
        item_id: state.message.itemId,
        output_index: state.message.outputIndex,
        content_index: 0,
        delta: delta.content,
        logprobs: [],
      });
    }

    for (const toolDelta of delta.tool_calls ?? []) {
      const toolState = ensureToolState(state, toolDelta, context.toolMap);
      if (
        !toolState.added &&
        upstreamToolNameIsComplete(context.toolMap, toolState.upstreamName)
      ) {
        toolState.added = true;
        yield event(state, "response.output_item.added", {
          output_index: toolState.outputIndex,
          item: toolInProgressItem(toolState),
        });
      }

      const argumentsDelta = toolDelta.function?.arguments;
      if (typeof argumentsDelta === "string" && argumentsDelta.length > 0) {
        toolState.arguments += argumentsDelta;
        if (toolState.kind === "function" && toolState.added) {
          yield event(state, "response.function_call_arguments.delta", {
            item_id: toolState.itemId,
            output_index: toolState.outputIndex,
            delta: argumentsDelta,
          });
        }
      }
    }

    if (choice.finish_reason) {
      state.finishReason = choice.finish_reason;
    }
  }

  if (!state.createdEmitted) {
    initializeStreamState(state, {}, context);
    state.createdEmitted = true;
    yield event(state, "response.created", {
      response: makeResponseObject({
        id: state.responseId,
        createdAt: state.createdAt,
        model: state.model,
        status: "in_progress",
        output: [],
        usage: null,
        originalRequest: context.originalRequest,
      }),
    });
  }

  if (state.message) {
    const completed = makeCompletedMessage(
      state.message.text,
      state.message.itemId,
      assistantPhase(state.toolCalls.size > 0, state.finishReason),
    );
    state.output[state.message.outputIndex] = completed;
    yield event(state, "response.output_text.done", {
      item_id: state.message.itemId,
      output_index: state.message.outputIndex,
      content_index: 0,
      text: state.message.text,
      logprobs: [],
    });
    yield event(state, "response.content_part.done", {
      item_id: state.message.itemId,
      output_index: state.message.outputIndex,
      content_index: 0,
      part: completed.content[0],
    });
    yield event(state, "response.output_item.done", {
      output_index: state.message.outputIndex,
      item: completed,
    });
  }

  for (const toolState of [...state.toolCalls.values()].sort(
    (a, b) => a.outputIndex - b.outputIndex,
  )) {
    if (!toolState.added) {
      toolState.added = true;
      yield event(state, "response.output_item.added", {
        output_index: toolState.outputIndex,
        item: toolInProgressItem(toolState),
      });
    }

    const completed = completedToolStateItem(toolState);
    state.output[toolState.outputIndex] = completed;
    if (toolState.kind === "custom") {
      const input = completed.input;
      if (input.length > 0) {
        yield event(state, "response.custom_tool_call_input.delta", {
          item_id: toolState.itemId,
          output_index: toolState.outputIndex,
          delta: input,
        });
      }
      yield event(state, "response.custom_tool_call_input.done", {
        item_id: toolState.itemId,
        output_index: toolState.outputIndex,
        input,
      });
    } else {
      yield event(state, "response.function_call_arguments.done", {
        item_id: toolState.itemId,
        output_index: toolState.outputIndex,
        name: toolState.name,
        arguments: completed.arguments,
      });
    }
    yield event(state, "response.output_item.done", {
      output_index: toolState.outputIndex,
      item: completed,
    });
  }

  rememberReasoningForToolCalls(
    context.reasoningStore,
    [...state.toolCalls.values()].map((toolState) => ({ id: toolState.callId })),
    state.reasoningContent,
  );

  const incomplete = state.finishReason === "length";
  const finalResponse = makeResponseObject({
    id: state.responseId,
    createdAt: state.createdAt,
    model: state.model,
    status: incomplete ? "incomplete" : "completed",
    output: state.output.filter(Boolean),
    usage: state.usage,
    incompleteReason: incomplete ? "max_output_tokens" : null,
    originalRequest: context.originalRequest,
  });
  yield event(state, incomplete ? "response.incomplete" : "response.completed", {
    response: finalResponse,
  });
}

function appendResponsesInput(
  messages,
  input,
  toolMap,
  reasoningStore,
  handoffVerifier,
  visibleHandoffAvailable,
) {
  if (typeof input === "string") {
    messages.push({ role: "user", content: input });
    return;
  }
  if (input === undefined || input === null) {
    return;
  }
  if (!Array.isArray(input)) {
    throw new BridgeError("input must be a string or an array of Responses items.", {
      param: "input",
    });
  }

  for (const item of input) {
    if (!item || typeof item !== "object") {
      throw new BridgeError("Every input item must be an object.", {
        param: "input",
      });
    }
    if (IGNORED_INPUT_TYPES.has(item.type)) {
      continue;
    }
    if (item.type === "agent_message") {
      messages.push(translateAgentMessage(
        item,
        handoffVerifier,
        visibleHandoffAvailable,
      ));
    } else if (item.type === "message" || item.role) {
      messages.push(translateMessage(item));
    } else if (item.type === "function_call") {
      const callId = item.call_id ?? item.id ?? makeId("call");
      const upstreamName = resolveUpstreamToolName(
        toolMap,
        item.name ?? "",
        nonEmptyString(item.namespace),
        "function",
      );
      appendAssistantToolCall(messages, {
        id: callId,
        type: "function",
        function: {
          name: upstreamName,
          arguments: stringifyArguments(item.arguments),
        },
      }, reasoningStore?.get?.(callId));
    } else if (item.type === "custom_tool_call") {
      const callId = item.call_id ?? item.id ?? makeId("call");
      const upstreamName = resolveUpstreamToolName(
        toolMap,
        item.name ?? "",
        nonEmptyString(item.namespace),
        "custom",
      );
      appendAssistantToolCall(messages, {
        id: callId,
        type: "function",
        function: {
          name: upstreamName,
          arguments: JSON.stringify({ input: coerceToolOutput(item.input) }),
        },
      }, reasoningStore?.get?.(callId));
    } else if (
      item.type === "function_call_output" ||
      item.type === "custom_tool_call_output"
    ) {
      messages.push({
        role: "tool",
        tool_call_id: item.call_id,
        content: coerceToolOutput(item.output),
      });
    } else {
      throw new BridgeError(`Unsupported Responses input item type: ${item.type ?? "unknown"}.`, {
        param: "input",
        code: "unsupported_input_item",
      });
    }
  }
}

const AGENT_ROUTE_MAX_LENGTH = 256;
const AGENT_ROUTE_PATTERN = /^[A-Za-z0-9/_\-.:@]+$/;
const AGENT_MESSAGE_PREFIX_OPEN = "[Codex agent_message]";
const AGENT_MESSAGE_PREFIX_CLOSE = "[/Codex agent_message]";

function translateAgentMessage(item, handoffVerifier, visibleHandoffAvailable) {
  const author = requireAgentRoute(item.author, "author");
  const recipient = requireAgentRoute(item.recipient, "recipient");
  const metadata = JSON.stringify({ author, recipient });
  const prefix = `${AGENT_MESSAGE_PREFIX_OPEN}\n${metadata}\n${AGENT_MESSAGE_PREFIX_CLOSE}\n\n`;
  const { content: translatedContent, hadSignedHandoff } =
    translateAgentMessageContent(item.content, handoffVerifier, recipient);
  if (!contentHasUpstreamValue(translatedContent)) {
    throw new BridgeError(
      "An agent_message must contain a non-empty Kimi-compatible task payload.",
      { param: "input", code: "missing_agent_message_content" },
    );
  }
  if (
    !hadSignedHandoff &&
    !visibleHandoffAvailable &&
    agentMessageIsEmptyPayloadShell(translatedContent)
  ) {
    if (agentMessageIsFollowupShell(translatedContent)) {
      throw new BridgeError(
        "Codex wrapped a follow-up to an existing Kimi subagent in provider-private encrypted state, which this bridge cannot decrypt. Wait for automatic completion; for new instructions, submit a new visible [KIMI_TASK] and create a new Kimi subagent.",
        { param: "input", code: "unsupported_cross_provider_followup" },
      );
    }
    throw new BridgeError(
      "The Kimi subagent task payload is empty. Install and trust the Codex Kimi handoff hooks, or include a visible [KIMI_TASK] in forked history.",
      { param: "input", code: "missing_handoff_envelope" },
    );
  }
  return {
    role: "user",
    content: prependAgentMessagePrefix(
      translatedContent,
      prefix,
    ),
  };
}

function translateAgentMessageContent(content, handoffVerifier, recipient) {
  if (!Array.isArray(content)) {
    return { content: translateContent(content, "user"), hadSignedHandoff: false };
  }

  const normalized = [];
  let signedTask = null;
  for (const part of content) {
    if (!part || typeof part !== "object" || part.type !== "encrypted_content") {
      normalized.push(part);
      continue;
    }
    const envelope = typeof part.encrypted_content === "string" &&
      part.encrypted_content.startsWith(ENVELOPE_PREFIX)
      ? part.encrypted_content
      : null;
    if (envelope) {
      if (signedTask !== null) {
        throw new BridgeError(
          "An agent_message must not contain multiple local handoff envelopes.",
          { param: "input", code: "invalid_handoff_envelope" },
        );
      }
      if (!handoffVerifier) {
        throw new BridgeError(
          "A signed local handoff was received, but its verification key is unavailable.",
          { param: "input", code: "handoff_key_unavailable" },
        );
      }
      signedTask = handoffVerifier.verifyForRecipient(envelope, recipient);
    }
    // encrypted_content is opaque provider state. A third-party bridge
    // cannot decrypt it and must never reinterpret or forward it. The only
    // exception is a locally signed CKB1 envelope verified above.
  }
  if (signedTask !== null) {
    normalized.push({ type: "input_text", text: signedTask });
  }
  return {
    content: translateContent(normalized, "user"),
    hadSignedHandoff: signedTask !== null,
  };
}

function inputHasVisibleHandoff(input) {
  if (!Array.isArray(input)) {
    return false;
  }
  return input.some((item) => {
    if (!item || typeof item !== "object" || item.type === "agent_message") {
      return false;
    }
    return visibleTextFragments(item.content).some((text) => markedTask(text) !== null);
  });
}

function visibleTextFragments(content) {
  if (typeof content === "string") {
    return [content];
  }
  if (!Array.isArray(content)) {
    return [];
  }
  return content
    .filter((part) => part && typeof part === "object" && TEXT_PART_TYPES.has(part.type))
    .map((part) => part.text)
    .filter((text) => typeof text === "string");
}

function markedTask(text) {
  const open = text.lastIndexOf("[KIMI_TASK]");
  if (open === -1) {
    return null;
  }
  const tail = text.slice(open + "[KIMI_TASK]".length);
  const close = tail.indexOf("[/KIMI_TASK]");
  if (close === -1) {
    return null;
  }
  return tail.slice(0, close).trim() || null;
}

function agentMessageIsEmptyPayloadShell(content) {
  const text = agentMessageShellText(content);
  if (text === null) {
    return false;
  }
  const index = text.lastIndexOf("Payload:");
  return index !== -1 && text.slice(index + "Payload:".length).trim().length === 0;
}

function agentMessageIsFollowupShell(content) {
  const text = agentMessageShellText(content);
  return text !== null && text
    .split(/\r?\n/)
    .some((line) => line.trim() === "Message Type: MESSAGE");
}

function agentMessageShellText(content) {
  if (typeof content === "string") {
    return content;
  } else if (
    Array.isArray(content) &&
    content.every((part) => part?.type === "text")
  ) {
    return content.map((part) => part.text ?? "").join("\n");
  }
  return null;
}

function requireAgentRoute(value, field) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > AGENT_ROUTE_MAX_LENGTH ||
    !AGENT_ROUTE_PATTERN.test(value)
  ) {
    throw new BridgeError(
      `An agent_message ${field} must be 1-${AGENT_ROUTE_MAX_LENGTH} ASCII characters using only letters, numbers, /, _, -, ., :, or @.`,
      { param: "input", code: "invalid_agent_message" },
    );
  }
  return value;
}

function prependAgentMessagePrefix(content, prefix) {
  if (Array.isArray(content)) {
    return [{ type: "text", text: prefix }, ...content];
  }
  return `${prefix}${typeof content === "string" ? content : coerceToolOutput(content)}`;
}

function contentHasUpstreamValue(content) {
  if (typeof content === "string") {
    return content.trim().length > 0;
  }
  return Array.isArray(content) && content.some((part) => {
    if (part?.type === "text") {
      return typeof part.text === "string" && part.text.trim().length > 0;
    }
    return part?.type === "image_url" || part?.type === "video_url";
  });
}

function translateMessage(item) {
  const role = item.role === "developer" ? "system" : item.role;
  if (!["system", "user", "assistant", "tool"].includes(role)) {
    throw new BridgeError(`Unsupported message role: ${item.role ?? "unknown"}.`, {
      param: "input",
      code: "unsupported_message_role",
    });
  }

  const message = {
    role,
    content: translateContent(item.content, role),
  };
  if (nonEmptyString(item.name)) {
    message.name = item.name;
  }
  if (role === "tool" && nonEmptyString(item.tool_call_id ?? item.call_id)) {
    message.tool_call_id = item.tool_call_id ?? item.call_id;
  }
  return message;
}

function translateContent(content, role) {
  if (typeof content === "string") {
    return content;
  }
  if (!Array.isArray(content)) {
    return coerceToolOutput(content);
  }

  const parts = [];
  for (const part of content) {
    if (!part || typeof part !== "object") {
      continue;
    }
    if (TEXT_PART_TYPES.has(part.type)) {
      parts.push({ type: "text", text: part.text ?? "" });
    } else if (part.type === "encrypted_content") {
      // Outside agent_message, this remains opaque provider state.
      continue;
    } else if (part.type === "refusal") {
      parts.push({ type: "text", text: part.refusal ?? "" });
    } else if (part.type === "input_image" || part.type === "image_url") {
      if (role !== "user") {
        throw new BridgeError("Image content is only supported in user messages.", {
          param: "input",
          code: "unsupported_content_part",
        });
      }
      const rawUrl = part.image_url ?? part.url;
      const url = typeof rawUrl === "string" ? rawUrl : rawUrl?.url;
      if (!nonEmptyString(url)) {
        throw new BridgeError("An input_image part must contain image_url.", {
          param: "input",
        });
      }
      parts.push({
        type: "image_url",
        image_url: {
          url,
          ...(part.detail || rawUrl?.detail ? { detail: part.detail ?? rawUrl.detail } : {}),
        },
      });
    } else if (part.type === "input_video" || part.type === "video_url") {
      if (role !== "user") {
        throw new BridgeError("Video content is only supported in user messages.", {
          param: "input",
          code: "unsupported_content_part",
        });
      }
      const rawUrl = part.video_url ?? part.url;
      const url = typeof rawUrl === "string" ? rawUrl : rawUrl?.url;
      if (!nonEmptyString(url)) {
        throw new BridgeError("An input_video part must contain video_url.", {
          param: "input",
        });
      }
      parts.push({ type: "video_url", video_url: { url } });
    } else if (typeof part.text === "string") {
      parts.push({ type: "text", text: part.text });
    } else {
      throw new BridgeError(`Unsupported content part type: ${part.type ?? "unknown"}.`, {
        param: "input",
        code: "unsupported_content_part",
      });
    }
  }

  if (parts.every((part) => part.type === "text")) {
    return parts.map((part) => part.text).join("");
  }
  return parts;
}

const UPSTREAM_TOOL_NAME_LIMIT = 64;

function translateTools(tools) {
  if (!Array.isArray(tools)) {
    throw new BridgeError("tools must be an array.", { param: "tools" });
  }

  const reservedPlainNames = new Set(
    tools
      .filter(
        (tool) =>
          tool && (tool.type === "function" || tool.type === "custom"),
      )
      .map((tool) => nonEmptyString(tool.name ?? tool.function?.name))
      .filter(Boolean),
  );
  const chatTools = [];
  const toolMap = new Map();
  for (const tool of tools) {
    if (!tool || typeof tool !== "object") {
      throw new BridgeError("Every tool must be an object.", { param: "tools" });
    }

    if (tool.type === "function" || tool.type === "custom") {
      translateSingleTool(
        tool,
        null,
        null,
        chatTools,
        toolMap,
        reservedPlainNames,
      );
      continue;
    }

    if (tool.type === "namespace") {
      const namespace = requireNamespaceName(tool.name);
      if (!Array.isArray(tool.tools)) {
        throw new BridgeError("Every namespace tool must contain a tools array.", {
          param: "tools",
        });
      }
      for (const innerTool of tool.tools) {
        if (!innerTool || typeof innerTool !== "object") {
          throw new BridgeError("Every namespaced tool must be an object.", {
            param: "tools",
          });
        }
        if (innerTool.type !== "function" && innerTool.type !== "custom") {
          throw new BridgeError(
            `Unsupported tool type inside namespace ${namespace}: ${innerTool.type ?? "unknown"}. Only function and custom tools can be translated safely.`,
            { param: "tools", code: "unsupported_tool_type" },
          );
        }
        translateSingleTool(
          innerTool,
          namespace,
          typeof tool.description === "string" ? tool.description : "",
          chatTools,
          toolMap,
          reservedPlainNames,
        );
      }
      continue;
    }

    throw new BridgeError(
      `Unsupported Responses tool type: ${tool.type ?? "unknown"}. Only function, custom, and namespace tools can be translated safely.`,
      { param: "tools", code: "unsupported_tool_type" },
    );
  }
  return { chatTools, toolMap };
}

function translateSingleTool(
  tool,
  namespace,
  namespaceDescription,
  chatTools,
  toolMap,
  reservedPlainNames,
) {
  const definition = tool.function ?? tool;
  const name = requireToolName(tool.name ?? definition.name);
  const kind = tool.type === "custom" ? "custom" : "function";
  const upstreamName = registerToolMapping(toolMap, reservedPlainNames, {
    kind,
    name,
    namespace,
    original: tool,
  });
  const description = namespacedToolDescription(
    namespace,
    namespaceDescription,
    name,
    definition.description ?? (kind === "custom" ? "Accepts free-form text input." : ""),
  );

  if (kind === "custom") {
    const formatNote = definition.format
      ? `\nOriginal input constraint: ${JSON.stringify(definition.format)}`
      : "";
    chatTools.push({
      type: "function",
      function: {
        name: upstreamName,
        description: `${description}\nReturn the exact free-form tool input in the JSON field \"input\".${formatNote}`,
        parameters: {
          type: "object",
          properties: {
            input: { type: "string", description: "Exact free-form input for the tool." },
          },
          required: ["input"],
          additionalProperties: false,
        },
        strict: true,
      },
    });
    return;
  }

  chatTools.push({
    type: "function",
    function: {
      name: upstreamName,
      description,
      parameters: definition.parameters ?? {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
      ...(typeof definition.strict === "boolean" ? { strict: definition.strict } : {}),
    },
  });
}

function registerToolMapping(toolMap, reservedPlainNames, mapping) {
  if (
    [...toolMap.values()].some(
      (existing) =>
        existing.name === mapping.name &&
        (existing.namespace ?? null) === (mapping.namespace ?? null),
    )
  ) {
    throw new BridgeError(
      `Duplicate Responses tool identity: ${mapping.namespace ? `${mapping.namespace}/` : ""}${mapping.name}.`,
      { param: "tools" },
    );
  }
  if (!mapping.namespace) {
    if (toolMap.has(mapping.name)) {
      throw new BridgeError(`Duplicate Responses tool name: ${mapping.name}.`, {
        param: "tools",
      });
    }
    toolMap.set(mapping.name, mapping);
    return mapping.name;
  }

  for (let salt = 0; salt <= 0xffffffff; salt += 1) {
    const upstreamName = namespacedUpstreamName(mapping.namespace, mapping.name, salt);
    if (!toolMap.has(upstreamName) && !reservedPlainNames.has(upstreamName)) {
      toolMap.set(upstreamName, mapping);
      return upstreamName;
    }
  }
  throw new BridgeError("Unable to allocate a unique upstream tool name.", {
    param: "tools",
  });
}

function namespacedUpstreamName(namespace, name, salt = 0) {
  const namespaceHint = sanitizeToolNameComponent(namespace);
  const nameHint = sanitizeToolNameComponent(name);
  const suffix = hash(`${namespace}\0${name}\0${salt}`).slice(0, 12);
  const maxHintLength = UPSTREAM_TOOL_NAME_LIMIT - suffix.length - 1;
  const hint = `ns_${namespaceHint}_${nameHint}`.slice(0, maxHintLength);
  return `${hint}_${suffix}`;
}

function sanitizeToolNameComponent(value) {
  const sanitized = Array.from(String(value), (character) =>
    /[a-zA-Z0-9_-]/.test(character) ? character : "_"
  ).join("");
  return sanitized || "tool";
}

function namespacedToolDescription(
  namespace,
  namespaceDescription,
  name,
  description,
) {
  if (!namespace) {
    return description;
  }
  const lines = [
    `Codex namespaced tool: namespace \`${namespace}\`, tool \`${name}\`.`,
  ];
  if (namespaceDescription) {
    lines.push(`Namespace description: ${namespaceDescription}`);
  }
  if (description) {
    lines.push(description);
  }
  return lines.join("\n");
}

function findUpstreamToolName(toolMap, name, namespace, kind = null) {
  for (const [upstreamName, mapping] of toolMap ?? []) {
    if (
      mapping.name === name &&
      (mapping.namespace ?? null) === (namespace ?? null) &&
      (!kind || mapping.kind === kind)
    ) {
      return upstreamName;
    }
  }
  return null;
}

function resolveUpstreamToolName(toolMap, name, namespace, kind = null) {
  return (
    findUpstreamToolName(toolMap, name, namespace, kind) ??
    (namespace ? namespacedUpstreamName(namespace, name) : name)
  );
}

function translateToolChoice(choice, toolMap) {
  if (choice === undefined || choice === null) {
    return undefined;
  }
  if (["auto", "none", "required"].includes(choice)) {
    return choice;
  }
  if (typeof choice === "object") {
    if (choice.type === "allowed_tools") {
      return choice.mode === "required" ? "required" : "auto";
    }
    const name = choice.name ?? choice.function?.name;
    if (nonEmptyString(name)) {
      const namespace = nonEmptyString(choice.namespace ?? choice.function?.namespace);
      const upstreamName = findUpstreamToolName(toolMap, name, namespace);
      if (toolMap.size > 0 && !upstreamName) {
        throw new BridgeError(`tool_choice refers to an unknown tool: ${namespace ? `${namespace}/` : ""}${name}.`, {
          param: "tool_choice",
        });
      }
      return {
        type: "function",
        function: {
          name: upstreamName ?? (namespace ? namespacedUpstreamName(namespace, name) : name),
        },
      };
    }
  }
  throw new BridgeError("Unsupported tool_choice value.", {
    param: "tool_choice",
    code: "unsupported_parameter",
  });
}

function translateReasoningEffort(effort) {
  if (effort === undefined || effort === null) {
    return undefined;
  }
  if (["max", "ultra", "xhigh"].includes(effort)) {
    return "max";
  }
  if (["high", "medium"].includes(effort)) {
    return "high";
  }
  if (["low", "minimal", "minimum", "light"].includes(effort)) {
    return "low";
  }
  if (effort === "none") {
    return "low";
  }
  throw new BridgeError(`Unsupported reasoning effort: ${effort}.`, {
    param: "reasoning.effort",
  });
}

function translateResponseFormat(format) {
  if (!format || format.type === "text") {
    return undefined;
  }
  if (format.type === "json_object") {
    return { type: "json_object" };
  }
  if (format.type === "json_schema") {
    return {
      type: "json_schema",
      json_schema: {
        name: format.name ?? "response",
        schema: format.schema ?? {},
        ...(typeof format.strict === "boolean" ? { strict: format.strict } : {}),
        ...(format.description ? { description: format.description } : {}),
      },
    };
  }
  throw new BridgeError(`Unsupported text.format type: ${format.type}.`, {
    param: "text.format",
  });
}

function derivePromptCacheKey(input, model, messages) {
  const explicit = nonEmptyString(input.prompt_cache_key);
  if (explicit) {
    return explicit;
  }
  for (const candidate of [
    input.metadata?.session_id,
    input.metadata?.task_id,
    input.metadata?.thread_id,
    input.user,
  ]) {
    if (nonEmptyString(candidate)) {
      return `codex_${hash(candidate).slice(0, 40)}`;
    }
  }
  const stablePrefix = messages.slice(0, 2).map((message) => ({
    role: message.role,
    content: message.content,
  }));
  return `codex_${hash(JSON.stringify({ model, stablePrefix })).slice(0, 40)}`;
}

function assistantPhase(hasToolCalls, finishReason) {
  if (
    hasToolCalls ||
    ["tool_calls", "length", "content_filter"].includes(finishReason)
  ) {
    return "commentary";
  }
  return "final_answer";
}

function makeCompletedMessage(text, id = makeId("msg"), phase = "final_answer") {
  return {
    id,
    type: "message",
    status: "completed",
    role: "assistant",
    phase,
    content: [
      {
        type: "output_text",
        text,
        annotations: [],
        logprobs: [],
      },
    ],
  };
}

function makeCompletedToolCall(toolCall, toolMap = new Map()) {
  const upstreamName = toolCall?.function?.name;
  const mapping = toolMap?.get?.(upstreamName);
  const kind = mapping?.kind ?? "function";
  const name = mapping?.name ?? upstreamName;
  const namespace = mapping?.namespace ?? null;
  const callId = toolCall.id ?? makeId("call");
  if (kind === "custom") {
    return {
      id: makeId("ctc"),
      type: "custom_tool_call",
      status: "completed",
      call_id: callId,
      name,
      ...(namespace ? { namespace } : {}),
      input: extractCustomInput(toolCall.function?.arguments),
    };
  }
  return {
    id: makeId("fc"),
    type: "function_call",
    status: "completed",
    call_id: callId,
    name,
    ...(namespace ? { namespace } : {}),
    arguments: toolCall.function?.arguments ?? "{}",
  };
}

function startMessage(state) {
  const outputIndex = state.nextOutputIndex++;
  const itemId = makeId("msg");
  return {
    outputIndex,
    itemId,
    text: "",
    inProgressItem: {
      id: itemId,
      type: "message",
      status: "in_progress",
      role: "assistant",
      phase: "commentary",
      content: [],
    },
  };
}

function ensureToolState(state, delta, toolMap = new Map()) {
  const chatIndex = Number.isInteger(delta.index) ? delta.index : 0;
  let toolState = state.toolCalls.get(chatIndex);
  if (!toolState) {
    const upstreamName = delta.function?.name ?? "";
    const mapping = toolMap?.get?.(upstreamName);
    const kind = mapping?.kind ?? "function";
    toolState = {
      chatIndex,
      outputIndex: state.nextOutputIndex++,
      itemId: makeId(kind === "custom" ? "ctc" : "fc"),
      callId: delta.id ?? makeId("call"),
      upstreamName,
      name: mapping?.name ?? upstreamName,
      namespace: mapping?.namespace ?? null,
      arguments: "",
      kind,
      added: false,
    };
    state.toolCalls.set(chatIndex, toolState);
  } else {
    if (delta.id) {
      toolState.callId = delta.id;
    }
    if (delta.function?.name) {
      toolState.upstreamName += delta.function.name;
      const mapping = toolMap?.get?.(toolState.upstreamName);
      if (mapping) {
        if (!toolState.added && toolState.kind !== mapping.kind) {
          toolState.itemId = makeId(mapping.kind === "custom" ? "ctc" : "fc");
        }
        toolState.kind = mapping.kind;
        toolState.name = mapping.name;
        toolState.namespace = mapping.namespace ?? null;
      } else {
        toolState.name = toolState.upstreamName;
        toolState.namespace = null;
      }
    }
  }
  return toolState;
}

function toolInProgressItem(toolState) {
  if (toolState.kind === "custom") {
    return {
      id: toolState.itemId,
      type: "custom_tool_call",
      status: "in_progress",
      call_id: toolState.callId,
      name: toolState.name,
      ...(toolState.namespace ? { namespace: toolState.namespace } : {}),
      input: "",
    };
  }
  return {
    id: toolState.itemId,
    type: "function_call",
    status: "in_progress",
    call_id: toolState.callId,
    name: toolState.name,
    ...(toolState.namespace ? { namespace: toolState.namespace } : {}),
    arguments: "",
  };
}

function completedToolStateItem(toolState) {
  if (toolState.kind === "custom") {
    return {
      id: toolState.itemId,
      type: "custom_tool_call",
      status: "completed",
      call_id: toolState.callId,
      name: toolState.name,
      ...(toolState.namespace ? { namespace: toolState.namespace } : {}),
      input: extractCustomInput(toolState.arguments),
    };
  }
  return {
    id: toolState.itemId,
    type: "function_call",
    status: "completed",
    call_id: toolState.callId,
    name: toolState.name,
    ...(toolState.namespace ? { namespace: toolState.namespace } : {}),
    arguments: toolState.arguments || "{}",
  };
}

function upstreamToolNameIsComplete(toolMap, upstreamName) {
  if (!upstreamName) {
    return false;
  }
  if (!toolMap || toolMap.size === 0) {
    return true;
  }
  if (!toolMap.has(upstreamName)) {
    return false;
  }
  return ![...toolMap.keys()].some(
    (candidate) =>
      candidate.length > upstreamName.length && candidate.startsWith(upstreamName),
  );
}

function initializeStreamState(state, chunk, context) {
  state.chatId ??= chunk.id ?? makeId("chatcmpl");
  state.responseId ??= responseIdFrom(state.chatId);
  state.createdAt ??= Number.isFinite(chunk.created)
    ? chunk.created
    : Math.floor(Date.now() / 1000);
  state.model = chunk.model ?? state.model ?? context.model ?? "k3";
}

function event(state, type, fields) {
  return {
    type,
    sequence_number: state.sequence++,
    ...fields,
  };
}

function makeResponseObject({
  id,
  createdAt,
  model,
  status,
  output,
  usage,
  incompleteReason = null,
  originalRequest = {},
}) {
  return {
    id,
    object: "response",
    created_at: Number.isFinite(createdAt)
      ? createdAt
      : Math.floor(Date.now() / 1000),
    status,
    background: false,
    error: null,
    incomplete_details: incompleteReason ? { reason: incompleteReason } : null,
    instructions: originalRequest?.instructions ?? null,
    max_output_tokens: originalRequest?.max_output_tokens ?? null,
    model,
    output,
    parallel_tool_calls: originalRequest?.parallel_tool_calls ?? true,
    previous_response_id: null,
    prompt_cache_key: originalRequest?.prompt_cache_key ?? null,
    reasoning: originalRequest?.reasoning ?? null,
    safety_identifier: originalRequest?.safety_identifier ?? null,
    service_tier: "default",
    store: false,
    temperature: originalRequest?.temperature ?? null,
    text: originalRequest?.text ?? { format: { type: "text" } },
    tool_choice: originalRequest?.tool_choice ?? "auto",
    tools: originalRequest?.tools ?? [],
    top_p: originalRequest?.top_p ?? null,
    truncation: originalRequest?.truncation ?? "disabled",
    usage,
    user: originalRequest?.user ?? null,
    metadata: originalRequest?.metadata ?? {},
  };
}

function normalizeUsage(usage) {
  if (!usage) {
    return {
      input_tokens: 0,
      input_tokens_details: { cached_tokens: 0 },
      output_tokens: 0,
      output_tokens_details: { reasoning_tokens: 0 },
      total_tokens: 0,
    };
  }
  const inputTokens = usage.prompt_tokens ?? usage.input_tokens ?? 0;
  const outputTokens = usage.completion_tokens ?? usage.output_tokens ?? 0;
  return {
    input_tokens: inputTokens,
    input_tokens_details: {
      cached_tokens:
        usage.cached_tokens ?? usage.prompt_tokens_details?.cached_tokens ?? 0,
    },
    output_tokens: outputTokens,
    output_tokens_details: {
      reasoning_tokens:
        usage.completion_tokens_details?.reasoning_tokens ??
        usage.output_tokens_details?.reasoning_tokens ??
        0,
    },
    total_tokens: usage.total_tokens ?? inputTokens + outputTokens,
  };
}

function appendAssistantToolCall(messages, toolCall, reasoningContent) {
  const previous = messages.at(-1);
  if (
    previous?.role === "assistant" &&
    Array.isArray(previous.tool_calls) &&
    (previous.content === null || previous.content === "")
  ) {
    previous.tool_calls.push(toolCall);
    if (!previous.reasoning_content && nonEmptyString(reasoningContent)) {
      previous.reasoning_content = reasoningContent;
    }
    return;
  }
  messages.push({
    role: "assistant",
    content: null,
    tool_calls: [toolCall],
    ...(nonEmptyString(reasoningContent)
      ? { reasoning_content: reasoningContent }
      : {}),
  });
}

function rememberReasoningForToolCalls(store, toolCalls, reasoningContent) {
  if (!store?.set || !nonEmptyString(reasoningContent)) {
    return;
  }
  for (const toolCall of toolCalls ?? []) {
    store.set(toolCall?.id, reasoningContent);
  }
}

function coerceInstructionText(value) {
  if (typeof value === "string") {
    return value;
  }
  if (Array.isArray(value)) {
    return value
      .map((part) => (typeof part === "string" ? part : part?.text ?? ""))
      .join("");
  }
  throw new BridgeError("instructions must be a string or text-part array.", {
    param: "instructions",
  });
}

function coerceToolOutput(value) {
  if (typeof value === "string") {
    return value;
  }
  if (value === undefined || value === null) {
    return "";
  }
  return JSON.stringify(value);
}

function stringifyArguments(value) {
  if (typeof value === "string") {
    return value;
  }
  return JSON.stringify(value ?? {});
}

function normalizeAssistantText(content) {
  if (typeof content === "string") {
    return content;
  }
  if (Array.isArray(content)) {
    return content
      .map((part) => (typeof part === "string" ? part : part?.text ?? ""))
      .join("");
  }
  return "";
}

function extractCustomInput(argumentsText) {
  if (!argumentsText) {
    return "";
  }
  try {
    const parsed = JSON.parse(argumentsText);
    return typeof parsed?.input === "string" ? parsed.input : argumentsText;
  } catch {
    return argumentsText;
  }
}

function requireToolName(name) {
  if (!nonEmptyString(name)) {
    throw new BridgeError("Every function or custom tool must have a name.", {
      param: "tools",
    });
  }
  return name;
}

function requireNamespaceName(name) {
  if (!nonEmptyString(name)) {
    throw new BridgeError("Every namespace tool must have a name.", {
      param: "tools",
    });
  }
  return name;
}

function responseIdFrom(id) {
  if (typeof id === "string" && id.startsWith("resp_")) {
    return id;
  }
  const suffix = typeof id === "string" ? id.replace(/[^a-zA-Z0-9_-]/g, "") : makeId("r");
  return `resp_${suffix}`;
}

function makeId(prefix) {
  return `${prefix}_${randomUUID().replaceAll("-", "")}`;
}

function hash(value) {
  return createHash("sha256").update(String(value)).digest("hex");
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() ? value : null;
}

function assertPlainObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new BridgeError(`${label} must be a JSON object.`);
  }
}
