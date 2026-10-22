import type { Auth } from "@vellum/auth";
import type { Billing, Entitlement } from "@vellum/billing";
import { InvalidWebhook } from "@vellum/billing/polar";
import { identifier, type JobQueue, validateJob } from "@vellum/jobs";
import { decodeBundle, type ShareService, ShareUnavailable, zipBundle } from "@vellum/storage";
import { Hono } from "hono";
import { bodyLimit } from "hono/body-limit";
import type { observability } from "./observability";
export const PACKAGE_NAME = "@vellum/api";
function credentialHeaders(raw: Headers): Headers {
  const headers = new Headers(raw);
  if (headers.has("authorization")) headers.delete("cookie");
  return headers;
}
export function createApi(ports: {
  telemetry?: ReturnType<typeof observability>;
  shares?: ShareService;
  auth: Auth;
  billing: Billing;
  jobs: JobQueue;
  newId: () => string;
  accountOrigin: string;
  authorizeInput: (userId: string, inputId: string) => Promise<boolean>;
}) {
  const app = new Hono();
  app.use(
    "*",
    bodyLimit({
      maxSize: 1024 * 1024,
      onError: (c) => c.json({ error: "Request too large" }, 413),
    }),
  );
  app.use("*", async (c, next) => {
    c.header("Cache-Control", "no-store");
    c.header("X-Content-Type-Options", "nosniff");
    if (
      !["GET", "HEAD", "OPTIONS"].includes(c.req.method) &&
      !c.req.path.startsWith("/api/auth/") &&
      c.req.path !== "/v1/billing/webhook" &&
      c.req.path !== "/v1/telemetry" &&
      !c.req.header("authorization")
    ) {
      if (c.req.header("origin") !== ports.accountOrigin)
        return c.json({ error: "Untrusted origin" }, 403);
    }
    await next();
  });
  app.onError(() => {
    ports.telemetry?.event("cloud_failed");
    return new Response(JSON.stringify({ error: "Cloud service unavailable" }), {
      status: 503,
      headers: { "content-type": "application/json", "cache-control": "no-store" },
    });
  });
  app.post(
    "/v1/telemetry",
    bodyLimit({ maxSize: 1024, onError: (c) => c.json({ accepted: false }, 413) }),
    async (c) => {
      try {
        const accepted = ports.telemetry?.accept(await c.req.json()) ?? false;
        return c.json({ accepted }, accepted ? 202 : 400);
      } catch {
        return c.json({ accepted: false }, 400);
      }
    },
  );
  app.on(["GET", "POST"], "/api/auth/*", (c) => ports.auth.handler(c.req.raw));
  app.get("/v1/me", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    return session ? c.json({ user: session.user }) : c.json({ error: "Sign in required" }, 401);
  });
  app.get("/v1/entitlements", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    return c.json({
      entitlements: session ? await ports.billing.getEntitlements(session.user.id) : [],
    });
  });
  app.get("/v1/sessions", async (c) => {
    if (!(await ports.auth.getSession(credentialHeaders(c.req.raw.headers))))
      return c.json({ error: "Sign in required" }, 401);
    return c.json({ sessions: await ports.auth.sessions(credentialHeaders(c.req.raw.headers)) });
  });
  app.delete("/v1/sessions/:id", async (c) => {
    if (!(await ports.auth.getSession(credentialHeaders(c.req.raw.headers))))
      return c.json({ error: "Sign in required" }, 401);
    try {
      await ports.auth.revoke(credentialHeaders(c.req.raw.headers), c.req.param("id"));
    } catch {
      return c.json({ error: "Session unavailable" }, 404);
    }
    return c.json({ revoked: true });
  });
  app.post("/v1/billing/checkout", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    return c.json({ url: await ports.billing.createCheckout(session.user.id) });
  });
  app.post("/v1/billing/portal", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!ports.billing.createPortal) return c.json({ error: "Billing unavailable" }, 503);
    return c.json({ url: await ports.billing.createPortal(session.user.id) });
  });
  app.post("/v1/billing/webhook", async (c) => {
    try {
      await ports.billing.acceptWebhook(await c.req.text(), Object.fromEntries(c.req.raw.headers));
    } catch (error) {
      if (error instanceof InvalidWebhook) return c.json({ error: "Invalid billing webhook" }, 403);
      throw error;
    }
    return c.json({ received: true }, 202);
  });
  app.post("/v1/jobs", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    let job: ReturnType<typeof validateJob>;
    try {
      const body: unknown = await c.req.json();
      if (!body || typeof body !== "object" || "id" in body || "userId" in body) throw new Error();
      job = validateJob({ ...body, id: ports.newId(), userId: session.user.id });
    } catch {
      return c.json({ error: "Invalid job" }, 400);
    }
    const feature: Entitlement = job.kind === "pr_preview" ? "pr_previews" : "share_links";
    if (!(await ports.billing.getEntitlements(session.user.id)).includes(feature))
      return c.json({ error: "Pro required", feature }, 403);
    if (!(await ports.authorizeInput(session.user.id, job.inputId)))
      return c.json({ error: "Input unavailable" }, 404);
    return c.json({ id: await ports.jobs.enqueue(job) }, 202);
  });
  app.get("/v1/jobs/:id", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    const id = c.req.param("id");
    if (!identifier(id)) return c.json({ error: "Job unavailable" }, 404);
    const job = await ports.jobs.get(id, session.user.id);
    return job ? c.json(job) : c.json({ error: "Job unavailable" }, 404);
  });
  app.use("/v1/shares/*", async (c, next) => {
    c.header("Referrer-Policy", "no-referrer");
    c.header("X-Robots-Tag", "noindex, nofollow, noarchive");
    await next();
  });
  app.get("/v1/shares", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    return c.json({ shares: await ports.shares.list(session.user.id) });
  });
  app.post("/v1/shares", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!(await ports.billing.getEntitlements(session.user.id)).includes("share_links"))
      return c.json({ error: "Pro required" }, 403);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      const b = await c.req.json();
      return c.json(
        await ports.shares.begin(session.user.id, {
          size: b.size,
          sha256: b.sha256,
          shareId: b.shareId,
          ownerName: session.user.name,
        }),
        201,
      );
    } catch {
      return c.json({ error: "Upload unavailable; refresh and try again" }, 409);
    }
  });
  app.get("/v1/shares/:id/status", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      return c.json(await ports.shares.status(session.user.id, c.req.param("id")));
    } catch (e) {
      return e instanceof ShareUnavailable
        ? c.json({ error: "Share unavailable" }, 404)
        : c.json({ error: "Sharing unavailable" }, 503);
    }
  });
  app.delete("/v1/shares/:id", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      await ports.shares.revoke(session.user.id, c.req.param("id"));
      return c.json({ revoked: true });
    } catch {
      return c.json({ error: "Share unavailable; refresh and try again" }, 409);
    }
  });
  app.put("/v1/shares/:id/uploads/:upload/parts/:index", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!(await ports.billing.getEntitlements(session.user.id)).includes("share_links"))
      return c.json({ error: "Pro required" }, 403);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      await ports.shares.chunk(
        session.user.id,
        c.req.param("id"),
        c.req.param("upload"),
        Number(c.req.param("index")),
        new Uint8Array(await c.req.arrayBuffer()),
      );
      return c.json({ received: true });
    } catch {
      return c.json({ error: "Upload unavailable" }, 409);
    }
  });
  app.post("/v1/shares/:id/uploads/:upload/publish", async (c) => {
    const session = await ports.auth.getSession(credentialHeaders(c.req.raw.headers));
    if (!session) return c.json({ error: "Sign in required" }, 401);
    if (!(await ports.billing.getEntitlements(session.user.id)).includes("share_links"))
      return c.json({ error: "Pro required" }, 403);
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      return c.json(
        await ports.shares.publish(session.user.id, c.req.param("id"), c.req.param("upload")),
      );
    } catch {
      return c.json({ error: "Snapshot incomplete or changed; retry" }, 409);
    }
  });
  app.get("/v1/shares/:id/download", async (c) => {
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      const view = await ports.shares.view(c.req.param("id"));
      c.header("Content-Disposition", 'attachment; filename="design.zip"');
      return c.body(new Uint8Array(zipBundle(decodeBundle(view.bytes))), 200, {
        "Content-Type": "application/zip",
      });
    } catch (e) {
      return e instanceof ShareUnavailable
        ? c.json({ error: "Share unavailable" }, 404)
        : c.json({ error: "Sharing unavailable" }, 503);
    }
  });
  app.get("/v1/shares/:id", async (c) => {
    if (!ports.shares) return c.json({ error: "Sharing unavailable" }, 503);
    try {
      const view = await ports.shares.view(c.req.param("id"));
      return c.json({ snapshot: view.snapshot, bundle: decodeBundle(view.bytes) });
    } catch (e) {
      return e instanceof ShareUnavailable
        ? c.json({ error: "Share unavailable" }, 404)
        : c.json({ error: "Sharing unavailable" }, 503);
    }
  });
  return app;
}
