import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { drizzle } from "drizzle-orm/node-postgres";
import type { Pool } from "pg";
import * as schema from "./schema";

export { Pool } from "pg";
export function postgresAuth(pool: Pool) {
  return drizzleAdapter(drizzle(pool, { schema }), { provider: "pg", schema });
}
