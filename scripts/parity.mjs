#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkCoverage } from "./lib/parity.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(resolve(repo, path), "utf8");
try {
  const result = checkCoverage(
    read("docs/architecture/ARCHITECTURE.md"),
    JSON.parse(read("fixtures/parity/coverage.json")),
    read,
  );
  if (process.argv.includes("--json")) console.log(JSON.stringify(result, null, 2));
  else {
    console.log(
      `Parity: ${result.total} v1 rows mapped; ${result.covered} have active tests, ${result.pending.length} pending.`,
    );
    for (const row of result.pending)
      console.log(`  Prompt ${row.prompt}: ${row.surface} — ${row.reason}`);
    console.log(
      "Active mappings name behavior tests; scripted-agent E2E still needs the model/engine lanes.",
    );
  }
} catch (error) {
  console.error(`Parity: ${error.message}`);
  process.exitCode = 1;
}
