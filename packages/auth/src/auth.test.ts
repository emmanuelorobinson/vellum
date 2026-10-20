// TEST DATA. Uses Better Auth's in-memory adapter; no mail or network.

import { memoryAdapter } from "better-auth/adapters/memory";
import { expect, it, vi } from "vitest";
import { createAuth } from "./index";

async function signedInFixture() {
  let link = "";
  const auth = createAuth({
    baseURL: "http://localhost:3000",
    secret: "test-only-configuration-value-32-characters",
    database: memoryAdapter({
      user: [],
      session: [],
      account: [],
      verification: [],
      deviceCode: [],
      rateLimit: [],
    }),
    sendLink: async (message) => {
      link = message.url;
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
  const signIn = async (email = "person@example.test") => {
    expect((await post("/sign-in/magic-link", { email, callbackURL: "/device" })).status).toBe(200);
    const verified = await auth.handler(new Request(link));
    return verified.headers
      .getSetCookie()
      .map((x) => x.split(";")[0])
      .join("; ");
  };
  const cookie = await signIn();
  return { auth, cookie, post, signIn };
}

it.each(["", "Basic invalid", "Bearer", "Bearer invalid.signature"])(
  "does not use a browser cookie when Authorization is %j, across wrapper and protocol routes",
  async (authorization) => {
    const { auth, cookie } = await signedInFixture();
    const browser = new Headers({ cookie });
    const original = await auth.getSession(browser);
    expect(original).not.toBeNull();
    const mixed = new Headers({ cookie, authorization });
    expect(await auth.getSession(mixed)).toBeNull();
    await expect(auth.requireUser(mixed)).rejects.toThrow("Sign in required");
    await expect(auth.sessions(mixed)).rejects.toThrow("Sign in required");
    await expect(auth.revoke(mixed, original?.id ?? "")).rejects.toThrow("Sign in required");
    const response = await auth.handler(
      new Request("http://localhost:3000/api/auth/get-session", { headers: mixed }),
    );
    expect(await response.json()).toBeNull();
    const signOut = await auth.handler(
      new Request("http://localhost:3000/api/auth/sign-out", {
        method: "POST",
        headers: new Headers({
          cookie,
          authorization,
          origin: "http://localhost:3000",
          "content-type": "application/json",
        }),
        body: "{}",
      }),
    );
    // The protocol may acknowledge an unauthenticated sign-out, but it must keep the cookie session.
    expect(signOut.status).toBeLessThan(500);
    expect(await auth.getSession(browser)).toEqual(original);
    expect(mixed.get("cookie")).toBe(cookie);
  },
);

it("requires an origin shared by the CLI, account page and device protocol", () => {
  for (const baseURL of [
    "https://account.example.test/nested",
    "https://account.example.test/api/auth",
  ]) {
    expect(() =>
      createAuth({
        baseURL,
        secret: "test-only-configuration-value-32-characters",
        database: memoryAdapter({}),
        sendLink: vi.fn(),
      }),
    ).toThrow("Invalid authentication configuration");
  }
});

it("requires the claiming account for approval or denial, then consumes a denial without issuing a session", async () => {
  const { auth, cookie, post, signIn } = await signedInFixture();
  const other = await signIn("other@example.test");
  const response = await post("/device/code", { client_id: "vellum-cli" });
  const code = (await response.json()) as { user_code: string; device_code: string };
  expect((await post("/device/approve", { userCode: code.user_code }, { cookie })).status).toBe(
    400,
  );
  await auth.verifyDeviceCode(new Headers({ cookie }), code.user_code);
  for (const path of ["/device/approve", "/device/deny"]) {
    expect((await post(path, { userCode: code.user_code }, { cookie: other })).status).toBe(403);
    expect(
      (
        await post(
          path,
          { userCode: code.user_code },
          { cookie, authorization: "Bearer invalid.signature" },
        )
      ).status,
    ).toBe(401);
  }
  expect((await post("/device/deny", { userCode: code.user_code }, { cookie })).status).toBe(200);
  const redeem = () =>
    post("/device/token", {
      client_id: "vellum-cli",
      device_code: code.device_code,
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
    });
  expect(await (await redeem()).json()).toMatchObject({ error: "access_denied" });
  expect(await (await redeem()).json()).toMatchObject({ error: "invalid_grant" });
  expect((await auth.sessions(new Headers({ cookie }))).length).toBe(1);
});

it("expires device grants and enforces client binding and polling intervals", async () => {
  const { auth, cookie, post } = await signedInFixture();
  const response = await post("/device/code", { client_id: "vellum-cli" });
  const code = (await response.json()) as { user_code: string; device_code: string };
  const redeem = (client_id = "vellum-cli") =>
    post("/device/token", {
      client_id,
      device_code: code.device_code,
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
    });
  expect((await redeem("vellum-extension")).status).toBe(400);
  expect(await (await redeem()).json()).toMatchObject({ error: "authorization_pending" });
  expect(await (await redeem()).json()).toMatchObject({ error: "slow_down" });
  // Advance the vendor boundary clock; production logic does not sleep or invent a second expiry store.
  vi.useFakeTimers({ toFake: ["Date"] });
  try {
    vi.setSystemTime(Date.now() + 601_000);
    await expect(auth.verifyDeviceCode(new Headers({ cookie }), code.user_code)).rejects.toThrow();
    expect((await post("/device/approve", { userCode: code.user_code }, { cookie })).status).toBe(
      400,
    );
    expect(await (await redeem()).json()).toMatchObject({ error: "expired_token" });
    expect((await auth.sessions(new Headers({ cookie }))).length).toBe(1);
  } finally {
    vi.useRealTimers();
  }
});

it("requests device codes only for Vellum, requires a person to approve, and keeps secrets out of session views", async () => {
  const auth = createAuth({
    baseURL: "http://localhost:3000",
    secret: "test-only-configuration-value-32-characters",
    database: memoryAdapter({
      user: [],
      session: [],
      account: [],
      verification: [],
      deviceCode: [],
      rateLimit: [],
    }),
    sendLink: vi.fn(),
  });
  const post = (path: string, body: unknown) =>
    auth.handler(
      new Request(`http://localhost:3000/api/auth${path}`, {
        method: "POST",
        headers: { "content-type": "application/json", origin: "http://localhost:3000" },
        body: JSON.stringify(body),
      }),
    );
  expect((await post("/device/code", { client_id: "unknown" })).status).toBe(400);
  const response = await post("/device/code", { client_id: "vellum-cli" });
  expect(response.status).toBe(200);
  const data = (await response.json()) as { user_code: string };
  expect(typeof data.user_code).toBe("string");
  expect((await post("/device/approve", { userCode: data.user_code })).status).toBe(401);
  expect(await auth.getSession(new Headers())).toBeNull();
  await expect(auth.requireUser(new Headers())).rejects.toThrow("Sign in required");
});
it("completes one-time device approval, exposes only safe session IDs, and revokes the bearer", async () => {
  let link = "";
  const db = {
    user: [],
    session: [],
    account: [],
    verification: [],
    deviceCode: [],
    rateLimit: [],
  };
  const auth = createAuth({
    baseURL: "http://localhost:3000",
    secret: "test-only-configuration-value-32-characters",
    database: memoryAdapter(db),
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
  await post("/sign-in/magic-link", { email: "person@example.test", callbackURL: "/device" });
  expect(link.length > 0).toBe(true);
  const verified = await auth.handler(new Request(link));
  const cookie = verified.headers
    .getSetCookie()
    .map((x) => x.split(";")[0])
    .join("; ");
  expect((await auth.getSession(new Headers({ cookie }))) !== null).toBe(true);
  const code = (await (await post("/device/code", { client_id: "vellum-cli" })).json()) as {
    device_code: string;
    user_code: string;
  };
  await auth.verifyDeviceCode(new Headers({ cookie }), code.user_code);
  expect((await post("/device/approve", { userCode: code.user_code }, { cookie })).status).toBe(
    200,
  );
  const tokenResponse = await post("/device/token", {
    client_id: "vellum-cli",
    device_code: code.device_code,
    grant_type: "urn:ietf:params:oauth:grant-type:device_code",
  });
  expect(tokenResponse.status).toBe(200);
  const token = (await tokenResponse.json()) as { access_token: string };
  const bearer = new Headers({ authorization: `Bearer ${token.access_token}` });
  const safe = await auth.getSession(bearer);
  expect(safe !== null).toBe(true);
  expect(JSON.stringify(safe).includes(token.access_token)).toBe(false);
  expect(
    (
      await post("/device/token", {
        client_id: "vellum-cli",
        device_code: code.device_code,
        grant_type: "urn:ietf:params:oauth:grant-type:device_code",
      })
    ).status,
  ).toBe(400);
  const sessions = await auth.sessions(bearer);
  expect(sessions.length).toBe(2);
  expect(JSON.stringify(sessions).includes(token.access_token)).toBe(false);
  const signedOut = await auth.handler(
    new Request("http://localhost:3000/api/auth/sign-out", {
      method: "POST",
      headers: {
        authorization: `Bearer ${token.access_token}`,
        "content-type": "application/json",
      },
      body: "{}",
    }),
  );
  expect(signedOut.status).toBe(200);
  expect(await auth.getSession(bearer)).toBeNull();
});
