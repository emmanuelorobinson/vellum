#!/usr/bin/env node
// Token lints (guide Prompt 43), run by `pnpm lint`:
//   contrast  every text/background pair ≥ 4.5:1 (11–12 px text included), icons and focus ≥ 3:1, in every
//             appearance: the pairs vellum.pen uses plus packages/ui-tokens/source/contrast-pairs.json
//   usage     no colour literals in UI packages; selection blue and component violet never on buttons;
//             agent hues only on agent markers, cursors, leases, placeholders and session dots
//
//   node scripts/token-lint.mjs [--root <dir>] [--verbose]
// Rules: scripts/lib/design-tokens.mjs (checkContrast) and scripts/lib/token-usage.mjs.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { checkContrast } from "./lib/design-tokens.mjs";
import { checkUsage, isLinted, UI_ROOTS } from "./lib/token-usage.mjs";

const args = process.argv.slice(2);
const root = args.includes("--root")
  ? args[args.indexOf("--root") + 1]
  : join(dirname(fileURLToPath(import.meta.url)), "..");
const at = (...p) => join(root, ...p);

const tokensPath = at("packages/ui-tokens/source/tokens.json");
if (!existsSync(tokensPath)) {
  console.error("✗ packages/ui-tokens/source/tokens.json is missing. Run pnpm design:tokens.");
  process.exit(1);
}
const tokens = JSON.parse(readFileSync(tokensPath, "utf8"));
const declared = JSON.parse(
  readFileSync(at("packages/ui-tokens/source/contrast-pairs.json"), "utf8"),
);

const contrast = checkContrast({ tokens, declared });

function walk(dir, out) {
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else {
      const rel = relative(root, path).split("\\").join("/");
      if (isLinted(rel)) out.push({ path: rel, text: readFileSync(path, "utf8") });
    }
  }
  return out;
}
const files = UI_ROOTS.flatMap((r) => walk(at(r), []));
const hues = tokens.tokens
  .filter((t) => t.group === "agent")
  .map((t) => t.name.slice("agent/".length));
const usage = checkUsage({ files, hues });

if (args.includes("--verbose"))
  for (const c of contrast.checked)
    console.log(
      `  ${c.kind} ${c.foreground} on ${c.background} (${c.mode}) ${c.ratio.toFixed(2)}:1`,
    );
for (const e of contrast.errors) console.error(`✗ contrast: ${e.message}`);
for (const e of usage.errors) console.error(`✗ ${e.path}:${e.line} ${e.rule}: ${e.message}`);
if (contrast.errors.length || usage.errors.length) process.exit(1);
console.log(
  `✓ token lint: ${contrast.checked.length} contrast checks (${contrast.skipped.length} pairs on unbound colours skipped), ${files.length} UI files`,
);
