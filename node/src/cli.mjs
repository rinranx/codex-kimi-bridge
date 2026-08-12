import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";

import { BridgeError, toErrorEnvelope } from "./errors.mjs";
import {
  captureUserPrompt,
  defaultHandoffStateDir,
  loadHandoffVerifierIfPresent,
  rewritePreToolUse,
} from "./handoff.mjs";
import { translateResponsesRequest } from "./protocol.mjs";
import {
  createBridgeServer,
  DEFAULT_UPSTREAM,
  isPortAvailable,
  listen,
} from "./server.mjs";
import { parseSse } from "./sse.mjs";
import { VERSION } from "./version.mjs";

const execFileAsync = promisify(execFile);
const BOOLEAN_FLAGS = new Set([
  "json",
  "live",
  "stream",
  "quiet",
  "allow-non-loopback",
  "allow-insecure-upstream",
  "help",
  "version",
]);

export async function runCli(argv = process.argv.slice(2), io = defaultIo()) {
  const { command, flags, positionals } = parseArgs(argv);
  const json = flags.json === true;

  try {
    if ([undefined, "help", "--help", "-h"].includes(command)) {
      io.stdout.write(helpText());
      return 0;
    }
    if (["version", "--version", "-v"].includes(command)) {
      io.stdout.write(`${VERSION}\n`);
      return 0;
    }
    if (command === "serve") {
      return await serveCommand(flags, io);
    }
    if (command === "doctor") {
      return await doctorCommand(flags, io, json);
    }
    if (command === "hook") {
      return await hookCommand(flags, positionals, io);
    }
    if (command === "translate-request") {
      return await translateRequestCommand(flags, positionals, io, json);
    }
    if (command === "request") {
      return await requestCommand(flags, positionals, io, json);
    }

    throw new BridgeError(`Unknown command: ${command}. Run --help for usage.`, {
      code: "unknown_command",
    });
  } catch (error) {
    const envelope = toErrorEnvelope(error);
    if (json) {
      io.stdout.write(`${JSON.stringify({ ok: false, ...envelope })}\n`);
    } else {
      io.stderr.write(`Error: ${envelope.error.message}\n`);
    }
    return 1;
  }
}

async function serveCommand(flags, io) {
  const host = stringFlag(flags.host, "127.0.0.1");
  const port = integerFlag(flags.port, 8787);
  const upstreamUrl = stringFlag(flags.upstream, DEFAULT_UPSTREAM);
  const defaultModel = stringFlag(flags.model, "k3");
  const quiet = flags.quiet === true;
  const logger = quiet
    ? { error() {} }
    : { error: (line) => io.stderr.write(`${line}\n`) };

  const server = createBridgeServer({
    host,
    upstreamUrl,
    defaultModel,
    timeoutMs: integerFlag(flags["timeout-ms"], 7_200_000),
    maxBodyBytes: integerFlag(flags["max-body-bytes"], 128 * 1024 * 1024),
    allowNonLoopback: flags["allow-non-loopback"] === true,
    allowInsecureUpstream: flags["allow-insecure-upstream"] === true,
    logger,
    handoffStateDir: stringFlag(flags["handoff-state-dir"], defaultHandoffStateDir()),
  });
  const address = await listen(server, { host, port });
  if (!quiet) {
    io.stderr.write(
      `codex-kimi-bridge-node ${VERSION} listening on http://${host}:${address.port}\n`,
    );
    io.stderr.write(`upstream: ${new URL(upstreamUrl).origin}\n`);
    io.stderr.write("privacy: request bodies and credentials are not logged\n");
  }

  const stop = () => server.close(() => process.exit(0));
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  await new Promise((resolve, reject) => {
    server.once("close", resolve);
    server.once("error", reject);
  });
  return 0;
}

