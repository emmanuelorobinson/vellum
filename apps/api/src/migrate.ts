/** Explicit operator command. Never runs on service startup or against a default database. */
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { Pool } from "@vellum/auth/postgres";
import { required } from "./config";

const pool = new Pool({ connectionString: required(process.env, "DATABASE_URL"), max: 1 });
try {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    await client.query("SELECT pg_advisory_xact_lock(610061)");
    await client.query(
      "CREATE TABLE IF NOT EXISTS cloud_migration (name text PRIMARY KEY, checksum text NOT NULL)",
    );
    for (const name of ["auth", "billing"]) {
      const text = await readFile(
        new URL(import.meta.resolve(`@vellum/${name}/migration.sql`)),
        "utf8",
      );
      const checksum = createHash("sha256").update(text).digest("hex");
      const existing = await client.query<{ checksum: string }>(
        "SELECT checksum FROM cloud_migration WHERE name=$1",
        [name],
      );
      if (existing.rows[0]) {
        if (existing.rows[0].checksum !== checksum) throw new Error("Migration changed");
        continue;
      }
      await client.query(text);
      await client.query("INSERT INTO cloud_migration(name,checksum) VALUES($1,$2)", [
        name,
        checksum,
      ]);
    }
    await client.query("COMMIT");
    console.log("Cloud migrations applied");
  } catch {
    await client.query("ROLLBACK");
    throw new Error("Migration failed");
  } finally {
    client.release();
  }
} catch {
  console.error("Cloud migrations failed");
  process.exitCode = 1;
} finally {
  await pool.end();
}
