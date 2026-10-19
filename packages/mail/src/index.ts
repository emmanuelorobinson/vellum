import { SESv2Client, SendEmailCommand } from "@aws-sdk/client-sesv2";
export const PACKAGE_NAME = "@vellum/mail";
export interface Mailer {
  send(message: { to: string; subject: string; text: string }): Promise<void>;
}
export function createSesMailer(client: Pick<SESv2Client, "send">, from: string): Mailer {
  return {
    async send(message) {
      if (
        !message.to.includes("@") ||
        /[\r\n]/.test(message.to + from + message.subject) ||
        message.text.length > 100_000
      )
        throw new Error("Invalid mail");
      try {
        await client.send(
          new SendEmailCommand({
            FromEmailAddress: from,
            Destination: { ToAddresses: [message.to] },
            Content: {
              Simple: {
                Subject: { Data: message.subject, Charset: "UTF-8" },
                Body: { Text: { Data: message.text, Charset: "UTF-8" } },
              },
            },
          }),
        );
      } catch {
        throw new Error("Mail unavailable");
      }
    },
  };
}
export function sesMailer(region: string, from: string): Mailer {
  return createSesMailer(new SESv2Client({ region }), from);
}
