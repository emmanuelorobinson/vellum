// Release signing input comes from stdin (OS keychain or CI secret injection), never a saved key file.
import { createHash, createPrivateKey, sign } from "node:crypto";
import { createReadStream } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import { basename } from "node:path";

const args = process.argv.slice(2);
const option = (key) => (args.includes(key) ? args[args.indexOf(key) + 1] : undefined);
const version = option("--version"),
  platform = option("--platform"),
  arch = option("--arch"),
  output = option("--out");
const paths = args.flatMap((a, i) => (a === "--artifact" && args[i + 1] ? [args[i + 1]] : []));
if (
  !version ||
  !/^\d+\.\d+\.\d+$/.test(version) ||
  !["darwin", "win32", "linux"].includes(platform) ||
  !["x64", "arm64"].includes(arch) ||
  !output ||
  !paths.length ||
  paths.length > 8
)
  throw Error(
    "Use --version, --platform, --arch, --out and one or more --artifact paths; supply the Ed25519 private key on stdin",
  );
let pem = Buffer.alloc(0);
try {
  for await (const chunk of process.stdin) {
    if (pem.length + chunk.length > 16384) throw Error("signing key input is too large");
    pem = Buffer.concat([pem, chunk]);
  }
  const key = createPrivateKey(pem);
  if (key.asymmetricKeyType !== "ed25519") throw Error("Ed25519 signing key required");
  const files = [];
  for (const path of paths) {
    const url = basename(path);
    if (!/^[a-zA-Z0-9_.-]+$/.test(url) || url.startsWith(".")) throw Error("Invalid artifact name");
    const hash = createHash("sha512");
    for await (const bytes of createReadStream(path)) hash.update(bytes);
    files.push({ url, sha512: hash.digest("base64"), size: (await stat(path)).size });
  }
  const payload = Buffer.from(JSON.stringify({ version, platform, arch, files }));
  const signature = sign(null, payload, key).toString("base64");
  await writeFile(output, JSON.stringify({ payload: payload.toString("base64"), signature }), {
    flag: "wx",
    mode: 0o644,
  });
} finally {
  pem.fill(0);
}
