import { createAuth } from "@vellum/auth";
import { Pool, postgresAuth } from "@vellum/auth/postgres";
import { sesMailer } from "@vellum/mail";

let instance: ReturnType<typeof createAuth> | undefined;
export function accountAuth() {
  if (instance) return instance;
  const read = (name: string) => {
    const value = process.env[name];
    if (!value) throw new Error(`Missing ${name}`);
    return value;
  };
  const mail = sesMailer(read("AWS_REGION"), read("VELLUM_MAIL_FROM"));
  const pool = new Pool({ connectionString: read("DATABASE_URL"), max: 5 });
  pool.on("error", () => {
    console.error("Account database unavailable");
  });
  instance = createAuth({
    baseURL: read("VELLUM_ACCOUNT_URL"),
    secret: read("BETTER_AUTH_SECRET"),
    database: postgresAuth(pool),
    sendLink: ({ email, url }) =>
      mail.send({
        to: email,
        subject: "Sign in",
        text: `Open this link to sign in. It expires in five minutes.\n\n${url}\n\nIf you did not request this, ignore this email.`,
      }),
  });
  return instance;
}
