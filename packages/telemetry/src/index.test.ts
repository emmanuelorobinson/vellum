import { describe, expect, it, vi } from "vitest";
import { cloudTransport, createTelemetry, PACKAGE_NAME, scrubEvent } from "./index";

it("is off by default, drops unknown text and only sends the disclosed fields", async () => {
  const send = vi.fn(async (_e: unknown, _s: AbortSignal) => {});
  const telemetry = createTelemetry({ version: "0.0.0", os: "linux", send });
  telemetry.event("editor_opened", { count: 1 });
  expect(send).not.toHaveBeenCalled();
  telemetry.setEnabled(true);
  telemetry.event("TEST DATA secret prompt");
  telemetry.event("editor_opened", { count: 1, secret: 42 });
  await telemetry.flush();
  expect(send).toHaveBeenCalledOnce();
  expect(send.mock.calls[0]?.[0]).toEqual({
    event: "editor_opened",
    version: "0.0.0",
    os: "linux",
    count: 1,
  });
  expect(
    scrubEvent({
      event: "renderer_failed",
      message: "TEST DATA key",
      stack: "/private/repo",
      prompt: "TEST DATA",
      version: "0.0.0",
      os: "linux",
      count: 1,
    }),
  ).toEqual({ event: "renderer_failed", version: "0.0.0", os: "linux", count: 1 });
  expect(
    scrubEvent({ event: "editor_opened", version: "TEST DATA secret", os: "linux", count: 1 }),
  ).toBeNull();
});
it("revocation aborts pending sends, no backlog; dispose cannot be re-enabled", async () => {
  let signal: AbortSignal | undefined;
  const send = vi.fn((_e: unknown, s: AbortSignal) => {
    signal = s;
    return new Promise<void>((resolve) => s.addEventListener("abort", () => resolve()));
  });
  const t = createTelemetry({ version: "0.0.0", os: "macos", send });
  t.setEnabled(true);
  t.event("editor_opened");
  t.setEnabled(false);
  expect(signal?.aborted).toBe(true);
  await t.flush();
  t.event("editor_opened");
  expect(send).toHaveBeenCalledOnce();
  t.dispose();
  t.setEnabled(true);
  expect(t.enabled).toBe(false);
});
it("bounds concurrent reports and isolates transport failure from the product", async () => {
  const send = vi.fn(async () => {
    throw Error("TEST DATA secret url");
  });
  const t = createTelemetry({ version: "0.0.0", os: "windows", send });
  t.setEnabled(true);
  for (let i = 0; i < 100; i++) t.event("editor_opened");
  await expect(t.flush()).resolves.toBeUndefined();
  expect(send).toHaveBeenCalledTimes(4);
});
it("PostHog and Sentry receive minimal envelopes without user/request/stack/context", async () => {
  const fetcher = vi.fn(
    async (_url: string, _init?: RequestInit) => new Response("", { status: 202 }),
  );
  const send = cloudTransport({
    posthog: { origin: "https://ph.example", token: "TEST DATA project token" },
    sentry: { dsn: "https://public@sentry.example/12" },
    newId: () => "1234567890abcdef1234567890abcdef",
    fetch: fetcher,
  });
  const t = createTelemetry({ version: "0.0.0", os: "linux", send });
  t.setEnabled(true);
  t.event("renderer_failed");
  await t.flush();
  expect(fetcher).toHaveBeenCalledTimes(2);
  const bodies = fetcher.mock.calls.map((c) => String(c[1]?.body)).join("\n");
  expect(bodies).toContain('"$process_person_profile":false');
  expect(bodies).toContain('"type":"event"');
  expect(bodies).not.toMatch(/stacktrace|breadcrumbs|request|user|cookie|prompt/);
  for (const call of fetcher.mock.calls)
    expect(call[1]).toMatchObject({
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
    });
});
it("editor binding starts off, syncs loaded consent and aborts when Settings changes", async () => {
  const { bindConsent } = await import("./index");
  let on = false,
    listener = () => {};
  const send = vi.fn(async () => {});
  const t = createTelemetry({ version: "0.0.0", os: "macos", send });
  const binding = bindConsent(
    {
      get: () => on,
      subscribe(l) {
        listener = l;
        return {
          dispose() {
            listener = () => {};
          },
        };
      },
    },
    t,
  );
  t.event("editor_opened");
  expect(send).not.toHaveBeenCalled();
  on = true;
  listener();
  t.event("editor_opened");
  await t.flush();
  expect(send).toHaveBeenCalledOnce();
  on = false;
  listener();
  expect(t.enabled).toBe(false);
  binding.dispose();
});
it("native editor only reports after consent and never replays disabled events", async () => {
  const { editorReporter, relayTransport } = await import("./index");
  const fetcher = vi.fn(
    async (_url: string, _init?: RequestInit) => new Response("", { status: 202 }),
  );
  const t = createTelemetry({
    version: "0.0.0",
    os: "linux",
    send: relayTransport("https://relay.example", fetcher),
  });
  t.event("renderer_failed");
  t.setEnabled(true);
  await t.flush();
  expect(fetcher).not.toHaveBeenCalled();
  t.event("renderer_failed");
  await t.flush();
  expect(fetcher).toHaveBeenCalledOnce();
  expect(fetcher.mock.calls[0]?.[1]?.body).toBe(
    '{"event":"renderer_failed","version":"0.0.0","os":"linux","count":1}',
  );
  const noEndpoint = editorReporter({ version: "0.0.0", os: "linux" });
  noEndpoint.consent(true);
  noEndpoint.dispose();
  t.dispose();
});
it("invalid collector configuration errors never echo the configured value", () => {
  for (const options of [
    { posthog: { origin: "TEST DATA CANARY", token: "TEST DATA" } },
    { sentry: { dsn: "TEST DATA CANARY" } },
  ]) {
    try {
      cloudTransport({ ...options, newId: () => "" });
      throw Error("expected validation");
    } catch (error) {
      expect(String(error)).not.toContain("CANARY");
      expect(error).not.toHaveProperty("input");
    }
  }
});

describe("@vellum/telemetry", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/telemetry");
  });
});
