import http from "node:http";
import net from "node:net";
import { URL } from "node:url";

import { BridgeError, toErrorEnvelope } from "./errors.mjs";
import {
  translateChatCompletion,
  translateChatCompletionStream,
  translateResponsesRequest,
} from "./protocol.mjs";
import { encodeDone, encodeSse } from "./sse.mjs";
import { ReasoningStore } from "./reasoning-store.mjs";
import { VERSION } from "./version.mjs";

export const DEFAULT_UPSTREAM =
  "https://api.kimi.com/coding/v1/chat/completions";

export function createBridgeServer(options = {}) {
  return http.createServer(createBridgeHandler(options));
}

export function createBridgeHandler(options = {}) {
  const config = normalizeServerOptions(options);
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const reasoningStore = options.reasoningStore ?? new ReasoningStore();

  if (typeof fetchImpl !== "function") {
    throw new BridgeError("This Node.js runtime does not provide fetch().", {
      status: 500,
      code: "missing_fetch",
    });
  }

  return async (request, response) => {
    setSecurityHeaders(response);
    const requestUrl = new URL(request.url ?? "/", "http://bridge.local");

    if (request.method === "GET" && ["/", "/health", "/v1/health"].includes(requestUrl.pathname)) {
      sendJson(response, 200, {
        status: "ok",
        service: "codex-kimi-bridge",
        version: VERSION,
        model: config.defaultModel,
        upstream: redactUpstream(config.upstreamUrl),
        logging: "request bodies and credentials are not logged",
      });
      return;
    }

    if (request.method === "GET" && requestUrl.pathname === "/v1/models") {
      sendJson(response, 200, {
        object: "list",
        data: [config.defaultModel].map(
          (id) => ({ id, object: "model", created: 0, owned_by: "kimi-code" }),
        ),
      });
      return;
    }

    if (
      request.method !== "POST" ||
      !["/v1/responses", "/responses"].includes(requestUrl.pathname)
    ) {
      sendJson(response, 404, {
        error: {
          message: "Not found. Use POST /v1/responses or GET /health.",
          type: "invalid_request_error",
          code: "not_found",
          param: null,
        },
      });
      return;
    }

    const authorization = request.headers.authorization;
    if (!authorization?.startsWith("Bearer ") || authorization.length <= 7) {
      sendJson(response, 401, {
        error: {
          message: "A Bearer token is required. Codex should supply it through the provider auth command.",
          type: "authentication_error",
          code: "missing_api_key",
          param: null,
        },
      });
      return;
    }

    const controller = new AbortController();
    const abortForDisconnect = () => {
      if (!response.writableEnded) {
        controller.abort(new Error("Downstream client disconnected."));
      }
    };
    request.once("aborted", abortForDisconnect);
    response.once("close", abortForDisconnect);
    const timeout = setTimeout(
      () => controller.abort(new Error("Upstream request timed out.")),
      config.timeoutMs,
    );
    timeout.unref?.();

    try {
      const rawBody = await readRequestBody(request, config.maxBodyBytes);
      let parsedBody;
      try {
        parsedBody = JSON.parse(rawBody);
      } catch (error) {
        throw new BridgeError("Request body must be valid JSON.", {
          param: null,
          code: "invalid_json",
          cause: error,
        });
      }

      const translated = translateResponsesRequest(parsedBody, {
        defaultModel: config.defaultModel,
        reasoningStore,
      });
      const upstreamResponse = await fetchImpl(config.upstreamUrl, {
        method: "POST",
        headers: {
          authorization,
          "content-type": "application/json",
          accept: translated.body.stream ? "text/event-stream" : "application/json",
          "user-agent": `codex-kimi-bridge/${VERSION}`,
        },
        body: JSON.stringify(translated.body),
        signal: controller.signal,
        redirect: "error",
      });

      if (!upstreamResponse.ok) {
        const upstreamError = await readUpstreamError(upstreamResponse);
        sendJson(response, upstreamResponse.status, upstreamError);
        return;
      }

      if (translated.body.stream) {
        response.writeHead(200, {
          "content-type": "text/event-stream; charset=utf-8",
          "cache-control": "no-cache, no-transform",
          connection: "keep-alive",
          "x-accel-buffering": "no",
        });

        try {
          for await (const event of translateChatCompletionStream(
            upstreamResponse.body,
            translated.context,
          )) {
            if (!response.write(encodeSse(event))) {
              await waitForDrain(response);
            }
          }
          response.end(encodeDone());
        } catch (error) {
          if (!response.writableEnded) {
            const envelope = toErrorEnvelope(error);
            response.end(
              encodeSse({
                type: "error",
                sequence_number: Number.MAX_SAFE_INTEGER,
                ...envelope.error,
              }),
            );
          }
        }
        return;
      }

      const chat = await upstreamResponse.json();
      const translatedResponse = translateChatCompletion(chat, translated.context);
      sendJson(response, 200, translatedResponse);
    } catch (error) {
      if (controller.signal.aborted && !response.headersSent) {
        const message = controller.signal.reason?.message ?? "The request was aborted.";
        sendJson(response, 504, {
          error: {
            message,
            type: "timeout_error",
            code: "request_aborted",
            param: null,
          },
        });
      } else if (!response.headersSent) {
        const status = error instanceof BridgeError ? error.status : 500;
        sendJson(response, status, toErrorEnvelope(error));
      } else if (!response.writableEnded) {
        response.end();
      }
      config.logger?.error?.(
        JSON.stringify({
          event: "request_failed",
          status: error instanceof BridgeError ? error.status : 500,
          code: error instanceof BridgeError ? error.code : "bridge_error",
        }),
      );
    } finally {
      clearTimeout(timeout);
      request.off("aborted", abortForDisconnect);
      response.off("close", abortForDisconnect);
    }
  };
}

