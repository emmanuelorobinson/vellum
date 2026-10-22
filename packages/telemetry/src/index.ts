/** Opt-in, allowlisted telemetry. No SDK hooks, disk queue, replay, profiles or automatic exception capture. */
export const PACKAGE_NAME = "@vellum/telemetry";
export const EVENTS = [
  "editor_opened",
  "export_completed",
  "renderer_failed",
  "cli_started",
  "daemon_started",
  "cloud_request",
  "cloud_failed",
] as const;
export type EventName = (typeof EVENTS)[number];
export type OS = "macos" | "windows" | "linux" | "unknown";
export interface SafeEvent {
  event: EventName;
  version: string;
  os: OS;
  count: 1;
}
export type Send = (event: SafeEvent, signal: AbortSignal) => Promise<void>;
/** Reconstruct, never redact in place. Arbitrary text, errors, ids and nested properties have no wire field. */
export function scrubEvent(value: unknown): SafeEvent | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  if (
    !EVENTS.includes(v.event as EventName) ||
    typeof v.version !== "string" ||
    !/^\d{1,5}\.\d{1,5}\.\d{1,5}$/.test(v.version) ||
    !["macos", "windows", "linux", "unknown"].includes(v.os as string)
  )
    return null;
  return { event: v.event as EventName, version: v.version, os: v.os as OS, count: 1 };
}
export function createTelemetry(options: { version: string; os: OS; send?: Send }) {
  let enabled = false,
    disposed = false;
  const pending = new Map<AbortController, Promise<void>>();
  const stop = () => {
    for (const controller of pending.keys()) controller.abort();
  };
  const capture = (input: unknown) => {
    if (!enabled || disposed || !options.send || pending.size >= 4) return;
    const event = scrubEvent(input);
    if (!event) return;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 2000);
    let sent: Promise<void>;
    try {
      sent = options.send(event, controller.signal);
    } catch {
      sent = Promise.resolve();
    }
    const done = sent
      .catch(() => {})
      .finally(() => {
        clearTimeout(timer);
        pending.delete(controller);
      });
    pending.set(controller, done);
  };
  return {
    get enabled() {
      return enabled && !disposed && !!options.send;
    },
    setEnabled(on: boolean) {
      enabled = on === true && !disposed;
      if (!enabled) stop();
    },
    event(name: string, _metrics?: Readonly<Record<string, number>>) {
      capture({ event: name, version: options.version, os: options.os });
    },
    capture,
    async flush() {
      await Promise.all([...pending.values()]);
    },
    dispose() {
      disposed = true;
      enabled = false;
      stop();
    },
  };
}

function origin(raw: string): string {
  let u: URL;
  try {
    u = new URL(raw);
  } catch {
    throw Error("Invalid telemetry origin");
  }
  if (
    u.protocol !== "https:" ||
    u.username ||
    u.password ||
    u.search ||
    u.hash ||
    u.pathname !== "/"
  )
    throw Error("Telemetry requires an HTTPS origin");
  return u.origin;
}
export interface CloudOptions {
  posthog?: { origin: string; token: string };
  sentry?: { dsn: string };
  newId: () => string;
  fetch?: (url: string, init?: RequestInit) => Promise<Response>;
}
/** Server/native only. Tokens come from server configuration or SecretStore, never from a webview. */
export function cloudTransport(options: CloudOptions): Send {
  const ph = options.posthog ? `${origin(options.posthog.origin)}/i/v0/e/` : undefined;
  let sentry: { url: string; dsn: string } | undefined;
  if (options.sentry) {
    let dsn: URL;
    try {
      dsn = new URL(options.sentry.dsn);
    } catch {
      throw Error("Invalid Sentry DSN");
    }
    if (
      dsn.protocol !== "https:" ||
      !/^[A-Za-z0-9]+$/.test(dsn.username) ||
      dsn.password ||
      !/^\/\d+$/.test(dsn.pathname) ||
      dsn.search ||
      dsn.hash
    )
      throw Error("Invalid Sentry DSN");
    sentry = { url: `${dsn.origin}/api${dsn.pathname}/envelope/`, dsn: dsn.href };
  }
  const fetcher = options.fetch ?? fetch;
  return async (input, signal) => {
    const event = scrubEvent(input);
    if (!event || signal.aborted) return;
    const id = options.newId().replaceAll("-", "");
    if (!/^[a-f0-9]{32}$/.test(id)) return;
    const init = {
      method: "POST",
      signal,
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
    } as const;
    const requests: Promise<unknown>[] = [];
    if (ph)
      requests.push(
        fetcher(ph, {
          ...init,
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            api_key: options.posthog?.token,
            event: event.event,
            distinct_id: id,
            properties: {
              version: event.version,
              os: event.os,
              count: 1,
              $process_person_profile: false,
              $geoip_disable: true,
            },
          }),
        }),
      );
    if (sentry && ["renderer_failed", "cloud_failed"].includes(event.event)) {
      const payload = JSON.stringify({
        event_id: id,
        level: "error",
        message: event.event,
        tags: { version: event.version, os: event.os },
      });
      requests.push(
        fetcher(sentry.url, {
          ...init,
          headers: { "Content-Type": "application/x-sentry-envelope" },
          body: `${JSON.stringify({ event_id: id, dsn: sentry.dsn })}\n${JSON.stringify({ type: "event", length: new TextEncoder().encode(payload).length })}\n${payload}\n`,
        }),
      );
    }
    await Promise.allSettled(requests);
  };
}
/** Preference binding is structural so the wrapper does not import the editor. No event is replayed on opt-in. */
export function bindConsent(
  preferences: {
    get(key: "telemetry"): boolean;
    subscribe(listener: () => void): { dispose(): void };
  },
  telemetry: ReturnType<typeof createTelemetry>,
) {
  const sync = () => telemetry.setEnabled(preferences.get("telemetry"));
  sync();
  const subscription = preferences.subscribe(sync);
  return {
    dispose() {
      subscription.dispose();
      telemetry.dispose();
    },
  };
}
/** Local clients use a configured relay, with no account, cookies or collector tokens in the editor. */
export function relayTransport(endpoint: string, fetcher: CloudOptions["fetch"] = fetch): Send {
  const url = `${origin(endpoint)}/v1/telemetry`;
  return async (input, signal) => {
    const event = scrubEvent(input);
    if (!event || signal.aborted) return;
    await fetcher(url, {
      method: "POST",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(event),
    });
  };
}
/** One reporter per native editor window. The webview can send only its consent, never telemetry data. */
export function editorReporter(options: { endpoint?: string; version: string; os: OS }) {
  const telemetry = createTelemetry({
    ...options,
    send: options.endpoint ? relayTransport(options.endpoint) : undefined,
  });
  let reported = false;
  return {
    consent(enabled: boolean) {
      telemetry.setEnabled(enabled);
      if (telemetry.enabled && !reported) {
        reported = true;
        telemetry.event("editor_opened");
      }
    },
    dispose: () => telemetry.dispose(),
  };
}
