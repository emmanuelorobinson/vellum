// TEST DATA. Recorder command validation does not run Cargo, Claude Code or a provider.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const script = new URL("../record-engine-fixtures.mjs", import.meta.url);
function run(...args) {
  return spawnSync(process.execPath, [script.pathname, ...args], { encoding: "utf8" });
}
test("fixture recorder help describes quota and does not launch a build", () => {
  const result = run("--help");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /model quota/);
  assert.match(result.stdout, /signed-out/);
  assert.doesNotMatch(result.stdout, /Compiling/);
});
test("invalid recorder cases and unpaired options fail before running anything", () => {
  for (const args of [
    ["--case", "invented"],
    ["--claude"],
    ["--prompt", "repo secrets"],
    ["--case", "success", "unexpected"],
  ]) {
    const result = run(...args);
    assert.equal(result.status, 2);
    assert.match(result.stderr, /invalid arguments/);
  }
});
test("real engine fixture has an explicit version manifest and only projected protocol", () => {
  const base = new URL("../../fixtures/engines/claude-code/2.1.301/", import.meta.url);
  const manifest = JSON.parse(readFileSync(new URL("success.recorded.json", base), "utf8"));
  assert.equal(manifest.kind, "recorded");
  assert.equal(manifest.testData, true);
  assert.equal(manifest.version, "2.1.301");
  assert.ok(manifest.mcpCalls > 0);
  const text = readFileSync(new URL("success.recorded.ndjson", base), "utf8");
  const records = text
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.equal(records[0].mcp_servers[0].status, "connected");
  assert.equal(records.at(-1).type, "result");
  assert.equal(records.at(-1).is_error, false);
  assert.doesNotMatch(text, /accessToken|apiKeySource|session_id|"cwd"|@example/);
});
