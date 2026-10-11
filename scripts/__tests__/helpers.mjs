// Shared helpers for the script tests. `node --test` runs only *.test.mjs, so this file is not a test.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const at = (...parts) => join(repo, ...parts);
export const read = (...parts) => readFileSync(at(...parts), "utf8");
export const guideText = () => read("docs", "implementation", "vellum-implementation-guide.md");

export const scratch = (name) => mkdtempSync(join(tmpdir(), `vellum-${name}-`));
export const discard = (dir) => rmSync(dir, { recursive: true, force: true });
export const fileUrl = (dir) => pathToFileURL(dir).href;

export function put(root, file, text) {
  mkdirSync(dirname(join(root, file)), { recursive: true });
  writeFileSync(join(root, file), text);
}

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", ...options });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  return { status: result.status, stdout, stderr, output: stdout + stderr };
}

// Runs a bash script from the repository's scripts/ directory.
export function bash(script, args = [], options = {}) {
  return run("bash", [at("scripts", script), ...args], { cwd: options.cwd ?? repo });
}

// Parses a tab-separated file with one header line and '#' comment lines (scripts/refs.tsv).
export function parseTsv(text) {
  const lines = text.split("\n").filter((line) => line.trim() !== "" && !line.startsWith("#"));
  const [header, ...body] = lines;
  const keys = header.split("\t");
  return body.map((line) =>
    Object.fromEntries(line.split("\t").map((value, i) => [keys[i], value])),
  );
}

// A git repository with one commit per entry, so a test can pin an exact SHA. Returns the SHAs in
// order. Fetching by SHA and partial clones are switched on because a real origin allows both.
export function makeOrigin(dir, commits) {
  mkdirSync(dir, { recursive: true });
  run("git", ["init", "-q", "-b", "main", dir]);
  run("git", ["-C", dir, "config", "uploadpack.allowAnySHA1InWant", "true"]);
  run("git", ["-C", dir, "config", "uploadpack.allowFilter", "true"]);
  return commits.map(({ file, text }) => {
    put(dir, file, text);
    run("git", ["-C", dir, "add", "-A"]);
    const committed = run("git", [
      "-C",
      dir,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "-c",
      "commit.gpgsign=false",
      "commit",
      "-q",
      "-m",
      `add ${file}`,
    ]);
    if (committed.status !== 0) throw new Error(committed.output);
    return run("git", ["-C", dir, "rev-parse", "HEAD"]).stdout.trim();
  });
}
