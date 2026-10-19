// TEST DATA. Uses Better Auth's in-memory adapter; no mail or network.

import { memoryAdapter } from "better-auth/adapters/memory";
import { expect, it, vi } from "vitest";
import { createAuth } from "./index";

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
