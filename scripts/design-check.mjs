#!/usr/bin/env node
// CI check for the design source. Needs no pen.dev: it reads the committed inventory and snapshots.
//
//   scripts/design-check [--root <dir>]
//
// Fails when docs/design/vellum.pen changed since the snapshots were taken, when a frame DESIGN.md
// "Deliverables" asks for is missing, or when frame, component or variable names break the convention.
// Rules: scripts/lib/design-check.mjs. Known differences between the design file and the guide's
// convention are recorded in docs/design/DESIGN-GAPS.md.
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { checkDesign } from "./lib/design-check.mjs";

const args = process.argv.slice(2);
const rootIndex = args.indexOf("--root");
const root =
  rootIndex === -1 ? join(dirname(fileURLToPath(import.meta.url)), "..") : args[rootIndex + 1];
const at = (...p) => join(root, ...p);
const fail = (lines) => {
  for (const line of lines) console.error(line);
  process.exit(1);
};

const pen = at("docs/design/vellum.pen");
const inventoryPath = at("docs/design/snapshots/inventory.json");
if (!existsSync(pen)) fail(["✗ docs/design/vellum.pen is missing"]);
if (!existsSync(inventoryPath)) {
  fail([
    "✗ docs/design/snapshots/inventory.json is missing.",
    "  UI prompts must not start without at least the snapshots. Run pnpm design:snapshot (needs pen login or PEN_CLI_KEY).",
  ]);
}

const aliasPath = at("scripts/design-component-aliases.json");
const optional = (path) => (existsSync(path) ? readFileSync(path, "utf8") : "");
const { errors, warnings } = checkDesign({
  inventory: JSON.parse(readFileSync(inventoryPath, "utf8")),
  designMd: readFileSync(at("docs/design/DESIGN.md"), "utf8"),
  sourceSha256: createHash("sha256").update(readFileSync(pen)).digest("hex"),
  gapsText: optional(at("docs/design/DESIGN-GAPS.md")),
  familyAliases:
    aliasPath && existsSync(aliasPath) ? JSON.parse(readFileSync(aliasPath, "utf8")) : {},
  snapshotExists: (file) => existsSync(at("docs/design/snapshots", file)),
});

for (const w of warnings) console.log(`! ${w.message}`);
if (errors.length) fail(errors.map((e) => `✗ ${e.message}`));
console.log(`✓ design check passed (${warnings.length} recorded gaps or v1.1 frames)`);
