#!/usr/bin/env node
// Builds docs/reference/REFERENCE-INDEX.md from guide §3.2 (hard problems) and §3.3 (other sources), and
// checks that every file or directory the guide names exists in refs/ (guide Prompt 2).
//
// Usage: node scripts/reference-index.mjs            write the index
//        node scripts/reference-index.mjs --check    fail if a listed path is missing or the index is stale
//        --root <dir>  repository root (defaults to this repository; the tests pass a scratch directory)
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const INDEX = join("docs", "reference", "REFERENCE-INDEX.md");
const GUIDE = join("docs", "implementation", "vellum-implementation-guide.md");

// A backticked token counts as a path when it names a directory (trailing slash) or a file with one of
// these extensions. Identifiers such as `workspace.fs` or `codex` are not paths.
const FILE_EXTENSION =
  /\.(rs|ts|tsx|js|jsx|mjs|cjs|md|json|toml|css|html|yml|yaml|sh|txt|lock|svg|png|wasm)$/;
const isPathLike = (token) => token.endsWith("/") || FILE_EXTENSION.test(token);

// Splits the guide's §3.2 and §3.3 tables into rows. Nothing else in the guide is read.
export function parseGuide(markdown) {
  const problems = [];
  const sources = [];
  let section = null;
  for (const line of markdown.split("\n")) {
    if (/^#{1,2} /.test(line)) {
      section = /^## 3\.2\b/.test(line) ? "3.2" : /^## 3\.3\b/.test(line) ? "3.3" : null;
      continue;
    }
    if (!line.startsWith("|")) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());
    if (section === "3.2" && /^R\d+$/.test(cells[0])) {
      problems.push({ id: cells[0], problem: cells[1], study: cells[2], usedIn: cells[3] });
    }
    if (section === "3.3" && cells[0] !== "Source" && !/^-+$/.test(cells[0])) {
      sources.push({ source: cells[0], readFor: cells[1] });
    }
  }
  return { problems, sources };
}

// `a/{b,c}.rs` -> [`a/b.rs`, `a/c.rs`]
export function expandBraces(token) {
  const match = token.match(/\{([^{}]*)\}/);
  if (!match) return [token];
  const [whole, alternatives] = match;
  const before = token.slice(0, match.index);
  const after = token.slice(match.index + whole.length);
  return alternatives.split(",").flatMap((alt) => expandBraces(before + alt + after));
}

const dirOf = (path) => (posix.dirname(path) === "." ? "" : posix.dirname(path));

// Finds `rel` the way the guide means it: relative to the directory of the previous path in the same
// row, walking up its ancestors, and finally the repository root. A bare Rust file named inside a crate
// directory (R3's `scripts/gentest/` (`main.rs`)) is also looked for in that crate's `src/`.
// Returns { path, isDir } or null.
function locate(repoDir, prevDir, rel) {
  const target = rel.replace(/\/$/, "");
  const inCrate = target.endsWith(".rs") && !target.includes("/");
  let base = prevDir;
  for (;;) {
    for (const candidate of [
      posix.join(base, target),
      ...(inCrate ? [posix.join(base, "src", target)] : []),
    ]) {
      const stat = statSync(join(repoDir, candidate), { throwIfNoEntry: false });
      if (stat) return { path: candidate, isDir: stat.isDirectory() };
    }
    if (base === "") return null;
    base = dirOf(base);
  }
}

// Resolves every path of one row. Returns { repos, found: [{repo, path, isDir}], missing: [text] }.
// A row's segments are separated by ';'. A segment starts with a pinned repository name, or inherits the
// repository of the segment before it.
function resolveStudy(problem, repoNames, root) {
  const repos = [];
  const found = [];
  const missing = [];
  let repo = null;
  for (const segment of problem.study
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean)) {
    const lead = segment.match(/^([a-z][a-z0-9-]*) /);
    let body = segment;
    if (lead && repoNames.has(lead[1])) {
      repo = lead[1];
      body = segment.slice(lead[0].length);
    } else if (lead && segment.slice(lead[0].length).startsWith("`")) {
      throw new Error(`${problem.id}: "${lead[1]}" is not a pinned repository (scripts/refs.tsv)`);
    } else if (repo === null) {
      throw new Error(`${problem.id}: "${segment}" does not start with a pinned repository`);
    }
    if (!repos.includes(repo)) repos.push(repo);

    const repoDir = join(root, "refs", repo);
    const tokens = [...body.matchAll(/`([^`]+)`/g)].map((m) => m[1]).filter(isPathLike);
    if (tokens.length > 0 && !existsSync(repoDir)) {
      throw new Error(`refs/${repo} is not cloned: run pnpm refs first`);
    }
    let prevDir = "";
    for (const token of tokens) {
      let last = null;
      for (const rel of expandBraces(token)) {
        const hit = locate(repoDir, prevDir, rel);
        if (hit) {
          found.push({ repo, ...hit });
          last = hit;
        } else {
          missing.push(`${problem.id} ${repo}: ${rel}`);
        }
      }
      if (last) prevDir = last.isDir ? last.path : dirOf(last.path);
    }
  }
  return { repos, found, missing };
}

const escapeRe = (text) => text.replace(/[.*+?^${}()|[\]\\-]/g, "\\$&");

// The §3.3 source names the repository it means by name. A backticked path there must be unambiguous.
function resolveSource(row, repoNames, root) {
  const tokens = [...row.source.matchAll(/`([^`]+)`/g)].map((m) => m[1]).filter(isPathLike);
  if (tokens.length === 0) return { found: [], missing: [] };
  const named = [...repoNames].filter((name) =>
    new RegExp(`\\b${escapeRe(name)}\\b`, "i").test(row.source),
  );
  if (named.length !== 1) {
    throw new Error(
      `§3.3 "${row.source}" names ${named.length} pinned repositories; one is needed for its paths`,
    );
  }
  const repo = named[0];
  const repoDir = join(root, "refs", repo);
  if (!existsSync(repoDir)) throw new Error(`refs/${repo} is not cloned: run pnpm refs first`);
  const found = [];
  const missing = [];
  let prevDir = "";
  for (const token of tokens) {
    let last = null;
    for (const rel of expandBraces(token)) {
      const hit = locate(repoDir, prevDir, rel);
      if (hit) {
        found.push({ repo, ...hit });
        last = hit;
      } else {
        missing.push(`§3.3 ${repo}: ${rel}`);
      }
    }
    if (last) prevDir = last.isDir ? last.path : dirOf(last.path);
  }
  return { found, missing };
}

