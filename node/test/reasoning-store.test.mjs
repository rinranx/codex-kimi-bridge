import assert from "node:assert/strict";
import test from "node:test";

import { ReasoningStore } from "../src/reasoning-store.mjs";

test("reasoning cache stays bounded and never accepts invalid entries", () => {
  const store = new ReasoningStore({ maxEntries: 2, maxBytes: 32 });
  assert.equal(store.set(null, "secret"), false);
  assert.equal(store.set("call_oversize", "x".repeat(33)), false);

  assert.equal(store.set("call_1", "one"), true);
  assert.equal(store.set("call_2", "two"), true);
  assert.equal(store.set("call_3", "three"), true);
  assert.equal(store.get("call_1"), null);
  assert.equal(store.get("call_2"), "two");
  assert.equal(store.get("call_3"), "three");
});
