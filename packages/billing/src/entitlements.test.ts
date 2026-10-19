// TEST DATA: synthetic accounts and subscriptions, no vendor requests.
import { describe, expect, it } from "vitest";
import {
  CLOUD_FEATURES,
  canUse,
  createBilling,
  LOCAL_FEATURES,
  MemoryEntitlementStore,
} from "./index";

const event = (id: string, version = 10, active = true, subscriptionId = "sub") => ({
  eventId: id,
  subscriptionId,
  userId: "person",
  productId: "pro",
  version,
  active,
  validUntil: 100,
});
const setup = () => {
  const store = new MemoryEntitlementStore();
  const billing = createBilling({
    store,
    clock: () => 50,
    products: ["pro"],
    provider: {
      createCheckout: async () => "https://checkout.example.test",
      parseWebhook: () => null,
    },
  });
  return { store, billing };
};
describe("entitlement boundary", () => {
  it("keeps every local feature available signed out, offline, free, expired or Pro", () => {
    for (const grants of [[], CLOUD_FEATURES])
      for (const feature of LOCAL_FEATURES) {
        expect(canUse(feature, grants)).toBe(true);
      }
    for (const feature of CLOUD_FEATURES) {
      expect(canUse(feature, [])).toBe(false);
      expect(canUse(feature, CLOUD_FEATURES)).toBe(true);
    }
    expect(canUse("invented" as never, CLOUD_FEATURES)).toBe(false);
  });
  it("deduplicates, rejects stale delivery and expires without a webhook", async () => {
    const { store, billing } = setup();
    await store.apply(event("one"));
    await store.apply(event("one", 20, false));
    expect(await billing.getEntitlements("person")).toEqual(CLOUD_FEATURES);
    await store.apply(event("two", 20, false));
    await store.apply(event("old", 15));
    expect(await billing.getEntitlements("person")).toEqual([]);
    await store.apply(event("new", 30));
    expect(await store.grants("person", 100, ["pro"])).toEqual([]);
    expect(await store.grants("other", 50, ["pro"])).toEqual([]);
    expect(await store.grants("person", 50, ["unknown"])).toEqual([]);
  });
  it("keeps independent subscriptions and makes equal-version denial win", async () => {
    const { store, billing } = setup();
    await store.apply(event("a"));
    await store.apply(event("b", 10, true, "second"));
    await store.apply(event("c", 10, false));
    await store.apply(event("d", 10));
    expect(await billing.getEntitlements("person")).toEqual(CLOUD_FEATURES);
    await store.apply(event("e", 10, false, "second"));
    expect(await billing.getEntitlements("person")).toEqual([]);
  });
});
