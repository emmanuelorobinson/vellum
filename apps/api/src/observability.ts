import {
  type CloudOptions,
  cloudTransport,
  createTelemetry,
  type OS,
  scrubEvent,
} from "@vellum/telemetry";
/** Operator opt-in is independent of local-client consent. No request context is passed to collectors. */
export function observability(
  env: Record<string, string | undefined>,
  options: {
    newId: () => string;
    clock: () => number;
    os?: OS;
    version?: string;
    fetch?: CloudOptions["fetch"];
  },
) {
  const enabled = env.VELLUM_TELEMETRY === "true";
  if (
    enabled &&
    (Boolean(env.VELLUM_POSTHOG_ORIGIN) !== Boolean(env.VELLUM_POSTHOG_TOKEN) ||
      !((env.VELLUM_POSTHOG_ORIGIN && env.VELLUM_POSTHOG_TOKEN) || env.VELLUM_SENTRY_DSN))
  ) {
    throw Error("Configure a telemetry collector before opting in");
  }
  const telemetry = createTelemetry({
    version: options.version ?? "0.0.0",
    os: options.os ?? "unknown",
    send: enabled
      ? cloudTransport({
          posthog:
            env.VELLUM_POSTHOG_ORIGIN && env.VELLUM_POSTHOG_TOKEN
              ? { origin: env.VELLUM_POSTHOG_ORIGIN, token: env.VELLUM_POSTHOG_TOKEN }
              : undefined,
          sentry: env.VELLUM_SENTRY_DSN ? { dsn: env.VELLUM_SENTRY_DSN } : undefined,
          newId: options.newId,
          fetch: options.fetch,
        })
      : undefined,
  });
  telemetry.setEnabled(enabled);
  let window = options.clock(),
    received = 0;
  return {
    event: telemetry.event,
    accept(value: unknown) {
      const safe = scrubEvent(value);
      if (!safe || !telemetry.enabled) return false;
      const now = options.clock();
      if (now - window >= 60_000) {
        window = now;
        received = 0;
      }
      if (received++ >= 120) return false;
      telemetry.capture(safe);
      return true;
    },
    async flush() {
      await telemetry.flush();
    },
    dispose() {
      telemetry.dispose();
    },
  };
}
