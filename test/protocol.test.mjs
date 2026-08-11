import assert from "node:assert/strict";
import { Readable } from "node:stream";
import test from "node:test";

import { BridgeError } from "../src/errors.mjs";
import {
  translateChatCompletion,
  translateChatCompletionStream,
  translateResponsesRequest,
} from "../src/protocol.mjs";
import { ReasoningStore } from "../src/reasoning-store.mjs";

test("translates a Responses request to Kimi Chat Completions", () => {
  const translated = translateResponsesRequest({
    model: "k3",
    instructions: "Review only. Do not edit.",
    input: [
      {
        type: "message",
        role: "user",
        content: [
          { type: "input_text", text: "Inspect this screenshot." },
          {
            type: "input_image",
            image_url: "data:image/png;base64,AAAA",
            detail: "high",
          },
        ],
      },
    ],
    tools: [
      {
        type: "function",
        name: "read_file",
        description: "Read a file",
        parameters: {
          type: "object",
          properties: { path: { type: "string" } },
          required: ["path"],
          additionalProperties: false,
        },
        strict: true,
      },
      {
        type: "custom",
        name: "apply_patch",
        description: "Apply a patch",
      },
    ],
    reasoning: { effort: "xhigh" },
    max_output_tokens: 4096,
    parallel_tool_calls: true,
    stream: true,
  });

  assert.equal(translated.body.model, "k3");
  assert.equal(translated.body.messages[0].role, "system");
  assert.equal(translated.body.messages[1].content[1].type, "image_url");
  assert.equal(translated.body.reasoning_effort, "max");
  assert.equal(translated.body.max_completion_tokens, 4096);
  assert.equal("parallel_tool_calls" in translated.body, false);
  assert.deepEqual(translated.body.stream_options, { include_usage: true });
  assert.match(translated.body.prompt_cache_key, /^codex_[a-f0-9]{40}$/);
  assert.equal(translated.body.tools[0].function.name, "read_file");
  assert.equal(translated.body.tools[1].function.name, "apply_patch");
  assert.equal(translated.context.toolMap.get("apply_patch").kind, "custom");
});

test("translates Responses tool history back into Chat messages", () => {
  const reasoningStore = new ReasoningStore();
  reasoningStore.set("call_1", "private preserved reasoning");
  const translated = translateResponsesRequest({
    model: "k3",
    input: [
      { role: "user", content: "Read package.json" },
      {
        type: "function_call",
        call_id: "call_1",
        name: "read_file",
        arguments: "{\"path\":\"package.json\"}",
      },
      {
        type: "function_call_output",
        call_id: "call_1",
        output: "{\"name\":\"demo\"}",
      },
    ],
    stream: false,
  }, { reasoningStore });

  assert.equal(translated.body.messages[1].role, "assistant");
  assert.equal(translated.body.messages[1].tool_calls[0].id, "call_1");
  assert.equal(
    translated.body.messages[1].reasoning_content,
    "private preserved reasoning",
  );
  assert.deepEqual(translated.body.messages[2], {
    role: "tool",
    tool_call_id: "call_1",
    content: "{\"name\":\"demo\"}",
  });
});

test("rejects unsupported built-in tools instead of silently dropping them", () => {
  assert.throws(
    () =>
      translateResponsesRequest({
        model: "k3",
        input: "Search",
        tools: [{ type: "web_search_preview" }],
      }),
    (error) =>
      error instanceof BridgeError && error.code === "unsupported_tool_type",
  );
});

test("translates a non-streaming Chat response", () => {
  const reasoningStore = new ReasoningStore();
  const request = translateResponsesRequest({
    model: "k3",
    input: "Use the tool",
    tools: [
      {
        type: "function",
        name: "read_file",
        parameters: { type: "object", properties: {} },
      },
    ],
    stream: false,
  }, { reasoningStore });
  const response = translateChatCompletion(
    {
      id: "chatcmpl_123",
      created: 123,
      model: "k3",
      choices: [
        {
          finish_reason: "tool_calls",
          message: {
            role: "assistant",
            content: "I will inspect it.",
            reasoning_content: "reason before tool use",
            tool_calls: [
              {
                id: "call_abc",
                type: "function",
                function: { name: "read_file", arguments: "{\"path\":\"a\"}" },
              },
            ],
          },
        },
      ],
      usage: {
        prompt_tokens: 12,
        completion_tokens: 5,
        total_tokens: 17,
        cached_tokens: 3,
      },
    },
    request.context,
  );

  assert.equal(response.object, "response");
  assert.equal(response.status, "completed");
  assert.equal(response.output[0].type, "message");
  assert.equal(response.output[1].type, "function_call");
  assert.equal(response.output[1].call_id, "call_abc");
  assert.equal(response.usage.input_tokens_details.cached_tokens, 3);
  assert.equal(reasoningStore.get("call_abc"), "reason before tool use");
});

