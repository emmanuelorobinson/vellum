// TEST DATA: collectors are injected; credentials/content never enter captured bodies.
import { expect, it, vi } from "vitest";
import { observability } from "./observability";

it("cloud stays silent unless explicitly enabled; scrubs client payloads", async () => {
  const fetcher = vi.fn(async (_u: string, _i?: RequestInit) => new Response("", { status: 202 }));
  const options = {
    newId: () => "0123456789abcdef0123456789abcdef",
    fetch: fetcher,
    clock: () => 0,
  };
  const env = {
    VELLUM_POSTHOG_ORIGIN: "https://ph.example",
    VELLUM_POSTHOG_TOKEN: "TEST DATA key",
  };
  const off = observability(env, options);
  expect(off.accept({ event: "editor_opened", os: "linux", version: "0.0.0" })).toBe(false);
  await off.flush();
  expect(fetcher).not.toHaveBeenCalled();
  const on = observability({ ...env, VELLUM_TELEMETRY: "true" }, options);
  expect(
    on.accept({
      event: "editor_opened",
      os: "linux",
      version: "0.0.0",
      prompt: "TEST DATA private",
      path: "/private/repo",
      key: "TEST DATA secret",
    }),
  ).toBe(true);
  expect(on.accept({ event: "TEST DATA secret", os: "linux", version: "0.0.0" })).toBe(false);
  await on.flush();
  expect(fetcher).toHaveBeenCalledOnce();
  expect(String(fetcher.mock.calls[0]?.[1]?.body)).not.toMatch(/private|secret|prompt|path/);
  on.dispose();
});
it("cloud limits ingress without storing IPs or identities", () => {
  const on = observability(
    {
      VELLUM_TELEMETRY: "true",
      VELLUM_POSTHOG_ORIGIN: "https://ph.example",
      VELLUM_POSTHOG_TOKEN: "TEST DATA",
    },
    {
      newId: () => "0123456789abcdef0123456789abcdef",
      clock: () => 0,
      fetch: async () => new Response(),
    },
  );
  for (let i = 0; i < 120; i++)
    expect(on.accept({ event: "cli_started", version: "0.0.0", os: "linux" })).toBe(true);
  expect(on.accept({ event: "cli_started", version: "0.0.0", os: "linux" })).toBe(false);
  on.dispose();
});
it("explicit opt-in with no collector fails clearly; disabled malformed configuration stays silent", () => {
  expect(() =>
    observability({ VELLUM_TELEMETRY: "true" }, { newId: () => "", clock: () => 0 }),
  ).toThrow("collector");
  expect(() =>
    observability(
      { VELLUM_POSTHOG_ORIGIN: "TEST DATA invalid" },
      { newId: () => "", clock: () => 0 },
    ),
  ).not.toThrow();
});
