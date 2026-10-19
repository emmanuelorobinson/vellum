import { accountAuth } from "../../../../lib/auth";
export const runtime = "nodejs";
export const dynamic = "force-dynamic";
async function handler(request: Request) {
  try {
    return await accountAuth().handler(request);
  } catch {
    return Response.json(
      { error: "Account service unavailable" },
      { status: 503, headers: { "cache-control": "no-store" } },
    );
  }
}

export { handler as GET, handler as POST };
