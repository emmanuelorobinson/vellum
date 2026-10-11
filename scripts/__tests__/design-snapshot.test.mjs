// scripts/design-snapshot against a fake `pen` CLI in a throwaway repository root.
// The real CLI needs `pen login` or PEN_CLI_KEY (tagged design-mcp, run locally); these tests need neither.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { bash, discard, put, scratch } from "./helpers.mjs";

const PEN_BYTES = "FAKE-PEN-BYTES";
const sha = (text) => createHash("sha256").update(text).digest("hex");

// What the query snippet prints. Rows are JSON after a tag, between two markers.
const RAW_QUERY_OUTPUT = [
  "some shell chrome that must be ignored",
  "@@vellum-inventory-begin",
  '@C ["K1","Component / Agent Panel / Building"]',
  '@C ["K2","Component / Composer / Stop"]',
  '@F ["AAA","C03 · Agent workspace · building · light",0,0,1280,800,["K1","K2"]]',
  '@F ["BBB","C03 · Agent workspace · building · dark",1400,0,1280,800,["K1"]]',
  '@F ["CCC","C03 · Agent workspace · building · 1512 · light",0,900,1512,982,[]]',
  '@F ["DDD","Library · Actor markers",0,2000,1600,1029,[]]',
  '@V {"themes":{"appearance":["light","dark","hc-light","hc-dark"]},"names":["chrome/panel","agent/teal"]}',
  "@@vellum-inventory-end",
].join("\n");

const FAKE_PEN = `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
const mode = process.env.FAKE_PEN_MODE || "ok";
if (mode === "unauthenticated") {
  console.error('[ERROR] Authentication required. Run "pen login" or set PEN_CLI_KEY environment variable.');
  process.exit(1);
}
const skill = mode === "drift"
  ? "The execute tool no longer exposes anything we know."
  : "function Get(...); function GetVariables(); function Print(...); function Export(nodeIds, format, outputPath, options);";
const input = fs.readFileSync(0, "utf8").split("\\n");
for (const line of input) {
  if (line.startsWith("read_skill(")) console.log(skill);
  const m = line.match(/^execute\\(\\{ input: (".*") \\}\\)$/);
  if (!m) continue;
  const snippet = JSON.parse(m[1]);
  if (snippet.includes("@@vellum-inventory-begin")) {
    console.log(fs.readFileSync(process.env.FAKE_PEN_QUERY_OUTPUT, "utf8"));
    continue;
  }
  const e = snippet.match(/Export\\(\\[(.*?)\\],"png","(.*?)",\\{scale:(\\d)\\}\\)/);
  if (e) {
    fs.mkdirSync(e[2], { recursive: true });
    for (const id of JSON.parse("[" + e[1] + "]")) {
      fs.writeFileSync(path.join(e[2], id + ".png"), "PNG@" + e[3] + "x:" + id);
    }
  }
}
`;

function fixtureRoot() {
  const root = scratch("design-snapshot");
  put(root, "docs/design/vellum.pen", PEN_BYTES);
  put(root, "fake-pen.cjs", FAKE_PEN);
  put(root, "query-output.txt", RAW_QUERY_OUTPUT);
  assert.equal(bash("design-open", ["--root", root, "--no-open"]).status, 0);
  return root;
}

const snapshot = (root, extraEnv = {}, extra = []) =>
  bash("design-snapshot", ["--root", root, ...extra], {
    env: {
      ...process.env,
      VELLUM_PEN_CLI: `node ${join(root, "fake-pen.cjs")}`,
      FAKE_PEN_QUERY_OUTPUT: join(root, "query-output.txt"),
      ...extraEnv,
    },
  });

const inventoryOf = (root) =>
  JSON.parse(readFileSync(join(root, "docs/design/snapshots/inventory.json"), "utf8"));

test("design-snapshot writes inventory.json with the fields the guide lists", () => {
  const root = fixtureRoot();
  try {
    const result = snapshot(root);
    assert.equal(result.status, 0, result.output);
    const inv = inventoryOf(root);
    assert.equal(inv.source_sha256, sha(PEN_BYTES));
    const first = inv.frames.find((f) => f.frame_id === "AAA");
    assert.deepEqual(
      {
        frame_id: first.frame_id,
        screen_id: first.screen_id,
        title: first.title,
        theme: first.theme,
        width: first.width,
        page: first.page,
        bounds: first.bounds,
        components: first.components,
      },
      {
        frame_id: "AAA",
        screen_id: "C03",
        title: "Agent workspace · building",
        theme: "light",
        width: 1280,
        page: null,
        bounds: { x: 0, y: 0, width: 1280, height: 800 },
        components: ["Component / Agent Panel / Building", "Component / Composer / Stop"],
      },
    );
    assert.equal(inv.frames.find((f) => f.frame_id === "CCC").width, 1512);
    assert.deepEqual(inv.variables.names, ["chrome/panel", "agent/teal"]);
    assert.deepEqual(
      inv.components.map((c) => c.id),
      ["K1", "K2"],
    );
  } finally {
    discard(root);
  }
});

