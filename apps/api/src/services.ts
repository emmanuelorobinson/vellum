import { randomBytes } from "node:crypto";
import { createAuth } from "@vellum/auth";
import { Pool, postgresAuth } from "@vellum/auth/postgres";
import { createBilling } from "@vellum/billing";
import { polarBilling } from "@vellum/billing/polar";
import { postgresEntitlements } from "@vellum/billing/postgres";
import { bullQueue } from "@vellum/jobs/bullmq";
import { sesMailer } from "@vellum/mail";
import { ShareService } from "@vellum/storage";
import { r2Objects } from "@vellum/storage/r2";
import { required } from "./config";
export function services(env: NodeJS.ProcessEnv) {
  const read = (name: string) => required(env, name);
  const baseURL = read("VELLUM_ACCOUNT_URL");
  const pool = new Pool({ connectionString: read("DATABASE_URL"), max: 10 });
  pool.on("error", () => {
    console.error("Account database unavailable");
  });
  const mail = sesMailer(read("AWS_REGION"), read("VELLUM_MAIL_FROM"));
  const auth = createAuth({
    baseURL,
    secret: read("BETTER_AUTH_SECRET"),
    database: postgresAuth(pool),
    sendLink: ({ email, url }) =>
      mail.send({
        to: email,
        subject: "Sign in",
        text: `Open this link to sign in. It expires in five minutes.\n\n${url}\n\nIf you did not request this, ignore this email.`,
      }),
  });
  const productId = read("POLAR_PRO_PRODUCT_ID");
  const billing = createBilling({
    clock: Date.now,
    store: postgresEntitlements(pool),
    products: [productId],
    provider: polarBilling({
      productId,
      accessToken: read("POLAR_ACCESS_TOKEN"),
      webhookSecret: read("POLAR_WEBHOOK_SECRET"),
      successUrl: new URL("/", baseURL).href,
      sandbox: env.POLAR_SANDBOX === "true",
    }),
  });
  const jobs = bullQueue({ url: read("REDIS_URL") });
  return {
    shares: new ShareService({
      objects: r2Objects({
        endpoint: read("R2_ENDPOINT"),
        bucket: read("R2_BUCKET"),
        accessKeyId: read("R2_ACCESS_KEY_ID"),
        secretAccessKey: read("R2_SECRET_ACCESS_KEY"),
      }),
      clock: Date.now,
      newId: () => randomBytes(32).toString("base64url"),
    }),
    auth,
    billing,
    jobs,
    accountOrigin: new URL(baseURL).origin,
    close: async () => {
      await jobs.close();
      await pool.end();
    },
  };
}