async function doctorCommand(flags, io, json) {
  const host = stringFlag(flags.host, "127.0.0.1");
  const port = integerFlag(flags.port, 8787);
  const upstreamUrl = stringFlag(flags.upstream, DEFAULT_UPSTREAM);
  const portAvailable = await isPortAvailable(host, port);
  let localService = null;

  if (!portAvailable) {
    try {
      const response = await fetch(`http://${host}:${port}/health`);
      localService = response.ok ? await response.json() : null;
    } catch {
      localService = null;
    }
  }

  const auth = await detectAuth({ readSecret: flags.live === true });
  const checks = {
    node: {
      ok: Number(process.versions.node.split(".")[0]) >= 20,
      version: process.versions.node,
      required: ">=20",
    },
    bind: {
      ok: portAvailable || localService?.service === "codex-kimi-bridge-node",
      host,
      port,
      port_available: portAvailable,
      running_service: localService?.service ?? null,
      running_version: localService?.version ?? null,
    },
    upstream: {
      ok: new URL(upstreamUrl).protocol === "https:",
      url: redactUrl(upstreamUrl),
      live_checked: false,
      reachable: null,
    },
    auth: {
      available: auth.available,
      source: auth.source,
      note: "serve mode normally receives the Bearer token from Codex and does not store it",
    },
    privacy: {
      request_body_logging: false,
      credential_logging: false,
      default_bind_is_loopback: host === "127.0.0.1",
    },
  };

  if (flags.live === true) {
    if (!auth.secret) {
      checks.upstream.live_checked = true;
      checks.upstream.reachable = false;
      checks.upstream.error =
        "No KIMI_CODE_API_KEY environment variable or macOS Keychain item was found.";
    } else {
      try {
        const response = await fetch(upstreamUrl, {
          method: "POST",
          headers: {
            authorization: `Bearer ${auth.secret}`,
            "content-type": "application/json",
            "user-agent": `codex-kimi-bridge-node/${VERSION}`,
          },
          body: JSON.stringify({
            model: stringFlag(flags.model, "k3"),
            messages: [{ role: "user", content: "Reply with OK only." }],
            max_completion_tokens: 8,
            reasoning_effort: "low",
            stream: false,
            prompt_cache_key: "codex-kimi-bridge-node-doctor",
          }),
        });
        checks.upstream.live_checked = true;
        checks.upstream.reachable = response.ok;
        if (!response.ok) {
          checks.upstream.http_status = response.status;
        }
      } catch (error) {
        checks.upstream.live_checked = true;
        checks.upstream.reachable = false;
        checks.upstream.error = error.message;
      }
    }
  }

  const ok =
    checks.node.ok &&
    checks.bind.ok &&
    checks.upstream.ok &&
    (flags.live !== true || checks.upstream.reachable === true);
  const result = { ok, version: VERSION, checks };
  if (json) {
    io.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } else {
    io.stdout.write(`codex-kimi-bridge-node ${VERSION}\n`);
    io.stdout.write(`Node.js: ${checks.node.ok ? "OK" : "FAIL"} (${checks.node.version})\n`);
    io.stdout.write(
      `Port ${host}:${port}: ${portAvailable ? "available" : localService?.service === "codex-kimi-bridge-node" ? "bridge already running" : "occupied"}\n`,
    );
    io.stdout.write(`Auth for test commands: ${auth.source}\n`);
    io.stdout.write(`Upstream: ${checks.upstream.live_checked ? (checks.upstream.reachable ? "reachable" : "failed") : "not contacted"}\n`);
  }
  return ok ? 0 : 1;
}

async function translateRequestCommand(flags, positionals, io, json) {
  const source = await readJsonInput(flags, positionals, io);
  const handoffStateDir = stringFlag(
    flags["handoff-state-dir"],
    defaultHandoffStateDir(),
  );
  const translated = translateResponsesRequest(source, {
    defaultModel: stringFlag(flags.model, "k3"),
    handoffVerifier: loadHandoffVerifierIfPresent(handoffStateDir),
  });
  const result = {
    ok: true,
    request: translated.body,
    tool_kinds: Object.fromEntries(
      [...translated.context.toolMap.entries()].map(([name, value]) => [name, value.kind]),
    ),
  };
  io.stdout.write(`${JSON.stringify(json ? result : result.request, null, 2)}\n`);
  return 0;
}

