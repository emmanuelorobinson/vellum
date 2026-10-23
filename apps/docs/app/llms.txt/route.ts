import { documentationIndex } from "../../.generated/agent-docs.ts";

export const dynamic = "force-static";

// The index for agents: one line per page, generated from the content on every docs:prepare run.
export function GET() {
  return new Response(documentationIndex, {
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "X-Content-Type-Options": "nosniff",
    },
  });
}
