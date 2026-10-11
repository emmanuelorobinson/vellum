#!/usr/bin/env node
import { spawnSync } from "node:child_process";
// Regenerates docs/design/snapshots/ (PNG at 1x and 2x for every screen frame, plus inventory.json) from
// the read-only copy of the design, using the pen.dev CLI in headless interactive mode.
//
//   scripts/design-snapshot [--root <dir>]                 drive `pen interactive` (needs pen login or PEN_CLI_KEY)
//   scripts/design-snapshot --print-query                  print the read-only query, to run in an MCP session
//   scripts/design-snapshot --raw <file> --staging <dir>   build the output from a query result and exports
//                                                          made elsewhere (<dir>/1x/<id>.png, <dir>/2x/<id>.png)
//   scripts/design-snapshot --raw <file> --staging <dir> --print-export
//                                                          print the two Export calls for those frame ids
//
// VELLUM_PEN_CLI overrides the pen command (default: `pen` on PATH, else the pinned npx package).
// The document is only read: nothing here calls save(), and the CLI's output file is never written.
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildInventory,
  executeLine,
  exportSnippet,
  INVENTORY_QUERY,
  parseRaw,
  verifyExecuteApi,
} from "./lib/pen-queries.mjs";

const PINNED_CLI = "npx --yes @pen.dev/cli@0.3.11"; // docs/architecture/version-baseline.md C9
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name) => (flag(name) ? args[args.indexOf(name) + 1] : undefined);
const root = option("--root") ?? join(dirname(fileURLToPath(import.meta.url)), "..");

const die = (message) => {
  console.error(message);
  process.exit(1);
};
const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const quote = (s) => `'${s.replaceAll("'", `'\\''`)}'`;

if (flag("--print-query")) {
  console.log(INVENTORY_QUERY);
  process.exit(0);
}

const source = join(root, "docs/design/vellum.pen");
const cache = join(root, ".design-cache");
const copy = join(cache, "vellum.readonly.pen");
const outDir = join(root, "docs/design/snapshots");

if (!existsSync(source)) die("docs/design/vellum.pen is missing.");
if (!existsSync(copy) || sha256(copy) !== sha256(source))
  die(
    "The read-only copy is missing or older than docs/design/vellum.pen. Run scripts/design-open first.",
  );
const sourceSha256 = sha256(source);

function penCommand() {
  // biome-ignore lint/suspicious/noUndeclaredEnvVars: a developer override read at run time, not a Turborepo task input
  const override = process.env.VELLUM_PEN_CLI;
  if (override) return override;
  const onPath = spawnSync("sh", ["-c", "command -v pen"], { encoding: "utf8" });
  return onPath.status === 0 ? "pen" : PINNED_CLI;
}

// One headless `pen interactive` session. The output file is a scratch path that is never saved to.
function interactive(lines) {
  const cmd = `${penCommand()} interactive -i ${quote(copy)} -o ${quote(join(cache, "headless-out.pen"))}`;
  const result = spawnSync(cmd, {
    shell: true,
    input: `${[...lines, "exit()"].join("\n")}\n`,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  if (/Authentication required/i.test(output)) {
    die('The pen CLI is not signed in. Run "pen login" (or set PEN_CLI_KEY), then run this again.');
  }
  if (result.status !== 0)
    die(`pen interactive failed (exit ${result.status}):\n${output.slice(-2000)}`);
  return output;
}

function finalize(rawText, staging, exporter) {
  const inventory = buildInventory({ raw: parseRaw(rawText), sourceSha256, exporter });
  const screens = inventory.frames.filter((f) => f.snapshots);
  const missing = [];
  for (const f of screens)
    for (const scale of ["1x", "2x"]) {
      const file = join(staging, scale, `${f.frame_id}.png`);
      if (!existsSync(file)) missing.push(`missing export: ${file} (${f.name})`);
    }
  if (missing.length) die(missing.join("\n"));

  mkdirSync(outDir, { recursive: true });
  for (const name of readdirSync(outDir)) if (name.endsWith(".png")) rmSync(join(outDir, name));
  for (const f of screens)
    for (const scale of ["1x", "2x"])
      copyFileSync(join(staging, scale, `${f.frame_id}.png`), join(outDir, f.snapshots[scale]));
  writeFileSync(join(outDir, "inventory.json"), `${JSON.stringify(inventory, null, 2)}\n`);
  console.log(
    `wrote ${screens.length * 2} snapshots and inventory.json (${inventory.frames.length} frames) to docs/design/snapshots/`,
  );
}

if (flag("--raw")) {
  const rawText = readFileSync(option("--raw"), "utf8");
  const staging = option("--staging") ?? die("--raw needs --staging <dir>");
  if (flag("--print-export")) {
    const ids = buildInventory({ raw: parseRaw(rawText), sourceSha256, exporter: "" })
      .frames.filter((f) => f.snapshots)
      .map((f) => f.frame_id);
    for (const [scale, dir] of [
      [1, "1x"],
      [2, "2x"],
    ])
      console.log(exportSnippet({ ids, scale, dir: join(staging, dir) }));
    process.exit(0);
  }
  finalize(rawText, staging, option("--exporter") ?? "pen.dev MCP (execute)");
} else {
  // read_skill first: the execute syntax is never assumed (guide §2.1 rule 2).
  const skill = interactive(['read_skill({ path: "execute.md" })']);
  try {
    verifyExecuteApi(skill);
  } catch (error) {
    die(error.message);
  }
  const rawText = interactive([executeLine(INVENTORY_QUERY)]);
  let parsed;
  try {
    parsed = parseRaw(rawText);
  } catch (error) {
    die(`${error.message}\n${rawText.slice(-1500)}`);
  }
  const ids = buildInventory({ raw: parsed, sourceSha256, exporter: "" })
    .frames.filter((f) => f.snapshots)
    .map((f) => f.frame_id);
  const staging = join(cache, "snapshot-staging");
  rmSync(staging, { recursive: true, force: true });
  interactive(
    [
      [1, "1x"],
      [2, "2x"],
    ].map(([scale, dir]) => executeLine(exportSnippet({ ids, scale, dir: join(staging, dir) }))),
  );
  finalize(rawText, staging, "pen.dev CLI (pen interactive)");
}
