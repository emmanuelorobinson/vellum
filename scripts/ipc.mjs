#!/usr/bin/env node
// Tagged native IPC lane. Builds only the daemon binary and IPC adapter; no renderer needed.
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const offline = process.argv.includes("--offline") ? ["--offline"] : [];
function cargo(args, env = {}) {
  const result = spawnSync("cargo", [...args, ...offline], {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
cargo(["build", "-p", "vellum", "--bin", "vellum"]);
const binary = resolve(
  root,
  process.env.CARGO_TARGET_DIR ?? "target",
  "debug",
  process.platform === "win32" ? "vellum.exe" : "vellum",
);
// Cargo flags go before the test-runner separator.
const result = spawnSync(
  "cargo",
  ["test", ...offline, "-p", "vellum-ipc-local", "--test", "ipc", "--", "--ignored"],
  { cwd: root, stdio: "inherit", env: { ...process.env, VELLUM_IPC_BINARY: binary } },
);
if (result.error) throw result.error;
process.exit(result.status ?? 1);
