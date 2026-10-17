#!/usr/bin/env node
// Design tokens from the read-only design copy (guide Prompt 43). Exports every variable (name, type,
// value per appearance) and the colour pairs the design uses to packages/ui-tokens/source/tokens.json with
// the design file's sha256, then generates the CSS variables and the TS theme object from it. It also
// exports the Tokens sheet ("Foundations · Tokens · all appearances") as the snapshot the Storybook token
// page is compared with.
//
//   scripts/design-tokens [--root <dir>]                   drive `pen interactive` (needs pen login or PEN_CLI_KEY)
//   scripts/design-tokens --print-query                    print the read-only query, to run in an MCP session
//   scripts/design-tokens --print-export --staging <dir>   print the Export call for the Tokens sheet
//   scripts/design-tokens --raw <file> [--staging <dir>]   build everything from a query result (and exports)
//                                                          made elsewhere (<dir>/1x/<id>.png, <dir>/2x/<id>.png)
//   scripts/design-tokens --generate                       regenerate CSS and TS from the committed tokens.json
//   scripts/design-tokens --check                          CI: tokens.json matches the inventory and vellum.pen,
//                                                          generated files are current, DESIGN.md gaps are recorded
//
// Nothing here writes to the design: the query uses Get, GetVariables and Print, the export uses Export.
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildTokens,
  checkGaps,
  generateCss,
  generateTs,
  serializeTokens,
} from "./lib/design-tokens.mjs";
import { penInteractive } from "./lib/pen-cli.mjs";
import {
  executeLine,
  exportSnippet,
  parseTokensRaw,
  TOKENS_QUERY,
  tokensFrame,
  verifyExecuteApi,
} from "./lib/pen-queries.mjs";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name) => (flag(name) ? args[args.indexOf(name) + 1] : undefined);
const root = option("--root") ?? join(dirname(fileURLToPath(import.meta.url)), "..");
const at = (...p) => join(root, ...p);

const die = (message) => {
  console.error(message);
  process.exit(1);
};
const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

const PATHS = {
  tokens: "packages/ui-tokens/source/tokens.json",
  map: "packages/ui-tokens/source/design-md.json",
  css: "packages/ui-tokens/src/generated/tokens.css",
  ts: "packages/ui-tokens/src/generated/tokens.ts",
  snapshot1x: "docs/design/snapshots/foundations/Tokens.png",
  snapshot2x: "docs/design/snapshots/foundations/Tokens@2x.png",
};

if (flag("--print-query")) {
  console.log(TOKENS_QUERY);
  process.exit(0);
}

const inventoryPath = at("docs/design/snapshots/inventory.json");
if (!existsSync(inventoryPath))
  die("docs/design/snapshots/inventory.json is missing. Run pnpm design:snapshot first.");
const inventory = JSON.parse(readFileSync(inventoryPath, "utf8"));
const sheet = tokensFrame(inventory);
if (!sheet) die('inventory.json has no "Foundations · Tokens · …" frame.');

if (flag("--print-export")) {
  const staging = option("--staging") ?? die("--print-export needs --staging <dir>");
  for (const [scale, dir] of [
    [1, "1x"],
    [2, "2x"],
  ])
    console.log(exportSnippet({ ids: [sheet.frame_id], scale, dir: join(staging, dir) }));
  process.exit(0);
}

const gapFindings = (tokens) =>
  checkGaps({
    tokens,
    designMd: readFileSync(at("docs/design/DESIGN.md"), "utf8"),
    map: JSON.parse(readFileSync(at(PATHS.map), "utf8")),
    gapsText: existsSync(at("docs/design/DESIGN-GAPS.md"))
      ? readFileSync(at("docs/design/DESIGN-GAPS.md"), "utf8")
      : "",
  });

function report({ errors, warnings }) {
  for (const w of warnings) console.log(`! ${w.message}`);
  for (const e of errors) console.error(`✗ ${e.message}`);
}

function writeGenerated(tokens) {
  mkdirSync(dirname(at(PATHS.css)), { recursive: true });
  writeFileSync(at(PATHS.css), generateCss(tokens));
  writeFileSync(at(PATHS.ts), generateTs(tokens));
}