test("design-snapshot exports PNG at 1x and 2x for screen frames only, named from id, theme and width", () => {
  const root = fixtureRoot();
  try {
    const result = snapshot(root);
    assert.equal(result.status, 0, result.output);
    const files = readdirSync(join(root, "docs/design/snapshots")).sort();
    assert.deepEqual(files, [
      "C03-dark-1280.png",
      "C03-dark-1280@2x.png",
      "C03-light-1280.png",
      "C03-light-1280@2x.png",
      "C03-light-1512.png",
      "C03-light-1512@2x.png",
      "inventory.json",
    ]);
    const dir = join(root, "docs/design/snapshots");
    assert.equal(readFileSync(join(dir, "C03-light-1280.png"), "utf8"), "PNG@1x:AAA");
    assert.equal(readFileSync(join(dir, "C03-light-1280@2x.png"), "utf8"), "PNG@2x:AAA");
  } finally {
    discard(root);
  }
});

test("design-snapshot is deterministic and removes snapshots of frames that no longer exist", () => {
  const root = fixtureRoot();
  try {
    put(root, "docs/design/snapshots/OLD-light-1280.png", "stale");
    assert.equal(snapshot(root).status, 0);
    const first = readFileSync(join(root, "docs/design/snapshots/inventory.json"), "utf8");
    assert.equal(existsSync(join(root, "docs/design/snapshots/OLD-light-1280.png")), false);
    assert.equal(snapshot(root).status, 0);
    assert.equal(readFileSync(join(root, "docs/design/snapshots/inventory.json"), "utf8"), first);
  } finally {
    discard(root);
  }
});

test("design-snapshot stops when read_skill no longer describes the API it relies on", () => {
  const root = fixtureRoot();
  try {
    const result = snapshot(root, { FAKE_PEN_MODE: "drift" });
    assert.notEqual(result.status, 0);
    assert.match(result.output, /execute/);
    assert.match(result.output, /Export/);
    assert.equal(existsSync(join(root, "docs/design/snapshots/inventory.json")), false);
  } finally {
    discard(root);
  }
});

test("design-snapshot explains pen login / PEN_CLI_KEY when the CLI is not authenticated", () => {
  const root = fixtureRoot();
  try {
    const result = snapshot(root, { FAKE_PEN_MODE: "unauthenticated" });
    assert.notEqual(result.status, 0);
    assert.match(result.output, /pen login/);
    assert.match(result.output, /PEN_CLI_KEY/);
    assert.equal(existsSync(join(root, "docs/design/snapshots/inventory.json")), false);
  } finally {
    discard(root);
  }
});

test("design-snapshot refuses to run without a current read-only copy", () => {
  const root = fixtureRoot();
  try {
    put(root, "docs/design/vellum.pen", "A NEWER DESIGN");
    const result = snapshot(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /design-open/);
  } finally {
    discard(root);
  }
});

test("design-snapshot --raw and --staging finalize exports made in an MCP session", () => {
  const root = fixtureRoot();
  try {
    put(root, "stage/1x/AAA.png", "PNG@1x:AAA");
    put(root, "stage/2x/AAA.png", "PNG@2x:AAA");
    put(root, "stage/1x/BBB.png", "PNG@1x:BBB");
    put(root, "stage/2x/BBB.png", "PNG@2x:BBB");
    put(root, "stage/1x/CCC.png", "PNG@1x:CCC");
    put(root, "stage/2x/CCC.png", "PNG@2x:CCC");
    const result = bash("design-snapshot", [
      "--root",
      root,
      "--raw",
      join(root, "query-output.txt"),
      "--staging",
      join(root, "stage"),
    ]);
    assert.equal(result.status, 0, result.output);
    assert.equal(inventoryOf(root).source_sha256, sha(PEN_BYTES));
    assert.equal(
      readFileSync(join(root, "docs/design/snapshots/C03-dark-1280@2x.png"), "utf8"),
      "PNG@2x:BBB",
    );
  } finally {
    discard(root);
  }
});

test("design-snapshot --staging fails when an export is missing", () => {
  const root = fixtureRoot();
  try {
    put(root, "stage/1x/AAA.png", "PNG@1x:AAA");
    const result = bash("design-snapshot", [
      "--root",
      root,
      "--raw",
      join(root, "query-output.txt"),
      "--staging",
      join(root, "stage"),
    ]);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /missing export/);
  } finally {
    discard(root);
  }
});

test("design-snapshot never modifies docs/design/vellum.pen", () => {
  const root = fixtureRoot();
  try {
    snapshot(root);
    snapshot(root, { FAKE_PEN_MODE: "drift" });
    assert.equal(readFileSync(join(root, "docs/design/vellum.pen"), "utf8"), PEN_BYTES);
  } finally {
    discard(root);
  }
});

test("design-snapshot --print-query prints the read-only query for an agent session", () => {
  const result = bash("design-snapshot", ["--print-query"]);
  assert.equal(result.status, 0, result.output);
  assert.match(result.stdout, /@@vellum-inventory-begin/);
  assert.doesNotMatch(
    result.stdout,
    /\b(Insert|Update|Replace|Delete|Move|Copy|SetVariables|Generate)\(/,
  );
});
