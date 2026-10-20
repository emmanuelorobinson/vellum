import { cloudProxy } from "@vellum/host-browser/cloud-proxy";
export const dynamic = "force-dynamic";
async function handler(request: Request, context: { params: Promise<{ path: string[] }> }) {
  return cloudProxy(request, (await context.params).path, process.env.VELLUM_API_URL, false);
}

export { handler as GET, handler as PUT, handler as POST, handler as DELETE };
