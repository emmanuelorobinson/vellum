/** Fixed cloud API surface. Server composition supplies the origin; clients cannot choose an upstream. */
export async function cloudProxy(
  request: Request,
  parts: string[],
  origin: string | undefined,
  publicOnly = false,
): Promise<Response> {
  const path = parts.join("/");
  const publicPath = /^shares\/[A-Za-z0-9_-]{24,128}(?:\/download)?$/.test(path);
  const accountPath =
    /^(?:me|entitlements|sessions(?:\/[A-Za-z0-9_-]{1,256})?|shares(?:\/[A-Za-z0-9_-]{24,128})?|billing\/(?:checkout|portal))$/.test(
      path,
    );
  const uploadPath =
    /^shares\/[A-Za-z0-9_-]{24,128}\/(?:status|uploads\/[A-Za-z0-9_-]{24,128}\/(?:parts\/[0-9]{1,4}|publish))$/.test(
      path,
    );
  if (
    parts.some((p) => !p || p.includes(".") || p.includes("/") || p.includes("\\")) ||
    !(publicOnly ? publicPath && request.method === "GET" : accountPath || publicPath || uploadPath)
  )
    return new Response(null, { status: 404 });
  try {
    if (!origin) throw Error();
    const base = new URL(origin);
    if (
      base.username ||
      base.password ||
      base.pathname !== "/" ||
      base.search ||
      base.hash ||
      (base.protocol !== "https:" &&
        !(base.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(base.hostname)))
    )
      throw Error();
    const headers = new Headers();
    if (!publicOnly) {
      for (const h of ["cookie", "authorization", "origin", "content-type"]) {
        const v = request.headers.get(h);
        if (v) headers.set(h, v);
      }
    }
    let body: Uint8Array<ArrayBuffer> | undefined;
    if (!["GET", "HEAD"].includes(request.method) && request.body) {
      const chunks: Uint8Array[] = [],
        reader = request.body.getReader();
      let size = 0;
      const limit = path.includes("/parts/") ? 512 * 1024 : 64 * 1024;
      for (;;) {
        const chunk = await reader.read();
        if (chunk.done) break;
        size += chunk.value.length;
        if (size > limit) {
          await reader.cancel();
          return new Response(null, { status: 413 });
        }
        chunks.push(chunk.value);
      }
      body = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) {
        body.set(chunk, offset);
        offset += chunk.length;
      }
    }
    const upstream = await fetch(new URL(`/v1/${path}`, base), {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(30000),
    });
    const out = new Headers({
      "cache-control": "no-store",
      "referrer-policy": "no-referrer",
      "x-content-type-options": "nosniff",
      "x-robots-tag": "noindex, nofollow, noarchive",
    });
    for (const h of ["content-type", "content-disposition"]) {
      const value = upstream.headers.get(h);
      if (value) out.set(h, value);
    }
    return new Response(upstream.body, { status: upstream.status, headers: out });
  } catch {
    return Response.json(
      { error: "Cloud service unavailable" },
      { status: 503, headers: { "cache-control": "no-store" } },
    );
  }
}