async function hookCommand(flags, positionals, io) {
  const action = positionals[0];
  if (!action) {
    throw new BridgeError(
      "A hook action is required: user-prompt-submit or pre-tool-use.",
      { code: "missing_hook_action" },
    );
  }
  const stateDir = stringFlag(flags["state-dir"], defaultHandoffStateDir());
  let input;
  try {
    input = JSON.parse(await readAll(io.stdin));
  } catch (error) {
    throw new BridgeError("Hook input must be valid JSON.", {
      code: "invalid_json",
      cause: error,
    });
  }
  if (action === "user-prompt-submit") {
    captureUserPrompt(input, stateDir);
    return 0;
  }
  if (action === "pre-tool-use") {
    const output = rewritePreToolUse(input, stateDir);
    if (output !== null) {
      io.stdout.write(`${JSON.stringify(output)}\n`);
    }
    return 0;
  }
  throw new BridgeError(
    `Unknown hook action: ${action}. Use user-prompt-submit or pre-tool-use.`,
    { code: "unknown_hook_action" },
  );
}

async function requestCommand(flags, positionals, io, json) {
  const url = stringFlag(flags.url, "http://127.0.0.1:8787/v1/responses");
  const auth = await detectAuth({ readSecret: true });
  if (!auth.secret) {
    throw new BridgeError(
      "No API key is available. Set KIMI_CODE_API_KEY or store codex-kimi-code-api-key in macOS Keychain.",
      { status: 401, type: "authentication_error", code: "missing_api_key" },
    );
  }

  let body;
  if (flags.file || positionals[0] === "-") {
    body = await readJsonInput(flags, positionals, io);
  } else {
    const input =
      stringFlag(flags.input, null) ?? positionals.join(" ") ?? "Reply with OK only.";
    if (!input) {
      throw new BridgeError("request requires --input, positional text, or --file.");
    }
    body = {
      model: stringFlag(flags.model, "k3"),
      input,
      reasoning: { effort: stringFlag(flags.effort, "low") },
      stream: flags.stream === true,
    };
  }

  const response = await fetch(url, {
    method: "POST",
    headers: {
      authorization: `Bearer ${auth.secret}`,
      "content-type": "application/json",
      accept: body.stream ? "text/event-stream" : "application/json",
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    let error;
    try {
      error = await response.json();
    } catch {
      error = { error: { message: await response.text() } };
    }
    throw Object.assign(
      new BridgeError(error.error?.message ?? `HTTP ${response.status}`, {
        status: response.status,
        type: error.error?.type,
        code: error.error?.code,
        param: error.error?.param,
      }),
      { error: error.error },
    );
  }

  if (!body.stream) {
    const payload = await response.json();
    io.stdout.write(`${JSON.stringify(payload, null, json ? 2 : 2)}\n`);
    return 0;
  }

  const events = [];
  for await (const frame of parseSse(response.body)) {
    if (frame.data === "[DONE]") {
      break;
    }
    const event = JSON.parse(frame.data);
    if (json) {
      events.push(event);
    } else if (event.type === "response.output_text.delta") {
      io.stdout.write(event.delta);
    }
  }
  if (json) {
    io.stdout.write(`${JSON.stringify({ ok: true, events }, null, 2)}\n`);
  } else {
    io.stdout.write("\n");
  }
  return 0;
}

async function readJsonInput(flags, positionals, io) {
  let text;
  const file = stringFlag(flags.file, null);
  if (file) {
    text = await readFile(file, "utf8");
  } else if (positionals[0] && positionals[0] !== "-") {
    text = await readFile(positionals[0], "utf8");
  } else {
    text = await readAll(io.stdin);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new BridgeError("Input must be valid JSON.", {
      code: "invalid_json",
      cause: error,
    });
  }
}

async function detectAuth({ readSecret }) {
  if (process.env.KIMI_CODE_API_KEY) {
    return {
      available: true,
      source: "env:KIMI_CODE_API_KEY",
      secret: readSecret ? process.env.KIMI_CODE_API_KEY : null,
    };
  }

  if (process.platform === "darwin") {
    try {
      const args = ["find-generic-password", "-s", "codex-kimi-code-api-key"];
      if (readSecret) {
        args.push("-w");
      }
      const { stdout } = await execFileAsync(
        "/usr/bin/security",
        args,
        { encoding: "utf8", maxBuffer: 1024 * 1024 },
      );
      return {
        available: true,
        source: "macOS Keychain:codex-kimi-code-api-key",
        secret: readSecret ? stdout.trim() : null,
      };
    } catch {
      // Missing Keychain item is a normal doctor result.
    }
  }

  return { available: false, source: "missing", secret: null };
}

function parseArgs(argv) {
  const flags = {};
  const positionals = [];
  let command;

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!command && !token.startsWith("-")) {
      command = token;
      continue;
    }
    if (token === "--") {
      positionals.push(...argv.slice(index + 1));
      break;
    }
    if (token.startsWith("--")) {
      const equals = token.indexOf("=");
      if (equals !== -1) {
        flags[token.slice(2, equals)] = token.slice(equals + 1);
        continue;
      }
      const name = token.slice(2);
      if (BOOLEAN_FLAGS.has(name)) {
        flags[name] = true;
        if (!command && name === "help") {
          command = "help";
        } else if (!command && name === "version") {
          command = "version";
        }
        continue;
      }
      const next = argv[index + 1];
      if (next !== undefined && !next.startsWith("-")) {
        flags[name] = next;
        index += 1;
      } else {
        flags[name] = true;
      }
      continue;
    }
    if (token === "-h") {
      command ??= "help";
    } else if (token === "-v") {
      command ??= "version";
    } else {
      positionals.push(token);
    }
  }
  return { command, flags, positionals };
}

