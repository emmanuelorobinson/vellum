import { expect, it, vi } from "vitest";
import { createSesMailer } from "./index";

it("sends plain UTF-8 mail through SES without exposing vendor errors", async () => {
  const send = vi.fn().mockResolvedValue({});
  const mail = createSesMailer({ send }, "sender@example.test");
  await mail.send({ to: "person@example.test", subject: "Sign in", text: "Follow your link" });
  expect(send.mock.calls[0]?.[0].input).toMatchObject({
    Destination: { ToAddresses: ["person@example.test"] },
    Content: { Simple: { Body: { Text: { Charset: "UTF-8" } } } },
  });
  send.mockRejectedValue(new Error("private vendor error"));
  await expect(
    mail.send({ to: "person@example.test", subject: "Sign in", text: "link" }),
  ).rejects.toThrow("Mail unavailable");
});
