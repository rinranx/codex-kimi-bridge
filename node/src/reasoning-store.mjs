const DEFAULT_TTL_MS = 2 * 60 * 60 * 1000;
const DEFAULT_MAX_ENTRIES = 512;
const DEFAULT_MAX_BYTES = 64 * 1024 * 1024;

export class ReasoningStore {
  constructor(options = {}) {
    this.ttlMs = positiveInteger(options.ttlMs, DEFAULT_TTL_MS);
    this.maxEntries = positiveInteger(options.maxEntries, DEFAULT_MAX_ENTRIES);
    this.maxBytes = positiveInteger(options.maxBytes, DEFAULT_MAX_BYTES);
    this.entries = new Map();
    this.totalBytes = 0;
  }

  set(callId, reasoningContent) {
    if (!isNonEmptyString(callId) || !isNonEmptyString(reasoningContent)) {
      return false;
    }

    const bytes = Buffer.byteLength(reasoningContent, "utf8");
    if (bytes > this.maxBytes) {
      return false;
    }

    this.delete(callId);
    this.entries.set(callId, {
      value: reasoningContent,
      bytes,
      expiresAt: Date.now() + this.ttlMs,
    });
    this.totalBytes += bytes;
    this.prune();
    return true;
  }

  get(callId) {
    const entry = this.entries.get(callId);
    if (!entry) {
      return null;
    }
    if (entry.expiresAt <= Date.now()) {
      this.delete(callId);
      return null;
    }

    this.entries.delete(callId);
    this.entries.set(callId, entry);
    return entry.value;
  }

  delete(callId) {
    const entry = this.entries.get(callId);
    if (!entry) {
      return false;
    }
    this.entries.delete(callId);
    this.totalBytes -= entry.bytes;
    return true;
  }

  clear() {
    this.entries.clear();
    this.totalBytes = 0;
  }

  prune() {
    const now = Date.now();
    for (const [callId, entry] of this.entries) {
      if (entry.expiresAt <= now) {
        this.delete(callId);
      }
    }
    while (this.entries.size > this.maxEntries || this.totalBytes > this.maxBytes) {
      const oldest = this.entries.keys().next().value;
      if (oldest === undefined) {
        break;
      }
      this.delete(oldest);
    }
  }
}

function positiveInteger(value, fallback) {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function isNonEmptyString(value) {
  return typeof value === "string" && value.length > 0;
}
