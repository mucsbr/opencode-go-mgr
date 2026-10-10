import assert from "node:assert/strict";
import test from "node:test";
import { AccountRefreshTimeoutError, withAccountRefreshDeadline } from "./account-refresh-deadline.ts";

test("a stalled request is aborted and rejected even when it ignores its signal", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal!: AbortSignal;
  const result = withAccountRefreshDeadline(async current => {
    signal = current;
    return new Promise<never>(() => {});
  }, undefined, 50);
  await Promise.resolve();
  const rejected = assert.rejects(result, error => error instanceof AccountRefreshTimeoutError && error.code === "timeout");
  t.mock.timers.tick(50);
  await rejected;
  assert.equal(signal.aborted, true);
});

test("completion removes the deadline and preserves the response", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal!: AbortSignal;
  assert.equal(await withAccountRefreshDeadline(async current => { signal = current; return 42; }, undefined, 50), 42);
  t.mock.timers.tick(100);
  assert.equal(signal.aborted, false);
});

test("caller cancellation settles a stalled request without turning it into a timeout", async () => {
  const controller = new AbortController();
  let signal!: AbortSignal;
  const result = withAccountRefreshDeadline(async current => {
    signal = current;
    return new Promise<never>(() => {});
  }, controller.signal);
  await Promise.resolve();
  const reason = new Error("synthetic-cancel");
  const rejected = assert.rejects(result, error => error === reason);
  controller.abort(reason);
  await rejected;
  assert.equal(signal.reason, reason);
});

test("an already cancelled request never starts the transport", async () => {
  const controller = new AbortController();
  controller.abort();
  let calls = 0;
  await assert.rejects(withAccountRefreshDeadline(async () => { calls++; return 42; }, controller.signal), { name: "AbortError" });
  assert.equal(calls, 0);
});
