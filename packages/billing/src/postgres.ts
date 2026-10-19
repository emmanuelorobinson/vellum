import { and, eq, gt, inArray, sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";
import { bigint, boolean, pgTable, text } from "drizzle-orm/pg-core";
import type { Pool } from "pg";
import {
  CLOUD_FEATURES,
  type EntitlementStore,
  type SubscriptionUpdate,
  validateUpdate,
} from "./index";
export const subscriptions = pgTable("entitlements", {
  subscriptionId: text("subscription_id").primaryKey(),
  userId: text("user_id").notNull(),
  productId: text("product_id").notNull(),
  version: bigint("version", { mode: "number" }).notNull(),
  active: boolean("active").notNull(),
  validUntil: bigint("valid_until", { mode: "number" }).notNull(),
});
export const deliveries = pgTable("billing_delivery", { eventId: text("event_id").primaryKey() });
export function postgresEntitlements(pool: Pool): EntitlementStore {
  const db = drizzle(pool);
  return {
    async apply(update: SubscriptionUpdate) {
      validateUpdate(update);
      await db.transaction(async (tx) => {
        const inserted = await tx
          .insert(deliveries)
          .values({ eventId: update.eventId })
          .onConflictDoNothing()
          .returning();
        if (!inserted.length) return;
        // Serializes both the first insert and updates for this subscription. The lock key isn't a secret.
        await tx.execute(
          sql`select pg_advisory_xact_lock(hashtextextended(${update.subscriptionId},0))`,
        );
        const [old] = await tx
          .select()
          .from(subscriptions)
          .where(eq(subscriptions.subscriptionId, update.subscriptionId));
        if (old && old.userId !== update.userId) throw new Error("Subscription owner mismatch");
        if (
          old &&
          (old.version > update.version || (old.version === update.version && update.active))
        )
          return;
        const { eventId: _, ...row } = update;
        await tx
          .insert(subscriptions)
          .values(row)
          .onConflictDoUpdate({ target: subscriptions.subscriptionId, set: row });
      });
    },
    async grants(userId, now, products) {
      if (!products.length) return [];
      const rows = await db
        .select({ id: subscriptions.subscriptionId })
        .from(subscriptions)
        .where(
          and(
            eq(subscriptions.userId, userId),
            eq(subscriptions.active, true),
            gt(subscriptions.validUntil, now),
            inArray(subscriptions.productId, [...products]),
          ),
        )
        .limit(1);
      return rows.length ? [...CLOUD_FEATURES] : [];
    },
  };
}
