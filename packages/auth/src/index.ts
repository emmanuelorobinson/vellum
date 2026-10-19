import { type BetterAuthOptions, betterAuth } from "better-auth";
import { bearer } from "better-auth/plugins/bearer";
import { deviceAuthorization } from "better-auth/plugins/device-authorization";
import { magicLink } from "better-auth/plugins/magic-link";
export const PACKAGE_NAME = "@vellum/auth";
export interface User {
  id: string;
  email: string;
  name: string;
}
export interface Auth {
  handler(request: Request): Promise<Response>;
  getSession(headers: Headers): Promise<{ user: User; id: string } | null>;
  requireUser(headers: Headers): Promise<User>;
  verifyDeviceCode(headers: Headers, userCode: string): Promise<unknown>;
  sessions(headers: Headers): Promise<{ id: string; expiresAt: number; current: boolean }[]>;
  revoke(headers: Headers, id: string): Promise<void>;
}
export class SignInRequired extends Error {
  constructor() {
    super("Sign in required");
  }
}
export function createAuth(options: {
  baseURL: string;
  secret: string;
  database: BetterAuthOptions["database"];
  sendLink(message: { email: string; url: string }): Promise<void>;
}): Auth {
  const url = new URL(options.baseURL);
  if (
    (url.protocol !== "https:" &&
      !(url.protocol === "http:" && ["localhost", "127.0.0.1"].includes(url.hostname))) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    options.secret.length < 32
  )
    throw new Error("Invalid authentication configuration");
  const auth = betterAuth({
    baseURL: options.baseURL,
    secret: options.secret,
    database: options.database,
    trustedOrigins: [url.origin],
    telemetry: { enabled: false },
    logger: { disabled: true },
    advanced: { useSecureCookies: url.protocol === "https:" },
    session: { expiresIn: 60 * 60 * 24 * 30, cookieCache: { enabled: false } },
    rateLimit: { enabled: true, storage: "database", window: 60, max: 60 },
    plugins: [
      bearer(),
      deviceAuthorization({
        verificationUri: `${url.origin}/device`,
        expiresIn: "10m",
        interval: "5s",
        validateClient: (id) => ["vellum-cli", "vellum-desktop", "vellum-extension"].includes(id),
      }),
      magicLink({ storeToken: "hashed", sendMagicLink: options.sendLink }),
    ],
  });
  const getSession: Auth["getSession"] = async (headers) => {
    const result = await auth.api.getSession({ headers });
    return result
      ? {
          user: { id: result.user.id, email: result.user.email, name: result.user.name },
          id: result.session.id,
        }
      : null;
  };
  return {
    handler: (request) => auth.handler(request),
    getSession,
    async requireUser(headers) {
      const session = await getSession(headers);
      if (!session) throw new SignInRequired();
      return session.user;
    },
    verifyDeviceCode: (headers, userCode) =>
      auth.api.deviceVerify({ headers, query: { user_code: userCode } }),
    async sessions(headers) {
      const current = await getSession(headers);
      if (!current) throw new SignInRequired();
      return (await auth.api.listSessions({ headers })).map((s) => ({
        id: s.id,
        expiresAt: s.expiresAt.getTime(),
        current: s.id === current.id,
      }));
    },
    async revoke(headers, id) {
      if (!(await getSession(headers))) throw new SignInRequired();
      const target = (await auth.api.listSessions({ headers })).find((s) => s.id === id);
      if (!target) throw new Error("Session unavailable");
      await auth.api.revokeSession({ headers, body: { token: target.token } });
    },
  };
}