const shown = (hit) => `refs/${hit.repo}/${hit.path}${hit.isDir ? "/" : ""}`;

// The index text. Nothing in it depends on the clock or the machine, so it diffs cleanly.
export function renderIndex({ manifest, problems, sources, root }) {
  const repoNames = new Set(manifest.map((r) => r.name));
  const missing = [];
  const rows = problems.map((problem) => {
    const result = resolveStudy(problem, repoNames, root);
    missing.push(...result.missing);
    return { problem, ...result };
  });
  const sourceRows = sources.map((row) => {
    const result = resolveSource(row, repoNames, root);
    missing.push(...result.missing);
    return { row, ...result };
  });

  const out = [];
  out.push("# Reference index", "");
  out.push(
    "Generated by `scripts/reference-index.mjs` from guide §3.2 and §3.3. Do not edit by hand. After the",
    "guide or `scripts/refs.tsv` changes, run `pnpm refs:index`. `pnpm refs` clones the repositories and",
    "checks that every path below exists in `refs/`.",
    "",
  );
  out.push("## Repositories", "");
  out.push(
    "Each clone under `refs/` is read-only, blob-less and shallow, at the commit below. Penpot is read only",
    "(MPL-2.0). tldraw is read for docs only.",
    "",
    "| Name | Pinned commit | Licence | Use |",
    "|---|---|---|---|",
  );
  for (const r of manifest) {
    const use = r.use === "-" ? "" : r.use;
    out.push(`| ${r.name} | \`${r.sha}\` | ${r.licence === "-" ? "" : r.licence} | ${use} |`);
  }
  out.push("");

  out.push("## Hard problems (guide §3.2)", "");
  for (const { problem, repos, found } of rows) {
    out.push(`### ${problem.id} — ${problem.problem}`, "");
    out.push(`Used in: ${problem.usedIn}`, "");
    for (const repo of repos) {
      const paths = found.filter((h) => h.repo === repo).map((h) => `\`${shown(h)}\``);
      out.push(
        paths.length > 0
          ? `- ${repo}: ${paths.join(", ")}`
          : `- ${repo}: no file path, concept only`,
      );
    }
    out.push("");
  }

  out.push("## Other repositories and docs (guide §3.3)", "");
  out.push("| Source | Read for | Local path |", "|---|---|---|");
  for (const { row, found } of sourceRows) {
    const local =
      found.length > 0 ? found.map((h) => `\`${shown(h)}\``).join(", ") : "external, not cloned";
    out.push(`| ${row.source} | ${row.readFor} | ${local} |`);
  }
  out.push("");

  return { text: out.join("\n"), missing };
}

function parseArgs(argv) {
  const args = { check: false, root: DEFAULT_ROOT };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--check") args.check = true;
    else if (argv[i] === "--root") args.root = resolve(argv[++i] ?? "");
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  return args;
}

function main(argv) {
  const { check, root } = parseArgs(argv);
  const manifestText = readFileSync(join(root, "scripts", "refs.tsv"), "utf8");
  const [header, ...body] = manifestText.split("\n").filter((l) => l !== "" && !l.startsWith("#"));
  const keys = header.split("\t");
  const manifest = body.map((line) =>
    Object.fromEntries(line.split("\t").map((v, i) => [keys[i], v])),
  );

  if (!existsSync(join(root, "refs"))) {
    console.error("reference-index: refs/ is missing. Run pnpm refs first.");
    return 1;
  }
  const { problems, sources } = parseGuide(readFileSync(join(root, GUIDE), "utf8"));
  let rendered;
  try {
    rendered = renderIndex({ manifest, problems, sources, root });
  } catch (error) {
    console.error(`reference-index: ${error.message}`);
    return 1;
  }
  if (rendered.missing.length > 0) {
    console.error("reference-index: paths named in the guide are missing from refs/:");
    for (const line of rendered.missing) console.error(`  missing: ${line}`);
    return 1;
  }

  const indexPath = join(root, INDEX);
  if (check) {
    const committed = existsSync(indexPath) ? readFileSync(indexPath, "utf8") : null;
    if (committed !== rendered.text) {
      console.error(`reference-index: ${INDEX} is out of date. Run pnpm refs:index.`);
      return 1;
    }
    console.log(
      `reference-index: ${INDEX} is current; ${problems.length} hard problems and ${sources.length} sources resolve in refs/`,
    );
    return 0;
  }
  mkdirSync(dirname(indexPath), { recursive: true });
  writeFileSync(indexPath, rendered.text);
  console.log(`reference-index: wrote ${INDEX}`);
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
