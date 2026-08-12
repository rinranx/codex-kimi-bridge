import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { Readable } from "node:stream";
import test from "node:test";

import { createBridgeHandler } from "../src/server.mjs";

test("bridges a non-streaming request without logging the body or token", async () => {
  const captured = {};
  const logs = [];
  const handler = createBridgeHandler({
    fetchImpl: async (url, init) => {
      captured.url = url;
      captured.authorization = init.headers.authorization;
      captured.userAgent = init.headers["user-agent"];
      captured.body = JSON.parse(init.body);
      return new Response(
        JSON.stringify({
          id: "chatcmpl_mock",
          created: 123,
          model: "k3",
          choices: [
            {
              index: 0,
              finish_reason: "stop",
              message: { role: "assistant", content: "KIMI_BRIDGE_OK" },
            },
          ],
          usage: { prompt_tokens: 3, completion_tokens: 2, total_tokens: 5 },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    },
    logger: { error: (line) => logs.push(line) },
  });

  const response = await invoke(handler, {
    method: "POST",
    url: "/v1/responses",
    headers: {
      authorization: "Bearer super-secret-test-token",
      "content-type": "application/json",
    },
    body: {
      model: "k3",
      input: "PRIVATE_PROMPT_MARKER",
      reasoning: { effort: "low" },
      stream: false,
    },
  });
  const result = response.json();

  assert.equal(response.statusCode, 200);
  assert.equal(result.output[0].content[0].text, "KIMI_BRIDGE_OK");
  assert.equal(captured.authorization, "Bearer super-secret-test-token");
  assert.equal(captured.userAgent, "codex-kimi-bridge-node/0.1.0");
  assert.equal(captured.body.messages[0].content, "PRIVATE_PROMPT_MARKER");
  assert.equal(captured.body.reasoning_effort, "low");
  assert.equal(logs.join("\n").includes("super-secret-test-token"), false);
  assert.equal(logs.join("\n").includes("PRIVATE_PROMPT_MARKER"), false);
});

test("bridges Kimi Chat SSE to Responses SSE", async () => {
  const upstreamSse = [
    {
      id: "chatcmpl_sse",
      created: 100,
      model: "k3",
      choices: [{ index: 0, delta: { content: "Hello" }, finish_reason: null }],
    },
    {
      id: "chatcmpl_sse",
      model: "k3",
      choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
    },
  ].map((chunk) => `data: ${JSON.stringify(chunk)}\n\n`).join("") + "data: [DONE]\n\n";
  const handler = createBridgeHandler({
    fetchImpl: async () => new Response(upstreamSse, {
      status: 200,
      headers: { "content-type": "text/event-stream" },
    }),
    logger: { error() {} },
  });

  const response = await invoke(handler, {
    method: "POST",
    url: "/v1/responses",
    headers: {
      authorization: "Bearer test-token",
      "content-type": "application/json",
    },
    body: { model: "k3", input: "Hi", stream: true },
  });

  assert.equal(response.statusCode, 200);
  assert.match(response.text(), /event: response\.created/);
  assert.match(response.text(), /event: response\.output_text\.delta/);
  assert.match(response.text(), /event: response\.completed/);
  assert.match(response.text(), /data: \[DONE\]/);
});

test("preserves Kimi reasoning across a multi-step tool round trip", async () => {
  const upstreamBodies = [];
  const handler = createBridgeHandler({
    fetchImpl: async (_url, init) => {
      upstreamBodies.push(JSON.parse(init.body));
      if (upstreamBodies.length === 1) {
        return new Response(JSON.stringify({
          id: "chatcmpl_first",
          model: "k3",
          choices: [{
            index: 0,
            finish_reason: "tool_calls",
            message: {
              role: "assistant",
              content: null,
              reasoning_content: "private tool reasoning",
              tool_calls: [{
                id: "call_roundtrip",
                type: "function",
                function: { name: "read_file", arguments: "{\"path\":\"a.txt\"}" },
              }],
            },
          }],
        }), { status: 200, headers: { "content-type": "application/json" } });
      }
      return new Response(JSON.stringify({
        id: "chatcmpl_second",
        model: "k3",
        choices: [{
          index: 0,
          finish_reason: "stop",
          message: { role: "assistant", content: "done" },
        }],
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
    logger: { error() {} },
  });
  const headers = {
    authorization: "Bearer test-token",
    "content-type": "application/json",
  };

  const first = await invoke(handler, {
    method: "POST",
    url: "/v1/responses",
    headers,
    body: {
      model: "k3",
      input: "Read a.txt",
      tools: [{
        type: "function",
        name: "read_file",
        parameters: { type: "object", properties: {} },
      }],
      stream: false,
    },
  });
  assert.equal(first.json().output[0].call_id, "call_roundtrip");

  const second = await invoke(handler, {
    method: "POST",
    url: "/v1/responses",
    headers,
    body: {
      model: "k3",
      input: [
        { role: "user", content: "Read a.txt" },
        {
          type: "function_call",
          call_id: "call_roundtrip",
          name: "read_file",
          arguments: "{\"path\":\"a.txt\"}",
        },
        {
          type: "function_call_output",
          call_id: "call_roundtrip",
          output: "contents",
        },
      ],
      stream: false,
    },
  });

  assert.equal(second.statusCode, 200);
  assert.equal(
    upstreamBodies[1].messages[1].reasoning_content,
    "private tool reasoning",
  );
});

test("health is public but generation requires a Bearer token", async () => {
  const handler = createBridgeHandler({
    fetchImpl: async () => {
      throw new Error("fetch must not run for health or rejected auth");
    },
    logger: { error() {} },
  });

  const health = await invoke(handler, { method: "GET", url: "/health" });
  assert.equal(health.statusCode, 200);
  assert.equal(health.json().service, "codex-kimi-bridge-node");

  const denied = await invoke(handler, {
    method: "POST",
    url: "/v1/responses",
    headers: { "content-type": "application/json" },
    body: { model: "k3", input: "Hi" },
  });
  assert.equal(denied.statusCode, 401);
  assert.equal(denied.json().error.code, "missing_api_key");
});

async function invoke(handler, { method = "GET", url = "/", headers = {}, body } = {}) {
  const payload = body === undefined ? [] : [Buffer.from(JSON.stringify(body))];
  const request = Readable.from(payload);
  request.method = method;
  request.url = url;
  request.headers = headers;
  const response = new MemoryResponse();
  await handler(request, response);
  return response;
}

class MemoryResponse extends EventEmitter {
  constructor() {
    super();
    this.headers = {};
    this.statusCode = 200;
    this.headersSent = false;
    this.writableEnded = false;
    this.chunks = [];
  }

  setHeader(name, value) {
    this.headers[name.toLowerCase()] = value;
  }

  writeHead(status, headers = {}) {
    this.statusCode = status;
    this.headersSent = true;
    for (const [name, value] of Object.entries(headers)) {
      this.setHeader(name, value);
    }
    return this;
  }

  write(chunk) {
    this.headersSent = true;
    this.chunks.push(Buffer.from(chunk));
    return true;
  }

  end(chunk) {
    if (chunk !== undefined) {
      this.write(chunk);
    }
    this.writableEnded = true;
    this.emit("finish");
    return this;
  }

  text() {
    return Buffer.concat(this.chunks).toString("utf8");
  }

  json() {
    return JSON.parse(this.text());
  }
}
