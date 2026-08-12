export class BridgeError extends Error {
  constructor(message, options = {}) {
    super(message, options.cause ? { cause: options.cause } : undefined);
    this.name = "BridgeError";
    this.status = options.status ?? 400;
    this.code = options.code ?? "invalid_request_error";
    this.type = options.type ?? "invalid_request_error";
    this.param = options.param ?? null;
  }
}

export function toErrorEnvelope(error) {
  if (error?.error && typeof error.error === "object") {
    return { error: sanitizeProviderError(error.error) };
  }

  const known = error instanceof BridgeError;
  return {
    error: {
      message: known ? error.message : "The bridge could not complete the request.",
      type: known ? error.type : "bridge_error",
      code: known ? error.code : "bridge_error",
      param: known ? error.param : null,
    },
  };
}

function sanitizeProviderError(error) {
  return {
    message:
      typeof error.message === "string"
        ? error.message
        : "The upstream provider rejected the request.",
    type:
      typeof error.type === "string" ? error.type : "upstream_provider_error",
    code:
      typeof error.code === "string" || typeof error.code === "number"
        ? error.code
        : "upstream_provider_error",
    param:
      typeof error.param === "string" || error.param === null
        ? error.param
        : null,
  };
}
