import { assertEquals } from "jsr:@std/assert@^1";
import { decideEntitlement } from "../functions/_shared/entitlement-logic.ts";

const now = Date.parse("2026-07-21T12:00:00.000Z");

Deno.test("trial does not begin at account creation", () => {
  const result = decideEntitlement({
    now,
    trialStartedAt: null,
    state: "trialing",
    currentPeriodEnd: null,
    gracePeriodEnd: null,
  });
  assertEquals(result.source, "not_started");
  assertEquals(result.allowed, false);
});

Deno.test("activated trial remains valid for seven days", () => {
  const result = decideEntitlement({
    now,
    trialStartedAt: "2026-07-20T12:00:00.000Z",
    state: "trialing",
    currentPeriodEnd: null,
    gracePeriodEnd: null,
  });
  assertEquals(result.source, "trial");
  assertEquals(result.allowed, true);
  assertEquals(result.accessUntil, "2026-07-27T12:00:00.000Z");
});

Deno.test("cancellation preserves paid-through access", () => {
  const result = decideEntitlement({
    now,
    trialStartedAt: "2026-06-01T00:00:00.000Z",
    state: "canceled",
    currentPeriodEnd: "2026-08-01T00:00:00.000Z",
    gracePeriodEnd: null,
  });
  assertEquals(result.source, "subscription");
  assertEquals(result.allowed, true);
});

Deno.test("past-due subscription uses explicit grace deadline", () => {
  const result = decideEntitlement({
    now,
    trialStartedAt: "2026-06-01T00:00:00.000Z",
    state: "past_due",
    currentPeriodEnd: "2026-07-20T00:00:00.000Z",
    gracePeriodEnd: "2026-07-23T00:00:00.000Z",
  });
  assertEquals(result.source, "grace");
  assertEquals(result.allowed, true);
});

Deno.test("expired access stays expired", () => {
  const result = decideEntitlement({
    now,
    trialStartedAt: "2026-06-01T00:00:00.000Z",
    state: "unpaid",
    currentPeriodEnd: "2026-07-01T00:00:00.000Z",
    gracePeriodEnd: "2026-07-04T00:00:00.000Z",
  });
  assertEquals(result.source, "expired");
  assertEquals(result.allowed, false);
});