test("converts streamed text to semantic Responses events", async () => {
  const upstream = sseReadable([
    {
      id: "chatcmpl_stream",
      created: 10,
      model: "k3",
      choices: [{ index: 0, delta: { role: "assistant", content: "" }, finish_reason: null }],
    },
    {
      id: "chatcmpl_stream",
      model: "k3",
      choices: [{ index: 0, delta: { content: "Hello" }, finish_reason: null }],
    },
    {
      id: "chatcmpl_stream",
      model: "k3",
      choices: [{ index: 0, delta: { content: " world" }, finish_reason: null }],
    },
    {
      id: "chatcmpl_stream",
      model: "k3",
      choices: [{
        index: 0,
        delta: {},
        finish_reason: "stop",
        usage: { prompt_tokens: 4, completion_tokens: 2, total_tokens: 6 },
      }],
    },
  ]);

  const events = [];
  for await (const event of translateChatCompletionStream(upstream, {
    model: "k3",
    originalRequest: { model: "k3", stream: true },
    toolMap: new Map(),
  })) {
    events.push(event);
  }

  assert.equal(events[0].type, "response.created");
  assert.deepEqual(
    events.filter((event) => event.type === "response.output_text.delta").map((event) => event.delta),
    ["Hello", " world"],
  );
  const completed = events.at(-1);
  assert.equal(completed.type, "response.completed");
  assert.equal(completed.response.output[0].content[0].text, "Hello world");
  assert.equal(completed.response.usage.total_tokens, 6);
});

test("converts streamed function and custom tool calls", async () => {
  const reasoningStore = new ReasoningStore();
  const toolMap = new Map([
    ["read_file", { kind: "function" }],
    ["apply_patch", { kind: "custom" }],
  ]);
  const upstream = sseReadable([
    {
      id: "chatcmpl_tools",
      model: "k3",
      choices: [
        {
          index: 0,
          delta: {
            reasoning_content: "reasoning part one ",
            tool_calls: [
              {
                index: 0,
                id: "call_read",
                type: "function",
                function: { name: "read_file", arguments: "{\"path\":" },
              },
              {
                index: 1,
                id: "call_patch",
                type: "function",
                function: { name: "apply_patch", arguments: "{\"input\":\"***" },
              },
            ],
          },
          finish_reason: null,
        },
      ],
    },
    {
      id: "chatcmpl_tools",
      model: "k3",
      choices: [
        {
          index: 0,
          delta: {
            reasoning_content: "part two",
            tool_calls: [
              { index: 0, function: { arguments: "\"a.txt\"}" } },
              { index: 1, function: { arguments: " Begin Patch\"}" } },
            ],
          },
          finish_reason: "tool_calls",
        },
      ],
    },
  ]);

  const events = [];
  for await (const event of translateChatCompletionStream(upstream, {
    model: "k3",
    originalRequest: { model: "k3", stream: true },
    toolMap,
    reasoningStore,
  })) {
    events.push(event);
  }

  const completed = events.at(-1).response;
  assert.equal(completed.output[0].type, "function_call");
  assert.equal(completed.output[0].arguments, "{\"path\":\"a.txt\"}");
  assert.equal(completed.output[1].type, "custom_tool_call");
  assert.equal(completed.output[1].input, "*** Begin Patch");
  assert.ok(events.some((event) => event.type === "response.function_call_arguments.delta"));
  assert.ok(events.some((event) => event.type === "response.custom_tool_call_input.done"));
  assert.equal(reasoningStore.get("call_read"), "reasoning part one part two");
  assert.equal(reasoningStore.get("call_patch"), "reasoning part one part two");
});

function sseReadable(objects) {
  const text =
    objects.map((object) => `data: ${JSON.stringify(object)}\n\n`).join("") +
    "data: [DONE]\n\n";
  return Readable.toWeb(Readable.from([text]));
}
