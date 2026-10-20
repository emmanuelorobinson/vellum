// TEST DATA. Tagged @cloud: requires isolated Postgres/Redis; no external vendor or real keychain.
import { randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createAuth } from "@vellum/auth";
import { Pool, postgresAuth } from "@vellum/auth/postgres";
import { CLOUD_FEATURES } from "@vellum/billing";
import { postgresEntitlements } from "@vellum/billing/postgres";
import { bullQueue, bullWorker } from "@vellum/jobs/bullmq";
import { expect, it } from "vitest";

const databaseURL = process.env.VELLUM_TEST_DATABASE_URL;
const redisURL = process.env.VELLUM_TEST_REDIS_URL;
it.skipIf(!databaseURL)(
  "@cloud Postgres migrations, one-time device claim, revocation and concurrent entitlement replay",
  async () => {
    const admin = new Pool({ connectionString: databaseURL });
    const schema = `p61_${randomUUID().replaceAll("-", "")}`;
    await admin.query(`CREATE SCHEMA ${schema}`);
    const pool = new Pool({ connectionString: databaseURL, options: `-c search_path=${schema}` });
    try {
      for (const name of ["auth", "billing"])
        await pool.query(
          await readFile(new URL(import.meta.resolve(`@vellum/${name}/migration.sql`)), "utf8"),
        );
      let link = "";
      const auth = createAuth({
        baseURL: "http://localhost:3000",
        secret: "test-only-configuration-value-32-characters",
        database: postgresAuth(pool),
        sendLink: async (m) => {
          link = m.url;
        },
      });
      const post = (path: string, body: unknown, headers: Record<string, string> = {}) =>
        auth.handler(
          new Request(`http://localhost:3000/api/auth${path}`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
              origin: "http://localhost:3000",
              ...headers,
            },
            body: JSON.stringify(body),
          }),
        );
      expect(
        (
          await post("/sign-in/magic-link", {
            email: "person@example.test",
            callbackURL: "/device",
          })
        ).status,
      ).toBe(200);
      const verified = await auth.handler(new Request(link));
      const cookie = verified.headers
        .getSetCookie()
        .map((x) => x.split(";")[0])
        .join("; ");
      const session = await auth.getSession(new Headers({ cookie }));
      expect(session !== null).toBe(true);
      const response = await post("/device/code", { client_id: "vellum-cli" });
      expect(response.status).toBe(200);
      const code = (await response.json()) as { device_code: string; user_code: string };
      await auth.verifyDeviceCode(new Headers({ cookie }), code.user_code);
      expect((await post("/device/approve", { userCode: code.user_code }, { cookie })).status).toBe(
        200,
      );
      const redeem = () =>
        post("/device/token", {
          client_id: "vellum-cli",
          device_code: code.device_code,
          grant_type: "urn:ietf:params:oauth:grant-type:device_code",
        });
      const tokens = await Promise.all([redeem(), redeem()]);
      expect(tokens.map((t) => t.status).sort()).toEqual([200, 400]);
      const token = (await tokens.find((t) => t.status === 200)?.json()) as {
        access_token: string;
      };
      const bearer = new Headers({ authorization: `Bearer ${token.access_token}` });
      const cli = await auth.getSession(bearer);
      expect(cli !== null).toBe(true);
      const browser = new Headers({ cookie });
      const mixed = new Headers({ cookie, authorization: "Bearer invalid.signature" });
      expect(await auth.getSession(mixed)).toBeNull();
      expect(
        await (
          await auth.handler(
            new Request("http://localhost:3000/api/auth/get-session", {
              headers: mixed,
            }),
          )
        ).json(),
      ).toBeNull();
      expect((await post("/sign-out", {}, Object.fromEntries(mixed))).status).toBe(200);
      expect((await auth.getSession(browser))?.id).toBe(session?.id);
      expect(
        (
          await auth.getSession(
            new Headers({ cookie, authorization: bearer.get("authorization") ?? "" }),
          )
        )?.id,
      ).toBe(cli?.id);
      await auth.revoke(new Headers({ cookie }), cli?.id ?? "");
      expect(await auth.getSession(bearer)).toBeNull();
      expect(
        await auth.getSession(
          new Headers({ cookie, authorization: bearer.get("authorization") ?? "" }),
        ),
      ).toBeNull();
      expect((await auth.getSession(browser))?.id).toBe(session?.id);
      const store = postgresEntitlements(pool),
        userId = session?.user.id ?? "";
      const update = (eventId: string, version: number, active = true) => ({
        eventId,
        version,
        active,
        userId,
        productId: "pro",
        subscriptionId: "sub",
        validUntil: 100,
      });
      await Promise.all([
        store.apply(update("a", 10)),
        store.apply(update("b", 30, false)),
        store.apply(update("c", 20)),
      ]);
      expect(await store.grants(userId, 50, ["pro"])).toEqual([]);
      await store.apply(update("b", 40));
      expect(await store.grants(userId, 50, ["pro"])).toEqual([]);
      await store.apply(update("d", 40));
      expect(await store.grants(userId, 50, ["pro"])).toEqual(CLOUD_FEATURES);
      expect(await store.grants(userId, 100, ["pro"])).toEqual([]);
      expect(await store.grants("other", 50, ["pro"])).toEqual([]);
      const reconnected = postgresEntitlements(pool);
      expect(await reconnected.grants(userId, 50, ["pro"])).toEqual(CLOUD_FEATURES);
    } finally {
      await pool.end();
      await admin.query(`DROP SCHEMA ${schema} CASCADE`);
      await admin.end();
    }
  },
  20000,
);
it.skipIf(!redisURL)(
  "@cloud Redis worker consumes jobs, preserves ownership and returns safe artifacts",
  async () => {
    const queue = bullQueue({ url: redisURL ?? "" });
    const worker = bullWorker({ url: redisURL ?? "" }, async (job) => `artifact-${job.id}`);
    worker.on("error", () => {});
    const id = randomUUID(),
      job = { id, userId: "person", inputId: "bundle", kind: "thumbnail" as const };
    try {
      await worker.waitUntilReady();
      await queue.enqueue(job);
      await queue.enqueue(job);
      await queue.enqueue({ kind: job.kind, inputId: job.inputId, userId: job.userId, id: job.id });
      await expect(queue.enqueue({ ...job, userId: "other" })).rejects.toThrow();
      expect(await queue.get(id, "other")).toBeNull();
      for (let attempt = 0; attempt < 100; attempt++) {
        if ((await queue.get(id, "person"))?.state === "completed") break;
        await new Promise((resolve) => setTimeout(resolve, 20));
      }
      expect(await queue.get(id, "person")).toMatchObject({
        state: "completed",
        artifact: `artifact-${id}`,
      });
    } finally {
      await worker.close();
      await queue.close();
    }
  },
  10000,
);
