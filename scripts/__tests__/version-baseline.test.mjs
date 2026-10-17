// Version baseline (guide §5 and Prompt 2): every §5 line has an exact, non-prerelease version recorded in
// docs/architecture/version-baseline.md; the manifests pin the versions the baseline records; the
// reference SHAs are recorded; and no lockfile holds a prerelease (canary, beta, rc or alpha).
import assert from "node:assert/strict";
import { test } from "node:test";
import { parseTsv, read } from "./helpers.mjs";

const baseline = () => read("docs", "architecture", "version-baseline.md");

// Table rows of a Markdown document as arrays of trimmed cells (separator rows are dropped).
function tableRows(markdown) {
  return markdown
    .split("\n")
    .filter((line) => line.startsWith("|") && !/^\|[-:\s|]+\|$/.test(line))
    .map((line) =>
      line
        .split("|")
        .slice(1, -1)
        .map((cell) => cell.trim()),
    );
}

// The resolved version recorded for a component: the second cell of its row (the first cell).
function resolvedOf(component) {
  const row = tableRows(baseline()).find((cells) => cells[0] === component);
  assert.ok(row, `version-baseline.md has no row for "${component}"`);
  return row[1];
}

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\/@]/g, "\\$&");

// Every component named in guide §5 (plus the pins the manifests carry). Each must have a row.
const REQUIRED = [
  "Rust toolchain",
  "tokio",
  "axum",
  "interprocess",
  "loro",
  "taffy",
  "parley",
  "fontique",
  "skia-safe",
  "vello",
  "kurbo",
  "i_overlay",
  "rstar",
  "rquickjs",
  "rmcp",
  "schemars",
  "ts-rs",
  "wasm-bindgen",
  "notify",
  "rusqlite",
  "keyring",
  "clap",
  "chromiumoxide",
  "naga",
  "cargo-deny",
  "serde",
  "serde_json",
  "Node",
  "pnpm",
  "Turborepo",
  "TypeScript",
  "Vite",
  "React",
  "@types/react",
  "@types/react-dom",
  "@types/node",
  "@types/vscode",
  "@base-ui/react",
  "shadcn",
  "Tailwind CSS",
  "Zustand",
  "Zod",
  "i18next",
  "Electron",
  "electron-builder",
  "electron-updater",
  "@vscode/vsce",
  "ovsx",
  "canvaskit-wasm",
  "@modelcontextprotocol/sdk",
  "Playwright",
  "Vitest",
  "Storybook",
  "Biome",
  "dependency-cruiser",
  "Next.js",
  "better-auth",
  "drizzle-orm",
  "Hono",
  "Claude Code",
  "Codex CLI",
  "@pen.dev/cli",
];

test("every component in guide §5 has a row with one exact, non-prerelease version", () => {
  for (const name of REQUIRED) {
    const version = resolvedOf(name);
    assert.match(
      version,
      /^\d+\.\d+\.\d+$/,
      `${name} must record an exact version, got "${version}"`,
    );
  }
});

test("the manifests pin the versions the baseline records", () => {
  const pkg = JSON.parse(read("package.json"));
  const workspace = read("pnpm-workspace.yaml");
  const cargo = read("Cargo.toml");
  const catalog = (name) =>
    workspace.match(new RegExp(`^\\s+"?${escapeRe(name)}"?:\\s+(\\S+)\\s*$`, "m"))?.[1];
  const cargoDep = (name) =>
    cargo.match(new RegExp(`^${escapeRe(name)} = (?:\\{ version = )?"([^"]+)"`, "m"))?.[1];
  const channel = read("rust-toolchain.toml").match(/^channel = "([^"]+)"/m)?.[1];

  const pins = {
    pnpm: pkg.packageManager.replace("pnpm@", ""),
    Turborepo: pkg.devDependencies.turbo,
    Biome: pkg.devDependencies["@biomejs/biome"],
    "dependency-cruiser": pkg.devDependencies["dependency-cruiser"],
    TypeScript: catalog("typescript"),
    Vitest: catalog("vitest"),
    "@types/node": catalog("@types/node"),
    React: catalog("react"),
    "@types/react": catalog("@types/react"),
    "@types/react-dom": catalog("@types/react-dom"),
    "@types/vscode": catalog("@types/vscode"),
    "Next.js": catalog("next"),
    Electron: catalog("electron"),
    "Rust toolchain": channel,
    serde: cargoDep("serde"),
    serde_json: cargoDep("serde_json"),
    schemars: cargoDep("schemars"),
    "ts-rs": cargoDep("ts-rs"),
    clap: cargoDep("clap"),
  };
  for (const [name, version] of Object.entries(pins)) {
    assert.equal(resolvedOf(name), version, `${name}: baseline vs manifest`);
  }
});

test("Node is recorded on the 24 LTS line that .nvmrc and engines pin", () => {
  assert.match(read(".nvmrc").trim(), /^24$/);
  assert.match(resolvedOf("Node"), /^24\./);
  assert.match(read("package.json"), /"node": ">=24 <25"/);
});

test("the Rust MSRV is the minor line of the pinned toolchain", () => {
  const channel = read("rust-toolchain.toml").match(/^channel = "([^"]+)"/m)[1];
  const msrv = read("Cargo.toml").match(/^rust-version = "([^"]+)"/m)[1];
  assert.equal(channel.split(".").slice(0, 2).join("."), msrv);
});

test("the baseline records every reference repository at the SHA scripts/refs.tsv pins", () => {
  for (const { name, sha } of parseTsv(read("scripts", "refs.tsv"))) {
    assert.ok(baseline().includes(sha), `version-baseline.md lacks ${name} at ${sha}`);
  }
});

// Transitive packages that have never had a stable release, each allowed by name and exact version with a
// decision in docs/architecture/version-baseline.md. Nothing the repo pins itself may be a prerelease.
const PRERELEASE_EXCEPTIONS = [
  "@polka/url@1.0.0-next.29", // C24: via sirv ← @vitest/browser ← @storybook/addon-vitest
  "gensync@1.0.0-beta.2", // C25: owner-approved exact exception, via Babel ← React Storybook's docgen
];

test("no lockfile pins a prerelease version", () => {
  const cargoPrereleases = [
    ...read("Cargo.lock").matchAll(/^version = "(\d+\.\d+\.\d+-[^"]+)"/gm),
  ].map((m) => m[1]);
  const pnpmPrereleases = [
    ...read("pnpm-lock.yaml").matchAll(
      /^ {2}'?((?:@[^@\s/]+\/)?[^@\s']+)@(\d+\.\d+\.\d+-[0-9A-Za-z.-]+)/gm,
    ),
  ].map((m) => `${m[1]}@${m[2]}`);
  assert.deepEqual(cargoPrereleases, [], "Cargo.lock");
  assert.deepEqual(
    [...new Set(pnpmPrereleases)].filter((p) => !PRERELEASE_EXCEPTIONS.includes(p)),
    [],
    "pnpm-lock.yaml",
  );
  const baseline = read("docs", "architecture", "version-baseline.md");
  for (const exception of PRERELEASE_EXCEPTIONS)
    assert.ok(baseline.includes(exception), `${exception} has no decision in version-baseline.md`);
});