export async function listen(server, options = {}) {
  const host = options.host ?? "127.0.0.1";
  const port = options.port ?? 8787;
  await new Promise((resolve, reject) => {
    const onError = (error) => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = () => {
      server.off("error", onError);
      resolve();
    };
    server.once("error", onError);
    server.once("listening", onListening);
    server.listen(port, host);
  });
  return server.address();
}

export async function close(server) {
  if (!server.listening) {
    return;
  }
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

export async function isPortAvailable(host, port) {
  return await new Promise((resolve) => {
    const probe = net.createServer();
    probe.unref();
    probe.once("error", () => resolve(false));
    probe.listen(port, host, () => {
      probe.close(() => resolve(true));
    });
  });
}

function normalizeServerOptions(options) {
  const upstream = new URL(options.upstreamUrl ?? DEFAULT_UPSTREAM);
  const allowInsecure = options.allowInsecureUpstream === true;
  if (upstream.protocol !== "https:" && !(allowInsecure && isLoopback(upstream.hostname))) {
    throw new BridgeError(
      "The upstream URL must use HTTPS. Plain HTTP is allowed only for an explicit loopback test server.",
      { code: "unsafe_upstream_url" },
    );
  }
  if (!isLoopback(options.host ?? "127.0.0.1") && options.allowNonLoopback !== true) {
    throw new BridgeError(
      "Refusing to bind outside loopback. Pass allowNonLoopback only if you understand that API tokens will cross that interface.",
      { code: "unsafe_bind_address" },
    );
  }
  return {
    upstreamUrl: upstream.toString(),
    defaultModel: options.defaultModel ?? "k3",
    timeoutMs: positiveInteger(options.timeoutMs, 7_200_000),
    maxBodyBytes: positiveInteger(options.maxBodyBytes, 128 * 1024 * 1024),
    logger: options.logger ?? console,
  };
}

async function readRequestBody(request, limit) {
  const chunks = [];
  let total = 0;
  for await (const chunk of request) {
    total += chunk.length;
    if (total > limit) {
      throw new BridgeError(`Request body exceeds the ${limit}-byte limit.`, {
        status: 413,
        code: "request_too_large",
      });
    }
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

async function readUpstreamError(response) {
  const text = (await response.text()).slice(0, 1_048_576);
  try {
    return toErrorEnvelope(JSON.parse(text));
  } catch {
    return {
      error: {
        message: text || `The upstream provider returned HTTP ${response.status}.`,
        type: "upstream_provider_error",
        code: `http_${response.status}`,
        param: null,
      },
    };
  }
}

function sendJson(response, status, body) {
  if (response.writableEnded) {
    return;
  }
  const payload = JSON.stringify(body);
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(payload),
  });
  response.end(payload);
}

function setSecurityHeaders(response) {
  response.setHeader("x-content-type-options", "nosniff");
  response.setHeader("referrer-policy", "no-referrer");
  response.setHeader("permissions-policy", "geolocation=(), microphone=(), camera=()");
}

function waitForDrain(stream) {
  return new Promise((resolve, reject) => {
    stream.once("drain", resolve);
    stream.once("error", reject);
  });
}

function redactUpstream(url) {
  const parsed = new URL(url);
  return `${parsed.protocol}//${parsed.host}${parsed.pathname}`;
}

function isLoopback(host) {
  return ["127.0.0.1", "::1", "localhost"].includes(host);
}

function positiveInteger(value, fallback) {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}
