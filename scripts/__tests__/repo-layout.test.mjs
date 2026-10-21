// Guards the shape the guide (§8) and Prompt 1 promise: every crate/package/app exists with a smoke
// test, every advertised script exists, and AGENTS.md states the rules agents must follow.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const at = (...p) => join(repo, ...p);
const read = (...p) => readFileSync(at(...p), "utf8");

const rustCrates = [
  "vellum-types",
  "vellum-core",
  "contracts",
  "vellum-wasm",
  "vellum",
  "sync-server",
  "vellum-testkit",
  ...[
    "doc-loro",
    "layout-taffy",
    "text-parley",
    "index-rstar",
    "pathops-kurbo",
    "codec-json",
    "codec-json-dir",
    "assets-fs",
    "assets-s3",
    "render-skia",
    "render-vello",
    "shader-sksl",
    "script-quickjs",
    "sessions-sqlite",
    "secrets-keyring",
    "browser-cdp",
    "llm-openai-compat",
    "llm-openai-responses",
    "llm-anthropic",
    "llm-google",
    "engine-builtin",
    "engine-claude-code",
    "engine-codex",
    "import-svg",
    "import-image",
    "import-figma-clipboard",
    "import-figma-api",
    "import-html",
    "icons",
    "images-stock",
    "images-generate",
    "image-ops",
    "export-tokens",
    "export-html",
    "export-raster",
  ].map((a) => `adapters/${a}`),
];
// renderer-canvaskit is deliberately absent: it exists only if ADR-001 picks CanvasKit-in-TS.
const tsPackages = [
  "config",
  "types-ts",
  "renderer-api",
  "renderer-canvas2d",
  "host-api",
  "host-vscode",
  "host-electron",
  "host-browser",
  "host-fake",
  "engine-client",
  "editor-core",
  "editor-react",
  "ui-tokens",
  "contracts",
  "auth",
  "billing",
  "storage",
  "mail",
  "jobs",
  "telemetry",
  "design-systems",
  "skills",
  "styles",
  "prompt-packs",
  "test-utils",
].map((p) => `packages/${p}`);
const apps = ["extension", "desktop", "account", "api", "web", "docs"].map((a) => `apps/${a}`);

const hasTestFile = (dir) => {
  const found = [];
  const walk = (d) => {
    if (!existsSync(d)) return;
    for (const e of readdirSync(d)) {
      const p = join(d, e);
      if (e === "node_modules" || e === ".next") continue;
      if (statSync(p).isDirectory()) walk(p);
      else if (/\.(test|spec)\.(ts|tsx)$/.test(e) || (/\.rs$/.test(e) && p.includes("/tests/")))
        found.push(p);
    }
  };
  walk(dir);
  return found.length > 0;
};

test("every Rust crate in guide §8 is a workspace member with a smoke test", () => {
  for (const c of rustCrates) {
    assert.ok(existsSync(at("crates", c, "Cargo.toml")), `crates/${c}/Cargo.toml`);
    assert.ok(existsSync(at("crates", c, "src")), `crates/${c}/src`);
    assert.ok(hasTestFile(at("crates", c)), `crates/${c} has no test`);
  }
});

test("every TS package and app in guide §8 exists with a smoke test", () => {
  for (const d of [...tsPackages, ...apps]) {
    assert.ok(existsSync(at(d, "package.json")), `${d}/package.json`);
    assert.ok(hasTestFile(at(d)), `${d} has no test`);
    const pkg = JSON.parse(read(d, "package.json"));
    assert.equal(pkg.private, true, `${d} must be private`);
    assert.ok(pkg.scripts?.test, `${d} needs a test script`);
    assert.ok(pkg.scripts?.["type-check"], `${d} needs a type-check script`);
  }
});

test("brand constants agree between Rust and TypeScript", () => {
  const rust = read("crates", "vellum-types", "src", "brand.rs");
  const ts = read("packages", "config", "src", "index.ts");
  for (const needle of ["Vellum", "Design where your code lives."]) {
    assert.ok(rust.includes(`"${needle}"`), `brand.rs lacks ${needle}`);
    assert.ok(ts.includes(`"${needle}"`), `config lacks ${needle}`);
  }
});

