// The documentation site's own checks: the generator covers every source, the links resolve, and the copy rules hold.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import {
  brandProse,
  checkContent,
  decisionPages,
  describeType,
  frontmatter,
  groupTools,
  keyboardRows,
  listPages,
  loadToolSnapshot,
  prepareContent,
  RUNBOOKS,
  readableKey,
  rewriteLinks,
  runbookPages,
  TOOL_GROUPS,
} from "../docs-content.mjs";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const root = resolve(app, "../..");
const content = join(app, "content/docs");

prepareContent();

describe("sources are all covered", () => {
  it("every runbook has a title and a description", () => {
    const files = readdirSync(join(root, "docs/runbooks")).filter((name) => name.endsWith(".md"));
    const pages = runbookPages();
    assert.equal(pages.length, files.length);
    for (const page of pages) {
      assert.ok(page.title.length > 0 && page.description.length > 0, page.slug);
    }
  });

  it("every decision record is copied, with its own page", () => {
    const files = readdirSync(join(root, "docs/architecture/decisions")).filter((name) =>
      /^ADR-\d{3}/.test(name),
    );
    assert.equal(decisionPages().length, files.length);
    const slugs = new Set(decisionPages().map((page) => page.slug));
    assert.equal(slugs.size, files.length, "two records do not share a slug");
  });

  it("every MCP tool in the snapshot belongs to exactly one group", () => {
    const tools = loadToolSnapshot();
    const grouped = groupTools(tools);
    const names = grouped.flatMap((group) => group.tools.map((tool) => tool.name));
    assert.equal(names.length, tools.length);
    assert.equal(new Set(names).size, tools.length);
  });

  it("a tool that no group names fails, so a new tool cannot go undocumented", () => {
    const tools = [...loadToolSnapshot(), { name: "a_new_tool", inputSchema: {} }];
    assert.throws(() => groupTools(tools), /a_new_tool/);
  });

  it("the keyboard reference covers the editor keybinding table", () => {
    const rows = keyboardRows();
    assert.ok(rows.length >= 80, `found ${rows.length} bindings`);
    assert.ok(rows.some((row) => row.id === "palette.open"));
  });

  it("every tool group has a description", () => {
    for (const [slug, title, description] of TOOL_GROUPS) {
      assert.ok(slug && title && description, slug);
    }
    for (const [slug, title, description] of RUNBOOKS) {
      assert.ok(slug && title && description, slug);
    }
  });
});

describe("the generated text is faithful", () => {
  it("reads the editor key names as people write them", () => {
    assert.equal(readableKey("KeyF"), "F");
    assert.equal(readableKey("Digit1"), "1");
    assert.equal(readableKey("Mod+Slash"), "Mod+/");
    assert.equal(readableKey("Mod+Shift+KeyS"), "Mod+Shift+S");
  });

  it("writes the brand token in prose, but not in code or in a link target", () => {
    const text = "Vellum runs `Vellum` and [the site](https://example.com/Vellum).";
    assert.equal(
      brandProse(text),
      "{{brand}} runs `Vellum` and [the site](https://example.com/Vellum).",
    );
  });

  it("points links to runbooks and decisions at their pages, and makes other repository files code", () => {
    const out = rewriteLinks(
      "[CLI](../runbooks/cli.md) [ADR](../architecture/decisions/ADR-005-execute.md) [Arch](../architecture/ARCHITECTURE.md)",
      "docs/runbooks/x.md",
    );
    assert.match(out, /\]\(\/docs\/operations\/runbooks\/cli\)/);
    assert.match(out, /\]\(\/docs\/reference\/decisions\/adr-005-execute\)/);
    assert.match(out, /\(`docs\/architecture\/ARCHITECTURE\.md`\)/);
  });

  it("quotes frontmatter so a colon or a quote in a title cannot break it", () => {
    const block = frontmatter('A "quoted": title', "Line one\nline two");
    const [, title, description] = block.match(/title: (.*)\ndescription: (.*)\n---/) ?? [];
    assert.equal(JSON.parse(title), 'A "quoted": title');
    assert.equal(JSON.parse(description), "Line one\nline two");
  });

  it("describes parameter types in words", () => {
    assert.equal(describeType({ type: "array", items: { type: "string" } }), "array of string");
    assert.equal(describeType({ enum: ["a", "b"] }), "one of `a`, `b`");
    assert.equal(
      describeType({ anyOf: [{ type: "string" }, { type: "null" }] }),
      "string (optional)",
    );
  });
});

describe("the site as a whole", () => {
  it("lists every page in the agent index", () => {
    const pages = listPages().filter((page) => page.title && page.description);
    const index = readFileSync(join(app, ".generated/agent-docs.ts"), "utf8");
    for (const page of pages) {
      assert.ok(index.includes(page.url), `the index lacks ${page.url}`);
    }
  });

  it("has no product name written literally in hand-written prose", () => {
    const offenders = listPages()
      .filter(
        (page) =>
          !/^(operations\/runbooks|reference\/decisions|reference\/mcp-tools)\//.test(page.rel),
      )
      .filter((page) =>
        /\bVellum\b/.test(page.body.replace(/```[\s\S]*?```/g, "").replace(/`[^`\n]*`/g, "")),
      )
      .map((page) => page.rel);
    assert.deepEqual(offenders, []);
  });

  it("passes every check: links, navigation, frontmatter, the copy rules and placeholders", () => {
    assert.deepEqual(checkContent(), []);
  });

  it("is written for an index the brand rename can reach: no literal name in generated titles", () => {
    for (const page of listPages()) {
      if (page.title) assert.doesNotMatch(page.title, /\bVellum\b/, page.rel);
    }
  });

  it("keeps the navigation files and the pages in step", () => {
    for (const dir of [
      "",
      "getting-started",
      "using-vellum",
      "agents",
      "operations",
      "reference",
      "architecture",
      "development",
    ]) {
      const meta = JSON.parse(readFileSync(join(content, dir, "meta.json"), "utf8"));
      assert.ok(Array.isArray(meta.pages), dir || "root");
    }
  });
});
