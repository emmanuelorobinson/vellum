// Static server for the renderer spike page (Prompt 4). Local only: binds 127.0.0.1.
//
//   node server.mjs [port]      default port 4173; prints the URL
//
// Serves web/index.html and web/src, the build output in web/dist (scripts/build.sh), CanvasKit from
// node_modules, and /font.ttf: Geist Regular (OFL-1.1) taken from the installed `next` package, so no
// font is committed. Exports `startServer` for measure.mjs and the gpu test.

import { createReadStream, existsSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../..");

const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".ttf": "font/ttf",
  ".png": "image/png",
  ".json": "application/json",
};

export function fontPath() {
  const require = createRequire(join(repo, "apps/web/package.json"));
  const next = dirname(require.resolve("next/package.json"));
  const path = join(next, "dist/compiled/@vercel/og/Geist-Regular.ttf");
  if (!existsSync(path)) throw new Error(`font not found at ${path}`);
  return path;
}

function canvaskitDir() {
  const require = createRequire(join(here, "package.json"));
  return join(dirname(require.resolve("canvaskit-wasm/package.json")), "bin");
}

function resolveRequest(url) {
  const path = decodeURIComponent(new URL(url, "http://localhost").pathname);
  if (path === "/" || path === "/index.html") return join(here, "index.html");
  if (path === "/font.ttf") return fontPath();
  const mounts = [
    ["/src/", join(here, "src")],
    ["/dist/", join(here, "dist")],
    ["/canvaskit/", canvaskitDir()],
  ];
  for (const [prefix, root] of mounts) {
    if (path.startsWith(prefix)) {
      const file = normalize(join(root, path.slice(prefix.length)));
      return file.startsWith(root) ? file : null;
    }
  }
  return null;
}

export function startServer(port = 0) {
  const server = createServer((req, res) => {
    const file = resolveRequest(req.url ?? "/");
    if (!file || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end("not found");
      return;
    }
    res.writeHead(200, {
      "content-type": types[extname(file)] ?? "application/octet-stream",
      "cache-control": "no-store",
    });
    createReadStream(file).pipe(res);
  });
  return new Promise((ok) => {
    server.listen(port, "127.0.0.1", () =>
      ok({ server, url: `http://127.0.0.1:${server.address().port}/` }),
    );
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { url } = await startServer(Number(process.argv[2] ?? 4173));
  console.log(`renderer spike: ${url}?renderer=a  |  ${url}?renderer=b`);
}