if (flag("--check")) {
  const errors = [];
  if (!existsSync(at(PATHS.tokens))) die(`✗ ${PATHS.tokens} is missing. Run pnpm design:tokens.`);
  const tokens = JSON.parse(readFileSync(at(PATHS.tokens), "utf8"));
  if (tokens.source_sha256 !== inventory.source_sha256)
    errors.push(
      `${PATHS.tokens} was made from vellum.pen ${tokens.source_sha256.slice(0, 12)}…, the inventory from ${inventory.source_sha256.slice(0, 12)}…: tokens.json is stale. Run pnpm design:tokens.`,
    );
  const pen = at("docs/design/vellum.pen");
  if (existsSync(pen) && sha256(pen) !== tokens.source_sha256)
    errors.push(`${PATHS.tokens} does not match docs/design/vellum.pen. Run pnpm design:tokens.`);
  const names = tokens.tokens.map((t) => t.name);
  if (JSON.stringify(names) !== JSON.stringify(inventory.variables.names))
    errors.push(
      `${PATHS.tokens} and inventory.json list different variables. Run pnpm design:tokens.`,
    );
  for (const [path, expected] of [
    [PATHS.css, generateCss(tokens)],
    [PATHS.ts, generateTs(tokens)],
  ])
    if (!existsSync(at(path)) || readFileSync(at(path), "utf8") !== expected)
      errors.push(
        `${path} is not generated from ${PATHS.tokens}. Run node scripts/design-tokens.mjs --generate.`,
      );
  const gaps = gapFindings(tokens);
  report(gaps);
  for (const e of errors) console.error(`✗ ${e}`);
  if (errors.length || gaps.errors.length) process.exit(1);
  console.log(
    `✓ design tokens current (${names.length} variables, ${gaps.warnings.length} recorded gaps)`,
  );
  process.exit(0);
}

if (flag("--generate")) {
  writeGenerated(JSON.parse(readFileSync(at(PATHS.tokens), "utf8")));
  console.log(`wrote ${PATHS.css} and ${PATHS.ts}`);
  process.exit(0);
}

// From here on the design is read: the copy must be the current specification.
const source = at("docs/design/vellum.pen");
const cache = at(".design-cache");
const copy = join(cache, "vellum.readonly.pen");
if (!existsSync(source)) die("docs/design/vellum.pen is missing.");
if (!existsSync(copy) || sha256(copy) !== sha256(source))
  die(
    "The read-only copy is missing or older than docs/design/vellum.pen. Run scripts/design-open first.",
  );
const sourceSha256 = sha256(source);
if (sourceSha256 !== inventory.source_sha256)
  die("inventory.json is older than docs/design/vellum.pen. Run pnpm design:snapshot first.");

function finalize(rawText, staging, exporter) {
  let tokens;
  try {
    tokens = buildTokens({
      raw: parseTokensRaw(rawText),
      order: inventory.variables.names,
      sourceSha256,
      exporter,
    });
  } catch (error) {
    die(error.message);
  }
  mkdirSync(dirname(at(PATHS.tokens)), { recursive: true });
  writeFileSync(at(PATHS.tokens), serializeTokens(tokens));
  writeGenerated(tokens);
  console.log(
    `wrote ${PATHS.tokens} (${tokens.tokens.length} variables, ${tokens.pairs.length} pairs), ${PATHS.css}, ${PATHS.ts}`,
  );
  if (staging) {
    const exported = [
      [join(staging, "1x", `${sheet.frame_id}.png`), PATHS.snapshot1x],
      [join(staging, "2x", `${sheet.frame_id}.png`), PATHS.snapshot2x],
    ];
    const missing = exported.filter(([from]) => !existsSync(from)).map(([from]) => from);
    if (missing.length) die(`missing export: ${missing.join(", ")} (${sheet.name})`);
    mkdirSync(dirname(at(PATHS.snapshot1x)), { recursive: true });
    for (const [from, to] of exported) copyFileSync(from, at(to));
    console.log(`wrote ${PATHS.snapshot1x} and ${PATHS.snapshot2x}`);
  }
  const gaps = gapFindings(tokens);
  report(gaps);
  if (gaps.errors.length)
    die(
      "DESIGN.md and vellum.pen differ in ways DESIGN-GAPS.md does not record (above). A person resolves each one.",
    );
}

if (flag("--raw")) {
  finalize(
    readFileSync(option("--raw"), "utf8"),
    option("--staging"),
    option("--exporter") ?? "pen.dev MCP (execute)",
  );
} else {
  const run = (lines) => {
    try {
      return penInteractive({ copy, cache, lines });
    } catch (error) {
      die(error.message);
    }
  };
  // read_skill first: the execute syntax is never assumed (guide §2.1 rule 2).
  try {
    verifyExecuteApi(run(['read_skill({ path: "execute.md" })']));
  } catch (error) {
    die(error.message);
  }
  const rawText = run([executeLine(TOKENS_QUERY)]);
  const staging = join(cache, "tokens-staging");
  rmSync(staging, { recursive: true, force: true });
  run(
    [
      [1, "1x"],
      [2, "2x"],
    ].map(([scale, dir]) =>
      executeLine(exportSnippet({ ids: [sheet.frame_id], scale, dir: join(staging, dir) })),
    ),
  );
  finalize(rawText, staging, "pen.dev CLI (pen interactive)");
}
