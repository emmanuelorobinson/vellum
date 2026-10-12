// reports.md is the running hand-off for the person: what each prompt left open, decided or skipped.
// AGENTS.md must tell every session to read it first and to update it after each prompt.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { at, read } from "./helpers.mjs";

test("AGENTS.md points at reports.md: read it first, update it after every prompt", () => {
  const agents = read("AGENTS.md");
  assert.match(agents, /reports\.md/);
  const read_ = agents.slice(
    agents.indexOf("## Read, in this order"),
    agents.indexOf("## Read-only files"),
  );
  assert.match(read_, /reports\.md/, "reports.md is not in the reading order");
  assert.match(agents, /update `reports\.md`[^\n]*(after|at the end of) (each|every) prompt/i);
});

test("reports.md exists, explains itself and has an entry for every finished prompt", () => {
  assert.ok(existsSync(at("reports.md")), "reports.md is missing");
  const text = read("reports.md");
  assert.match(text, /^# Reports/);
  assert.match(text, /## Needs a person/);
  for (const n of [1, 2, 3])
    assert.match(text, new RegExp(`^## Prompt ${n}\\b`, "m"), `no entry for Prompt ${n}`);
});
