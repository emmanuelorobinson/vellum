import { execFile } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
/** Fixed absolute executable, no shell, no inherited credentials, bounded time/output. */
export function binaryRunner(binary: string) {
  if (!isAbsolute(binary)) throw new Error("Absolute binary required");
  return async (args: string[], cwd: string) => {
    const runtimeRoot = await mkdtemp(
      join(process.platform === "darwin" ? "/private/tmp" : tmpdir(), "vw-"),
    );
    const runDir = join(runtimeRoot, "run");
    const execute = (args: string[], timeout = 120000) =>
      new Promise<void>((resolve, reject) => {
        execFile(
          binary,
          ["--run-dir", runDir, ...args],
          {
            cwd,
            env: {
              PATH: "/usr/bin:/bin",
              HOME: cwd,
              TURBO_TELEMETRY_DISABLED: "1",
              NEXT_TELEMETRY_DISABLED: "1",
            },
            timeout,
            maxBuffer: 64 * 1024,
            killSignal: "SIGKILL",
          },
          (error) => (error ? reject(new Error("Render failed")) : resolve()),
        );
      });
    try {
      await execute(args);
    } finally {
      await execute(["daemon", "stop"], 10000).catch(() => {});
      await rm(runtimeRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
    }
  };
}
