// Downloads the text-fixture fonts listed in fonts.json into fonts/ (gitignored) and checks each SHA-256.
// Needs the network once; files already present with the right hash are kept.
//
//   node scripts/fetch-fonts.mjs [--check]     --check: verify only, never download

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const spike = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(spike, "fonts");
const { fonts } = JSON.parse(readFileSync(join(spike, "fonts.json"), "utf8"));
const checkOnly = process.argv.includes("--check");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

mkdirSync(dir, { recursive: true });
let failed = 0;
for (const font of fonts) {
  const path = join(dir, font.file);
  if (existsSync(path) && sha256(readFileSync(path)) === font.sha256) {
    console.log(`ok       ${font.file}`);
    continue;
  }
  if (checkOnly) {
    console.error(`missing  ${font.file} (run node scripts/fetch-fonts.mjs)`);
    failed++;
    continue;
  }
  const response = await fetch(font.url);
  if (!response.ok) {
    console.error(`FAILED   ${font.file}: HTTP ${response.status}`);
    failed++;
    continue;
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  const hash = sha256(bytes);
  if (hash !== font.sha256) {
    console.error(`FAILED   ${font.file}: sha256 ${hash}, expected ${font.sha256}`);
    failed++;
    continue;
  }
  writeFileSync(path, bytes);
  console.log(`fetched  ${font.file} (${bytes.length} bytes)`);
}
process.exit(failed ? 1 : 0);
