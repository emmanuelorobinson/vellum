// Chrome layout oracle generator (guide Prompt 6, D2, Invariant 7; ADR-002). See usage below.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { launchChrome } from "./lib/chrome.mjs";
import {
  compareFixtures,
  expectedFixture,
  measureInPage,
  parseFixtureHtml,
  renderCssHtml,
  resolveTree,
  stableJson,
} from "./lib/layout-oracle.mjs";

const USAGE = `usage: pnpm layout-oracle [--check] [--out DIR] [--only NAME]...

Lays out every fixture in fixtures/layout-oracle/*.html in headless Chrome (Vellum's sizing semantics
mapped to CSS, ADR-002) and writes the boxes Chrome computes to fixtures/layout-oracle/expected/.

  --check      compare with the committed expected files instead of writing them; exit 1 on any
               difference over 0.5 px, any changed props or ids, and any missing or stale file
  --out DIR    write the expected files to DIR (with --check: the regenerated files, for inspection)
  --only NAME  only this fixture (repeatable); the manifest is left alone

Needs Chrome: VELLUM_CHROME, else the platform's usual install path. Opens no network port.`;

const TOLERANCE_PX = 0.5;
const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixturesDir = join(repo, "fixtures", "layout-oracle");
const expectedDir = join(fixturesDir, "expected");

function parseArgs(argv) {
  const opts = { check: false, out: null, only: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--help" || a === "-h") opts.help = true;
    else if (a === "--check") opts.check = true;
    else if (a === "--out" && argv[i + 1]) opts.out = argv[++i];
    else if (a === "--only" && argv[i + 1]) opts.only.push(argv[++i]);
    else throw new Error(`unknown option ${a}\n\n${USAGE}`);
  }
  return opts;
}

async function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (e) {
    console.error(e.message);
    return 2;
  }
  if (opts.help) {
    console.log(USAGE);
    return 0;
  }

  const all = readdirSync(fixturesDir)
    .filter((f) => f.endsWith(".html"))
    .map((f) => f.slice(0, -5))
    .sort();
  const unknown = opts.only.filter((n) => !all.includes(n));
  if (unknown.length) {
    console.error(`no such fixture: ${unknown.join(", ")}`);
    return 2;
  }
  const names = opts.only.length ? opts.only : all;
  // Parse everything before starting Chrome, so a bad fixture fails fast.
  const resolved = names.map((name) => {
    try {
      return resolveTree(parseFixtureHtml(readFileSync(join(fixturesDir, `${name}.html`), "utf8")));
    } catch (e) {
      throw new Error(`${name}.html: ${e.message}`);
    }
  });

  const chrome = await launchChrome();
  const generated = [];
  try {
    for (const [i, name] of names.entries()) {
      await chrome.setContent(renderCssHtml(resolved[i]));
      const boxes = await chrome.evaluate(`(${measureInPage})()`);
      generated.push(expectedFixture(name, resolved[i], boxes));
    }
  } finally {
    await chrome.close();
  }
  const manifest = { chrome: chrome.version, fixtures: all, tolerancePx: TOLERANCE_PX };

  const outDir = opts.out ?? (opts.check ? null : expectedDir);
  if (outDir) {
    mkdirSync(outDir, { recursive: true });
    for (const fx of generated) writeFileSync(join(outDir, `${fx.fixture}.json`), stableJson(fx));
    if (!opts.only.length) writeFileSync(join(outDir, "manifest.json"), stableJson(manifest));
  }

  if (!opts.check) {
    console.log(
      `wrote ${generated.length} expected files from Chrome ${chrome.version} to ${outDir}`,
    );
    return 0;
  }

  const problems = [];
  for (const fx of generated) {
    const file = join(expectedDir, `${fx.fixture}.json`);
    if (!existsSync(file)) {
      problems.push(`${fx.fixture}: no committed expected file`);
      continue;
    }
    problems.push(...compareFixtures(JSON.parse(readFileSync(file, "utf8")), fx, TOLERANCE_PX));
  }
  if (!opts.only.length) {
    const committed = readdirSync(expectedDir)
      .filter((f) => f.endsWith(".json") && f !== "manifest.json")
      .map((f) => f.slice(0, -5));
    for (const stale of committed.filter((n) => !all.includes(n))) {
      problems.push(`${stale}: expected file without a fixture`);
    }
    const committedManifest = JSON.parse(readFileSync(join(expectedDir, "manifest.json"), "utf8"));
    if (stableJson(committedManifest.fixtures) !== stableJson(all)) {
      problems.push("manifest.json lists different fixtures");
    }
    if (committedManifest.chrome !== chrome.version) {
      console.log(
        `note: committed boxes come from Chrome ${committedManifest.chrome}; this run used ${chrome.version}`,
      );
    }
  }
  if (problems.length) {
    console.error(`${problems.length} difference(s) from the committed oracle:`);
    for (const p of problems) console.error(`  ${p}`);
    console.error("If the change is intended, run pnpm layout-oracle and commit the result.");
    return 1;
  }
  console.log(
    `${generated.length} fixtures match the committed oracle within ${TOLERANCE_PX} px (Chrome ${chrome.version})`,
  );
  return 0;
}

main().then(
  (code) => process.exit(code),
  (e) => {
    console.error(e.message);
    process.exit(1);
  },
);
