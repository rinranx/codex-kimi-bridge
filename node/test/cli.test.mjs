import assert from "node:assert/strict";
import { Readable, Writable } from "node:stream";
import test from "node:test";

import { runCli } from "../src/cli.mjs";

test("supports --version", async () => {
  const io = memoryIo();
  const code = await runCli(["--version"], io);
  assert.equal(code, 0);
  assert.equal(io.output(), "0.4.1\n");
});

test("supports global --json before translate-request", async () => {
  const io = memoryIo(
    JSON.stringify({
      model: "k3",
      input: "Hello",
      reasoning: { effort: "medium" },
      stream: false,
    }),
  );
  const code = await runCli(["--json", "translate-request", "-"], io);
  assert.equal(code, 0);
  const result = JSON.parse(io.output());
  assert.equal(result.ok, true);
  assert.equal(result.request.reasoning_effort, "high");
  assert.equal(result.request.messages[0].content, "Hello");
});

test("JSON errors are stable and contain no credentials", async () => {
  const io = memoryIo(
    JSON.stringify({ model: "k3", input: "secret body", tools: [{ type: "unknown" }] }),
  );
  const code = await runCli(["--json", "translate-request", "-"], io);
  assert.equal(code, 1);
  const result = JSON.parse(io.output());
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "unsupported_tool_type");
  assert.equal(io.output().includes("secret body"), false);
});

function memoryIo(stdinText = "") {
  let stdout = "";
  let stderr = "";
  return {
    stdin: Readable.from([stdinText]),
    stdout: new Writable({
      write(chunk, _encoding, callback) {
        stdout += chunk.toString();
        callback();
      },
    }),
    stderr: new Writable({
      write(chunk, _encoding, callback) {
        stderr += chunk.toString();
        callback();
      },
    }),
    output: () => stdout,
    errors: () => stderr,
  };
}
