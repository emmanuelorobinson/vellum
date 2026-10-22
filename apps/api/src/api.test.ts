import { observability } from "./observability";
// TEST DATA. In-process Hono requests with injected cloud boundaries.

import type { Auth } from "@vellum/auth";
import { CLOUD_FEATURES } from "@vellum/billing";
import { MemoryJobQueue } from "@vellum/jobs";
import { expect, it, vi } from "vitest";
import { createApi } from "./index";

const setup = (telemetry?: ReturnType<typeof observability>) => {
  const auth: Auth = {
    handler: vi.fn(async () => new Response("auth")),
    getSession: async (h) =>
      h.get("authorization") === "Bearer test-session"
        ? { user: { id: "person", email: "person@example.test", name: "Person" }, id: "session" }
        : null,
    requireUser: vi.fn(),
    verifyDeviceCode: vi.fn(),
    sessions: async () => [],
    revoke: vi.fn(),
  };
  const billing = {
    createCheckout: vi.fn(async () => "https://checkout.example.test"),
    getEntitlements: vi.fn(async () => [] as (typeof CLOUD_FEATURES)[number][]),
    acceptWebhook: vi.fn(async (_body: string, _headers: Record<string, string>) => {}),
  };
  const jobs = new MemoryJobQueue();
  const authorizeInput = vi.fn(
    async (user: string, input: string) => user === "person" && input === "bundle",
  );
  return {
    billing,
    jobs,
    authorizeInput,
    app: createApi({
      telemetry,
      auth,
      billing,
      jobs,
      authorizeInput,
      newId: () => "job-1",
      accountOrigin: "https://account.example.test",
    }),
  };
};
it("exposes signed-out state, returns no tokens, and rejects untrusted cookie mutations", async () => {
  const { app, billing } = setup();
  expect((await app.request("/v1/me")).status).toBe(401);
  expect((await app.request("/v1/entitlements")).status).toBe(200);
  const signed = await app.request("/v1/me", { headers: { authorization: "Bearer test-session" } });
  expect(await signed.json()).toEqual({
    user: { id: "person", email: "person@example.test", name: "Person" },
  });
  const result = await app.request("/v1/billing/checkout", {
    method: "POST",
    headers: { origin: "https://attacker.test", cookie: "session=value" },
  });
  expect(result.status).toBe(403);
  expect(billing.createCheckout).not.toHaveBeenCalled();
});
it("gates only cloud job submission and enforces input/result ownership", async () => {
  const { app, billing, jobs, authorizeInput } = setup();
  const post = (inputId: string) =>
    app.request("/v1/jobs", {
      method: "POST",
      headers: { authorization: "Bearer test-session", "content-type": "application/json" },
      body: JSON.stringify({ inputId, kind: "thumbnail" }),
    });
  expect((await post("bundle")).status).toBe(403);
  billing.getEntitlements.mockResolvedValue([...CLOUD_FEATURES]);
  expect((await post("other")).status).toBe(404);
  expect((await post("bundle")).status).toBe(202);
  expect(authorizeInput).toHaveBeenCalledWith("person", "bundle");
  expect(await jobs.get("job-1", "other")).toBeNull();
  expect(
    (await app.request("/v1/jobs/job-1", { headers: { authorization: "Bearer test-session" } }))
      .status,
  ).toBe(200);
});
it("hands raw webhook bytes to billing and masks unexpected errors", async () => {
  const { app, billing } = setup();
  const body = ' {"untouched":true} ';
  expect((await app.request("/v1/billing/webhook", { method: "POST", body })).status).toBe(202);
  expect(billing.acceptWebhook.mock.calls[0]?.[0]).toBe(body);
  billing.getEntitlements.mockRejectedValue(new Error("sensitive upstream detail"));
  const result = await app.request("/v1/entitlements", {
    headers: { authorization: "Bearer test-session" },
  });
  expect(result.status).toBe(503);
  expect(await result.text()).not.toContain("sensitive");
});
it("never falls back to cookies when a bearer header is supplied", async () => {
  const auth: Auth = {
    handler: vi.fn(),
    getSession: async (headers) =>
      headers.has("cookie")
        ? { user: { id: "person", email: "person@example.test", name: "Person" }, id: "session" }
        : null,
    requireUser: vi.fn(),
    verifyDeviceCode: vi.fn(),
    sessions: vi.fn(),
    revoke: vi.fn(),
  };
  const app = createApi({
    auth,
    billing: { createCheckout: vi.fn(), getEntitlements: async () => [], acceptWebhook: vi.fn() },
    jobs: new MemoryJobQueue(),
    newId: () => "job",
    authorizeInput: async () => false,
    accountOrigin: "https://account.example.test",
  });
  const result = await app.request("/v1/me", {
    headers: { authorization: "Bearer invalid", cookie: "session=valid" },
  });
  expect(result.status).toBe(401);
});

it("telemetry relay needs no account and never forwards request context", async () => {
  const fetcher = vi.fn(
    async (_url: string, _init?: RequestInit) => new Response("", { status: 202 }),
  );
  const t = observability(
    {
      VELLUM_TELEMETRY: "true",
      VELLUM_POSTHOG_ORIGIN: "https://ph.example",
      VELLUM_POSTHOG_TOKEN: "TEST DATA project",
    },
    { newId: () => "0123456789abcdef0123456789abcdef", clock: () => 0, fetch: fetcher },
  );
  const { app } = setup(t);
  const response = await app.request("/v1/telemetry", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: "TEST DATA canary",
      authorization: "TEST DATA canary",
    },
    body: JSON.stringify({
      event: "editor_opened",
      version: "0.0.0",
      os: "macos",
      prompt: "TEST DATA canary",
      url: "/repo/private",
    }),
  });
  expect(response.status).toBe(202);
  await t.flush();
  expect(fetcher).toHaveBeenCalledOnce();
  expect(JSON.stringify(fetcher.mock.calls)).not.toMatch(/canary|private|cookie|authorization/);
  expect(
    (await app.request("/v1/telemetry", { method: "POST", body: "x".repeat(1025) })).status,
  ).toBe(413);
  t.dispose();
});
