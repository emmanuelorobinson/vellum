// TEST DATA. Locally signed vendor messages, never calls Polar.
import { createHmac } from "node:crypto";
import { expect, it, vi } from "vitest";
import { InvalidWebhook, polarBilling } from "./polar";

const secret = "test-only-webhook-signing-material";
const provider = () =>
  polarBilling({
    accessToken: "test-only-unused",
    webhookSecret: secret,
    productId: "pro",
    successUrl: "https://account.example.test",
  });
const sign = (body: string) => {
  const timestamp = String(Math.floor(Date.now() / 1000)),
    id = "delivery-1";
  const signature = createHmac("sha256", secret)
    .update(`${id}.${timestamp}.${body}`)
    .digest("base64");
  return {
    "webhook-id": id,
    "webhook-timestamp": timestamp,
    "webhook-signature": `v1,${signature}`,
  };
};
it("verifies raw bytes before mapping subscription state and rejects modified/old deliveries", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-10-19T00:00:00Z"));
  try {
    const body = JSON.stringify({
      type: "subscription.updated",
      data: {
        id: "sub",
        customer: { external_id: "person" },
        product_id: "pro",
        created_at: "2026-10-01T00:00:00Z",
        modified_at: "2026-10-19T00:00:00Z",
        current_period_end: "2026-11-01T00:00:00Z",
        status: "active",
        cancel_at_period_end: true,
      },
    });
    const headers = sign(body),
      billing = provider();
    const update = await billing.parseWebhook(body, headers);
    expect(update).toMatchObject({
      eventId: "delivery-1",
      subscriptionId: "sub",
      userId: "person",
      active: true,
      productId: "pro",
      validUntil: Date.parse("2026-11-01T00:00:00Z"),
    });
    await expect(billing.parseWebhook(`${body} `, headers)).rejects.toBeInstanceOf(InvalidWebhook);
    vi.advanceTimersByTime(301000);
    await expect(billing.parseWebhook(body, headers)).rejects.toBeInstanceOf(InvalidWebhook);
  } finally {
    vi.useRealTimers();
  }
});