function helpText() {
  return `codex-kimi-bridge-node ${VERSION}

Zero-dependency local bridge: Codex Responses API -> Kimi Code Chat Completions.

Usage:
  codex-kimi-bridge-node serve [options]
  codex-kimi-bridge-node doctor [--json] [--live]
  codex-kimi-bridge-node hook <user-prompt-submit|pre-tool-use>
  codex-kimi-bridge-node translate-request [--file request.json | -] [--json]
  codex-kimi-bridge-node request [text] [--stream] [--json]

Commands:
  serve              Listen for Codex on 127.0.0.1:8787.
  doctor             Check Node, bind port, privacy defaults, auth source, and config.
  hook               Trusted local Codex task-handoff hook entry point.
  translate-request  Offline conversion of a Responses request to Kimi Chat JSON.
  request             Explicit raw test request to a running bridge.
  version             Print the bridge version.

Serve options:
  --host <host>                 Default: 127.0.0.1
  --port <port>                 Default: 8787
  --model <model>               Default: k3
  --upstream <url>              Default: ${DEFAULT_UPSTREAM}
  --timeout-ms <ms>             Default: 7200000
  --max-body-bytes <bytes>      Default: 134217728
  --quiet                       Suppress startup and sanitized error logs.
  --allow-non-loopback          Explicitly permit a non-loopback bind address.
  --allow-insecure-upstream     Permit plain HTTP only for loopback test servers.

Doctor options:
  --live                        Make one small Kimi request; consumes a tiny amount of quota.
  --json                        Stable machine-readable output.

Hook options:
  --state-dir <path>            Override the private local handoff state directory.

Translate options:
  --handoff-state-dir <path>    Verify local CKB1 handoff envelopes from this directory.

Request auth precedence:
  1. KIMI_CODE_API_KEY
  2. macOS Keychain service codex-kimi-code-api-key

Security defaults:
  - API keys and request bodies are never logged.
  - The server binds only to loopback unless explicitly overridden.
  - The upstream must use HTTPS unless it is an explicit loopback test server.
`;
}

function defaultIo() {
  return {
    stdin: process.stdin,
    stdout: process.stdout,
    stderr: process.stderr,
  };
}

function readAll(stream) {
  return new Promise((resolve, reject) => {
    let text = "";
    stream.setEncoding?.("utf8");
    stream.on("data", (chunk) => {
      text += chunk;
    });
    stream.once("end", () => resolve(text));
    stream.once("error", reject);
  });
}

function stringFlag(value, fallback) {
  return typeof value === "string" && value ? value : fallback;
}

function integerFlag(value, fallback) {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function redactUrl(value) {
  const url = new URL(value);
  return `${url.protocol}//${url.host}${url.pathname}`;
}
