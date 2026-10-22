// Ties the design tooling to the files it depends on: the rules design-check hard-codes must still be
// what DESIGN.md says, the committed inventory must pass, and the two design files must have their shape.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import {
  componentFamilies,
  HIGH_CONTRAST_IDS,
  NARROW_IDS,
  parseGaps,
  requiredFrames,
  WIDE_IDS,
} from "../lib/design-check.mjs";
import { at, bash, read } from "./helpers.mjs";

const designMd = () => read("docs", "design", "DESIGN.md");

test("DESIGN.md still asks for the high-contrast, 1512 and S13 widths that design-check hard-codes", () => {
  const md = designMd();
  assert.match(md, /C03, C05 and C10 also in high contrast/);
  assert.match(md, /plus C03 and C05 at 1512×982 and S13 at 960×640 and 760×640/);
  assert.deepEqual(HIGH_CONTRAST_IDS, ["C03", "C05", "C10"]);
  assert.deepEqual(WIDE_IDS, ["C03", "C05"]);
  assert.deepEqual(NARROW_IDS, { S13: [960, 760] });
});

test("DESIGN.md numbers 38 editor screens, 6 desktop, 3 web and 16 edge states", () => {
  const ids = new Set(requiredFrames(designMd()).map((r) => r.id));
  const count = (letter) => [...ids].filter((id) => id.startsWith(letter)).length;
  assert.deepEqual([count("C"), count("A"), count("W"), count("S")], [38, 6, 3, 16]);
});

test("only the v1.1 screens are optional", () => {
  const optional = new Set(
    requiredFrames(designMd())
      .filter((r) => r.optional)
      .map((r) => r.id),
  );
  assert.deepEqual([...optional].sort(), ["C31", "C32"]);
});

test("every Component library entry is found, aliased or recorded as a gap", () => {
  const aliases = JSON.parse(read("scripts", "design-component-aliases.json"));
  const families = componentFamilies(designMd());
  assert.ok(families.length >= 40, `expected the full Component library, got ${families.length}`);
  for (const alias of Object.keys(aliases))
    assert.ok(
      families.includes(alias),
      `design-component-aliases.json names "${alias}", which DESIGN.md does not list`,
    );
});

test("the committed inventory passes design-check and covers the whole design file", () => {
  const result = bash("design-check");
  assert.equal(result.status, 0, result.output);
  assert.match(result.stdout, /design check passed/);
  const inv = JSON.parse(read("docs", "design", "snapshots", "inventory.json"));
  assert.equal(inv.version, 1);
  assert.ok(inv.frames.length > 300);
  const files = new Set(readdirSync(at("docs", "design", "snapshots")));
  for (const f of inv.frames.filter((x) => x.snapshots))
    for (const file of Object.values(f.snapshots)) assert.ok(files.has(file), `${file} is missing`);
});

test("snapshots are PNG files", () => {
  const dir = at("docs", "design", "snapshots");
  const png = readdirSync(dir).filter((f) => f.endsWith(".png"));
  assert.ok(png.length > 0);
  for (const name of png.slice(0, 10)) {
    const head = readFileSync(at("docs", "design", "snapshots", name)).subarray(0, 8);
    assert.deepEqual(
      [...head],
      [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
      `${name} is not a PNG`,
    );
  }
});

test("DESIGN-DECISIONS.md is Appendix G, each choice UNDECIDED or one of its options with a date and who decided", () => {
  const sections = read("docs", "design", "DESIGN-DECISIONS.md").split(
    "\n## Approved v1.1 choices\n",
  );
  assert.equal(sections.length, 2);
  const [text, v11] = sections;
  const guide = read("docs", "implementation", "vellum-implementation-guide.md");
  const appendix = guide.slice(guide.indexOf("# Appendix G"));
  const open = appendix.indexOf("```markdown") + "```markdown\n".length;
  const template = appendix.slice(open, appendix.indexOf("\n```", open));
  // Only the Choice, Date and By cells may differ from the template, and only a person fills them in.
  const isRow = (l) => l.startsWith("| ") && !l.startsWith("| Decision") && !l.startsWith("|---");
  const cells = (l) =>
    l
      .slice(1, -1)
      .split("|")
      .map((c) => c.trim());
  const withoutChoices = (t) =>
    t
      .trim()
      .split("\n")
      .map((l) => (isRow(l) ? cells(l).slice(0, 2).join(" | ") : l))
      .join("\n");
  assert.equal(
    withoutChoices(text),
    withoutChoices(template),
    "DESIGN-DECISIONS.md differs from the Appendix G template",
  );
  const rows = text.split("\n").filter(isRow).map(cells);
  assert.equal(rows.length, 10);
  const added = v11.split("\n").filter(isRow).map(cells);
  assert.deepEqual(
    added.map(([decision]) => decision),
    [
      "Comments entry and placement",
      "Comment authorship and resolution",
      "Live collaboration and snapshot scope",
      "Multiplayer actor marks and hues",
      "Stale presence and following",
      "Role loss and room reset recovery",
      "Sharing management",
    ],
  );
  assert.match(v11, /owner approved the supplied Prompt 64 handoff defaults in chat/i);
  for (const [decision, options, choice, date, by] of [...rows, ...added]) {
    if (choice === "UNDECIDED") {
      assert.deepEqual([date, by], ["", ""], decision);
      continue;
    }
    assert.ok(
      options.split(" · ").includes(choice),
      `${decision}: "${choice}" is not one of ${options}`,
    );
    assert.match(date, /^\d{4}-\d{2}-\d{2}$/, decision);
    assert.notEqual(by, "", decision);
  }
});

test("DESIGN-GAPS.md has a header, a line format and one dated line per gap", () => {
  const text = read("docs", "design", "DESIGN-GAPS.md");
  assert.match(text, /^# Design gaps \(append-only\)/);
  assert.match(text, /Line format:/);
  const lines = text.split("\n").filter((l) => l.startsWith("- 2"));
  assert.ok(lines.length >= 10);
  for (const line of lines) {
    assert.match(
      line,
      // Earlier lines (Prompt 46) name a screen after DESIGN.md, give only the design's side, or say
      // "vellum.pen uses"; the file is append-only, so the grammar accepts those spellings.
      /^- \d{4}-\d{2}-\d{2} · [^·]+ · (?:(?:DESIGN\.md(?: [^:·]+)?|guide[^:]*) says: .+?[·;] )?vellum\.pen (?:has:|uses) /,
      line,
    );
  }
  const keys = parseGaps(text);
  assert.ok(keys.has("component-family:Script node"));
  assert.ok(keys.has("variable-name:vellum-ink"));
  for (const key of keys) assert.ok(!key.includes(";") && key.length < 80, `odd gap key: ${key}`);
});

test("the runbook exists and covers setup, the session rules and the unavailable-MCP case", () => {
  assert.ok(existsSync(at("docs", "runbooks", "design-mcp.md")));
  const text = read("docs", "runbooks", "design-mcp.md");
  for (const needle of [
    "Settings",
    "MCP",
    "scripts/design-open",
    "get_app_state",
    "read_skill",
    "design:snapshot",
    "design:check",
    "PEN_CLI_KEY",
    "snapshots",
  ])
    assert.ok(text.includes(needle), `runbook lacks "${needle}"`);
});
