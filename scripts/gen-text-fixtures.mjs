// TEST DATA: regenerate production text oracle from the unchanged Prompt 5 layout path.
// Needs Rust and the five fonts fetched by spikes/renderer/scripts/fetch-fonts.mjs (network once).
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
execFileSync(process.execPath, ["spikes/renderer/scripts/fetch-fonts.mjs", "--check"], {
  cwd: root,
  stdio: "inherit",
});
const output = execFileSync(
  "cargo",
  [
    "run",
    "--locked",
    "--quiet",
    "--manifest-path",
    "spikes/renderer/Cargo.toml",
    "-p",
    "spike-scene",
    "--example",
    "text-layout-oracle",
  ],
  { cwd: root, encoding: "utf8" },
);
const json = `${JSON.stringify({ comment: "TEST DATA: Prompt 5 text layout oracle; regenerate with scripts/gen-text-fixtures.mjs", fixtures: JSON.parse(output) }, null, 2)}\n`;
const path = new URL("../fixtures/text/prompt5.json", import.meta.url);
if (process.argv.includes("--check")) {
  if (readFileSync(path, "utf8") !== json) throw new Error("text oracle differs; regenerate it");
} else writeFileSync(path, json);
console.log("30 Prompt 5 text fixtures verified");
