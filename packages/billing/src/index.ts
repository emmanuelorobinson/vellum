/** Vellum feature vocabulary. Cloud dependencies never participate in a local operation. */
export const PACKAGE_NAME = "@vellum/billing";
export const CLOUD_FEATURES = [
  "cloud_sync",
  "share_links",
  "pr_previews",
  "premium_systems",
] as const;
export const LOCAL_FEATURES = [
  "editor",
  "cli",
  "mcp",
  "local_files",
  "community_systems",
  "exports",
  "builtin_agent",
  "installed_agents",
  "git",
  "self_hosted_sync",
] as const;
export type Entitlement = (typeof CLOUD_FEATURES)[number];
export type Feature = Entitlement | (typeof LOCAL_FEATURES)[number];
export function canUse(feature: Feature, grants: readonly Entitlement[]): boolean {
  return (
    (LOCAL_FEATURES as readonly string[]).includes(feature) ||
    (CLOUD_FEATURES.includes(feature as Entitlement) && grants.includes(feature as Entitlement))
  );
}
export interface SubscriptionUpdate {
  eventId: string;
  subscriptionId: string;
  userId: string;
  productId: string;
  /** Vendor resource modification time, UTC milliseconds. Equal-time denial wins. */
  version: number;
  active: boolean;
  validUntil: number;
}
export interface EntitlementStore {
  /** Atomic deduplication and conditional subscription upsert. */
  apply(update: SubscriptionUpdate): Promise<void>;
  grants(userId: string, now: number, products: readonly string[]): Promise<Entitlement[]>;
}
export interface BillingProvider {
  createCheckout(userId: string): Promise<string>;
  parseWebhook(
    body: string,
    headers: Record<string, string>,
  ): SubscriptionUpdate | null | Promise<SubscriptionUpdate | null>;
}
export interface Billing {
  createCheckout(userId: string): Promise<string>;
  getEntitlements(userId: string): Promise<Entitlement[]>;
  acceptWebhook(body: string, headers: Record<string, string>): Promise<void>;
}
export function createBilling(options: {
  store: EntitlementStore;
  provider: BillingProvider;
  products: readonly string[];
  clock: () => number;
}): Billing {
  return {
    createCheckout: (userId) => options.provider.createCheckout(userId),
    getEntitlements: (userId) => options.store.grants(userId, options.clock(), options.products),
    async acceptWebhook(body, headers) {
      const update = await options.provider.parseWebhook(body, headers);
      if (update)
        await options.store.apply({
          ...update,
          active: update.active && options.products.includes(update.productId),
        });
    },
  };
}
export function validateUpdate(update: SubscriptionUpdate) {
  if (
    ![update.eventId, update.subscriptionId, update.userId, update.productId].every(
      (x) => typeof x === "string" && x.length > 0 && x.length <= 256,
    ) ||
    !Number.isSafeInteger(update.version) ||
    !Number.isSafeInteger(update.validUntil) ||
    typeof update.active !== "boolean"
  )
    throw new Error("Invalid subscription");
}
/** Fake with the same ordering, expiry, ownership and replay contract as Postgres. */
export class MemoryEntitlementStore implements EntitlementStore {
  private events = new Set<string>();
  private subscriptions = new Map<string, SubscriptionUpdate>();
  async apply(update: SubscriptionUpdate) {
    validateUpdate(update);
    if (this.events.has(update.eventId)) return;
    const old = this.subscriptions.get(update.subscriptionId);
    if (old && old.userId !== update.userId) throw new Error("Subscription owner mismatch");
    this.events.add(update.eventId);
    if (!old || update.version > old.version || (update.version === old.version && !update.active))
      this.subscriptions.set(update.subscriptionId, { ...update });
  }
  async grants(userId: string, now: number, products: readonly string[]): Promise<Entitlement[]> {
    return [...this.subscriptions.values()].some(
      (x) =>
        x.userId === userId && x.active && x.validUntil > now && products.includes(x.productId),
    )
      ? [...CLOUD_FEATURES]
      : [];
  }
}
