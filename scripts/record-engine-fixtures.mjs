#!/usr/bin/env node
// TEST DATA. Invokes the production launch path with the versioned, bounded recorder.
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const args = process.argv.slice(2);
if (args.includes("--help")) {
  console.log(
    "Engines lane: fixtures:record [--engine claude-code|codex] [--case success|signed-out] [--claude path] [--codex path] [--model alias] [--out folder] [--transport app-server|exec] [--offline]\nUses your installed engine sign-in. The success case calls a Vellum canvas tool and uses model quota. Never signs you in or installs/updates an engine.",
  );
  process.exit(0);
}
const offline = args.includes("--offline") ? ["--offline"] : [];
const forwarded = args.filter((a) => a !== "--offline");
for (let i = 0; i < forwarded.length; i += 2) {
  if (
    !["--engine", "--case", "--claude", "--codex", "--model", "--out", "--transport"].includes(
      forwarded[i],
    ) ||
    !forwarded[i + 1] ||
    (forwarded[i] === "--transport" && !["app-server", "exec"].includes(forwarded[i + 1])) ||
    (forwarded[i] === "--engine" && !["claude-code", "codex"].includes(forwarded[i + 1])) ||
    (forwarded[i] === "--case" && !["success", "signed-out"].includes(forwarded[i + 1]))
  ) {
    console.error("record-engine-fixtures: invalid arguments; see --help");
    process.exit(2);
  }
}
const engineIndex = forwarded.indexOf("--engine");
const engine = engineIndex < 0 ? "claude-code" : forwarded.splice(engineIndex, 2)[1];
function cargo(args) {
  const result = spawnSync("cargo", args, {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
cargo(["build", ...offline, "-p", "vellum", "--bin", "vellum", "--no-default-features"]);
const binary = resolve(
  root,
  process.env.CARGO_TARGET_DIR ?? "target",
  "debug",
  process.platform === "win32" ? "vellum.exe" : "vellum",
);
cargo([
  "run",
  ...offline,
  "-p",
  "vellum",
  "--example",
  engine === "codex" ? "record_codex" : "record_claude",
  "--no-default-features",
  "--",
  "--vellum",
  binary,
  ...forwarded,
]);