test("every advertised root script exists and points at real files", () => {
  const pkg = JSON.parse(read("package.json"));
  const required = [
    "dev",
    "build",
    "lint",
    "type-check",
    "test",
    "test:watch",
    "format",
    "format:check",
    "refs",
    "refs:index",
    "deps:check",
    "clean-room",
    "copy-lint",
    "cargo:deny",
    "types:gen",
    "layout-oracle",
    "design:open",
    "design:snapshot",
    "design:check",
    "fixtures:record",
    "clean",
  ];
  for (const s of required) assert.ok(pkg.scripts?.[s], `package.json is missing script "${s}"`);
  for (const cmd of Object.values(pkg.scripts)) {
    for (const m of cmd.matchAll(/(?:bash|node)\s+(scripts\/[\w./-]+)/g)) {
      assert.ok(existsSync(at(m[1])), `script file ${m[1]} is referenced but missing`);
    }
  }
});

test("root files from Prompt 1 exist", () => {
  const files = [
    "Cargo.toml",
    "rust-toolchain.toml",
    "pnpm-workspace.yaml",
    "turbo.json",
    "tsconfig.base.json",
    "biome.json",
    ".editorconfig",
    ".gitignore",
    ".env.example",
    "AGENTS.md",
    "README.md",
    "THIRD_PARTY_NOTICES.md",
    "docker-compose.dev.yml",
    ".github/workflows/ci.yml",
  ];
  for (const f of files) assert.ok(existsSync(at(f)), `${f} is missing`);
  assert.match(read("rust-toolchain.toml"), /wasm32-unknown-unknown/);
});

test(".gitignore keeps the things that must never be committed out", () => {
  const lines = read(".gitignore")
    .split("\n")
    .map((l) => l.trim());
  for (const entry of ["refs/", "target/", ".design-cache/", ".env*", "*.vellum.recovery"]) {
    assert.ok(lines.includes(entry), `.gitignore lacks ${entry}`);
  }
  assert.ok(lines.includes("!.env.example"), ".env.example must stay tracked");
});

test(".env.example holds no secrets", () => {
  for (const line of read(".env.example").split("\n")) {
    const m = line.match(/^([A-Z0-9_]*(KEY|TOKEN|SECRET|PASSWORD)[A-Z0-9_]*)=(.*)$/);
    if (m) assert.equal(m[3].trim(), "", `${m[1]} must be blank in .env.example`);
  }
});

test("AGENTS.md states the rules every later prompt depends on", () => {
  const agents = read("AGENTS.md");
  const rules = [
    [/docs\/product/, "docs/product is named as read-only"],
    [/READ-ONLY/, "READ-ONLY is stated"],
    [/docs\/design\/vellum\.pen/, "vellum.pen is named"],
    [/docs\/format/, "docs/format is named"],
    [/refs\//, "refs/ is named"],
    [
      /DESIGN-GAPS\.md[^\n]*append-only|append-only[^\n]*DESIGN-GAPS\.md/i,
      "DESIGN-GAPS.md is append-only",
    ],
    [/Pencil MCP/, "Pencil MCP is the way to read the design"],
    [/scripts\/design-open/, "scripts/design-open"],
    [/get_app_state/, "confirm the document with get_app_state"],
    [/read_skill/, "call read_skill first"],
    [/screen ID/i, "frames are asked for by screen ID"],
    [/DESIGN-DECISIONS\.md/, "only recorded options are built"],
    [/Op transaction/, "one write path"],
    [/Origin/, "every change is attributed"],
    [/DocStore/, "nothing else touches DocStore"],
    [/adapters\//, "vendor crates live in adapters/"],
    [/§3\.2/, "hard problems point at the reference paths"],
    [/MIT\/Apache/, "porting only from MIT/Apache"],
    [/THIRD_PARTY_NOTICES\.md/, "ported code needs a notice"],
    [/penpot/i, "never port from penpot"],
    [/tldraw/i, "never port from tldraw"],
    [/pen\.dev/, "never copy from pen.dev"],
    [/blue[^\n]*selection/i, "blue means selection"],
    [/violet[^\n]*component/i, "violet means component"],
    [/amber[^\n]*needs you/i, "amber means needs you"],
    [/red[^\n]*error/i, "red means errors"],
    [/rounded squares/i, "agents are rounded squares"],
    [/circles/i, "people are circles"],
    [/never colou?r alone/i, "state is symbol + label"],
    [/no personas/i, "no personas"],
    [/exclamation/i, "no exclamation marks"],
    [/on your computer/, "copy: on your computer"],
    [/in your repo/, "copy: in your repo"],
    [/keychain/i, "keys live in the keychain"],
    [/canary/i, "the canary scanner"],
    [/telemetry/i, "no telemetry"],
    [/network listener/i, "no network listener"],
  ];
  for (const [re, why] of rules) assert.match(agents, re, `AGENTS.md must cover: ${why}`);
});
