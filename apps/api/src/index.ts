import type { Auth } from "@vellum/auth";
import type { Billing, Entitlement } from "@vellum/billing";
import { InvalidWebhook } from "@vellum/billing/polar";
import { identifier, type JobQueue, validateJob } from "@vellum/jobs";
import { Hono } from "hono";
import { bodyLimit } from "hono/body-limit";
export const PACKAGE_NAME = "@vellum/api";
function credentialHeaders(raw: Headers): Headers {
  const headers = new Headers(raw);
  if (headers.has("authorization")) headers.delete("cookie");
  return headers;
}
export function createApi(ports: {
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
      !c.req.header("authorization")
    ) {
      if (c.req.header("origin") !== ports.accountOrigin)
        return c.json({ error: "Untrusted origin" }, 403);
    }
    await next();
  });
  app.onError(
    () =>
      new Response(JSON.stringify({ error: "Cloud service unavailable" }), {
        status: 503,
        headers: { "content-type": "application/json", "cache-control": "no-store" },
      }),
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
  return app;
}
