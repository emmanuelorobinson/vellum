#!/usr/bin/env node
// Tagged model-http lane. No public endpoint, no account, no real API key.
import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const artifacts = mkdtempSync(join(tmpdir(), "vellum-model-canary-"));
const env = {
  ...process.env,
  VELLUM_CANARY_API_KEY: randomBytes(32).toString("hex"),
  VELLUM_CANARY_ARTIFACTS: artifacts,
};
const offline = process.argv.includes("--offline") ? ["--offline"] : [];
try {
  const result = spawnSync(
    "cargo",
    ["test", ...offline, "-p", "vellum-testkit", "--test", "providers", "--", "--include-ignored"],
    { cwd: root, env, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  );
  const session = process.argv.includes("--session")
    ? spawnSync(
        process.execPath,
        [
          "scripts/skia-build.mjs",
          "test",
          ...offline,
          "--locked",
          "-p",
          "vellum",
          "--test",
          "engine",
          "scripted_session_canary",
          "--",
          "--ignored",
        ],
        { cwd: root, env, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
      )
    : { status: 0 };
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}${session.stdout ?? ""}${session.stderr ?? ""}`;
  writeFileSync(join(artifacts, "tests.log"), output);
  // Never print a leaked credential, even when a test fails.
  process.stdout.write(output.replaceAll(env.VELLUM_CANARY_API_KEY, "[redacted]"));
  const testsPassed =
    !result.error && !session.error && result.status === 0 && session.status === 0;
  const roots = testsPassed ? [root, artifacts] : [artifacts];
  if (testsPassed && existsSync(join(homedir(), ".vellum"))) roots.push(join(homedir(), ".vellum"));
  if (!testsPassed)
    process.stderr.write(
      "canary: tests failed; only test artifacts will be scanned, full coverage is not claimed\n",
    );
  const scan = spawnSync(
    "cargo",
    [
      "run",
      "--release",
      ...offline,
      "-p",
      "vellum-testkit",
      "--example",
      "scan_canary",
      "--",
      ...roots,
    ],
    { cwd: root, env, stdio: "inherit" },
  );
  process.exitCode =
    result.error ||
    session.error ||
    scan.error ||
    result.status !== 0 ||
    session.status !== 0 ||
    scan.status !== 0
      ? 1
      : 0;
} finally {
  rmSync(artifacts, { recursive: true, force: true });
}
