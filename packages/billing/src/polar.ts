import { createPolar, webhooks } from "@polar-sh/sdk/2026-10";
import { type BillingProvider, validateUpdate } from "./index";
export class InvalidWebhook extends Error {
  constructor() {
    super("Invalid billing webhook");
  }
}
export function polarBilling(options: {
  accessToken: string;
  webhookSecret: string;
  productId: string;
  successUrl: string;
  sandbox?: boolean;
}): BillingProvider {
  const client = createPolar({
    accessToken: options.accessToken,
    environment: options.sandbox ? "sandbox" : "production",
  });
  return {
    async createPortal(userId) {
      try {
        const session = await client.customerSessions.create({ external_customer_id: userId });
        return session.customer_portal_url;
      } catch {
        throw new Error("Billing unavailable");
      }
    },
    async createCheckout(userId) {
      try {
        const checkout = await client.checkouts.create({
          products: [options.productId],
          external_customer_id: userId,
          success_url: options.successUrl,
        });
        return checkout.url;
      } catch {
        throw new Error("Billing unavailable");
      }
    },
    async parseWebhook(body, headers) {
      let event: Awaited<ReturnType<typeof webhooks.validateEvent>>;
      try {
        event = await webhooks.validateEvent(body, headers, options.webhookSecret);
      } catch {
        throw new InvalidWebhook();
      }
      if (!event.type.startsWith("subscription.")) return null;
      // The SDK verifies signature, freshness and event type. Validate our normalized fields too.
      if (!("current_period_end" in event.data) || !("customer" in event.data)) return null;
      const s = event.data;
      if (!s.customer.external_id) return null;
      const version = Date.parse(s.modified_at ?? s.created_at);
      const end = Math.min(
        (s.current_period_end ? Date.parse(s.current_period_end) : undefined) ?? 0,
        (s.ends_at ? Date.parse(s.ends_at) : undefined) ?? Number.MAX_SAFE_INTEGER,
      );
      const update = {
        eventId: headers["webhook-id"] ?? "",
        subscriptionId: s.id,
        userId: s.customer.external_id,
        productId: s.product_id,
        version,
        active:
          event.type !== "subscription.revoked" &&
          (s.status === "active" || s.status === "trialing") &&
          !s.ended_at,
        validUntil: end,
      };
      try {
        validateUpdate(update);
      } catch {
        throw new InvalidWebhook();
      }
      return update;
    },
  };
}
