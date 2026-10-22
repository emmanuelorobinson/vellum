// Tagged vscode lane: requires a desktop session, VS Code >= 1.140 and the bundled native daemon.

import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";
import { build } from "esbuild";

const root = fileURLToPath(new URL("..", import.meta.url));
await build({
  absWorkingDir: root,
  entryPoints: ["test/suite.ts"],
  bundle: true,
  platform: "node",
  format: "cjs",
  external: ["vscode"],
  outfile: "dist/test/suite.cjs",
});
const sandbox = await mkdtemp(
  resolve(process.platform === "darwin" ? "/private/tmp" : tmpdir(), "vellum-vsc-"),
);
const runDir = resolve(sandbox, "run");
await mkdir(resolve(sandbox, "home"), { recursive: true });
// A workspace folder: the daemon holds files inside the host's workspace (ADR-033).
await mkdir(resolve(sandbox, "workspace"), { recursive: true });
await mkdir(resolve(sandbox, "user-data/User"), { recursive: true });
await writeFile(
  resolve(sandbox, "user-data/User/settings.json"),
  JSON.stringify({ "telemetry.telemetryLevel": "off" }),
);
try {
  await runTests({
    extensionTestsEnv: {
      VELLUM_RUN_DIR: runDir,
      VELLUM_TEST_BINARY: resolve(
        root,
        "dist/package/bin",
        process.platform === "win32" ? "vellum.exe" : "vellum",
      ),
      VELLUM_CLIENT_HOME: resolve(sandbox, "home"),
      CODEX_HOME: resolve(sandbox, "home/.codex"),
      XDG_CONFIG_HOME: resolve(sandbox, "home/.config"),
      APPDATA: resolve(sandbox, "home/AppData"),
      CLAUDE_CONFIG_DIR: resolve(sandbox, "home/.claude"),
    },
    extensionDevelopmentPath: resolve(root, "dist/package"),
    extensionTestsPath: resolve(root, "dist/test/suite.cjs"),
    ...(process.env.VELLUM_VSCODE_EXECUTABLE
      ? { vscodeExecutablePath: process.env.VELLUM_VSCODE_EXECUTABLE }
      : { version: "1.140.0" }),
    launchArgs: [
      "--user-data-dir",
      resolve(sandbox, "user-data"),
      "--extensions-dir",
      resolve(sandbox, "extensions"),
      "--disable-extensions",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-workspace-trust",
      resolve(sandbox, "workspace"),
    ],
  });
} finally {
  spawnSync(
    resolve(root, "dist/package/bin", process.platform === "win32" ? "vellum.exe" : "vellum"),
    ["--run-dir", runDir, "daemon", "stop"],
    { stdio: "ignore" },
  );
  await rm(sandbox, { recursive: true, force: true });
}
