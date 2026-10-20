/** Recipient import is an explicit native operation. No credentials, redirects or checkout writes. */
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { brand } from "@vellum/config";
import { type Bundle, decodeBundle, MAX_BUNDLE } from "@vellum/storage";
export interface ShareLink {
  id: string;
  origin: string;
}
export function parseShareLink(value: string): ShareLink {
  if (value.length > 4096) throw Error("Invalid share link");
  const u = new URL(value);
  if (
    u.protocol !== `${brand.desktopProtocol}:` ||
    u.hostname !== "share" ||
    u.username ||
    u.password ||
    u.port ||
    u.hash ||
    !/^\/[A-Za-z0-9_-]{24,128}$/.test(u.pathname) ||
    [...u.searchParams.keys()].join(",") !== "origin"
  )
    throw Error("Invalid share link");
  const origin = new URL(u.searchParams.get("origin") ?? "");
  if (
    origin.username ||
    origin.password ||
    origin.pathname !== "/" ||
    origin.search ||
    origin.hash ||
    (origin.protocol !== "https:" &&
      !(
        origin.protocol === "http:" && ["127.0.0.1", "localhost", "[::1]"].includes(origin.hostname)
      ))
  )
    throw Error("Invalid share origin");
  return { id: u.pathname.slice(1), origin: origin.href };
}
export async function downloadSnapshot(link: ShareLink, fetcher: typeof fetch): Promise<Bundle> {
  const r = await fetcher(new URL(`/v1/shares/${link.id}`, link.origin), {
    redirect: "error",
    cache: "no-store",
    signal: AbortSignal.timeout(30000),
  });
  if (!r.ok || !r.body) throw Error("Share unavailable");
  const chunks: Uint8Array[] = [];
  let size = 0;
  const reader = r.body.getReader();
  for (;;) {
    const chunk = await reader.read();
    if (chunk.done) break;
    size += chunk.value.length;
    if (size > MAX_BUNDLE + 3 * 1024 * 1024) {
      await reader.cancel();
      throw Error("Snapshot too large");
    }
    chunks.push(chunk.value);
  }
  const value = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  return decodeBundle(Buffer.from(JSON.stringify(value.bundle)));
}
export async function installSnapshot(parent: string, bundle: Bundle): Promise<string> {
  const checked = decodeBundle(Buffer.from(JSON.stringify(bundle)));
  const root = await mkdtemp(join(parent, "vellum-share-"));
  try {
    for (const file of checked.files) {
      const path = join(root, ...file.path.split("/"));
      await mkdir(dirname(path), { recursive: true, mode: 0o700 });
      await writeFile(path, Buffer.from(file.data, "base64"), { flag: "wx", mode: 0o600 });
    }
    await writeFile(join(root, brand.shareRootMarker), "", { flag: "wx", mode: 0o600 });
    return join(root, ...checked.entry.split("/"));
  } catch {
    await rm(root, { recursive: true, force: true });
    throw Error("Snapshot could not be saved");
  }
}
