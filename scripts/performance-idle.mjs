// needs: native daemon and OS process counters. Isolated private state; no user documents or account.
import { execFile, spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";
import { summary } from "./performance-budget.mjs";

const exec = promisify(execFile);
const [binary, reportPath] = process.argv.slice(2);
if (!binary || !reportPath) throw Error("Usage: performance-idle.mjs RELEASE_BINARY REPORT.json");
const state = await mkdtemp(
    join(process.platform === "darwin" ? "/private/tmp" : tmpdir(), "vpf-"),
  ),
  run = join(state, "run");
await mkdir(run, { mode: 0o700 });
const child = spawn(resolve(binary), ["--run-dir", run, "daemon"], {
  stdio: "ignore",
  env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
});
let exited = false;
child.on("exit", () => {
  exited = true;
});
const closed = new Promise((resolve) => {
  child.once("exit", resolve);
  child.once("error", resolve);
});
try {
  if (!child.pid) throw Error("Daemon failed to start");
  let ready = false;
  for (let i = 0; i < 100; i++) {
    if (exited) throw Error("Daemon exited before readiness");
    try {
      await exec(resolve(binary), ["--run-dir", run, "status", "--json"], { timeout: 1000 });
      ready = true;
      break;
    } catch {
      await delay(100);
    }
  }
  if (!ready) throw Error("Daemon readiness timed out");
  const values = [];
  for (let i = 0; i < 20; i++) {
    if (exited) throw Error("Daemon exited during RSS sampling");
    const { stdout } =
      process.platform === "win32"
        ? await exec(
            "powershell.exe",
            [
              "-NoProfile",
              "-NonInteractive",
              "-Command",
              `(Get-Process -Id ${child.pid}).WorkingSet64`,
            ],
            { timeout: 2000 },
          )
        : await exec("ps", ["-o", "rss=", "-p", String(child.pid)], { timeout: 2000 });
    const n = Number(stdout.trim()) * (process.platform === "win32" ? 1 : 1024);
    if (!Number.isFinite(n) || n <= 0) throw Error("OS returned invalid RSS");
    values.push(n);
    await delay(100);
  }
  const report = JSON.parse(await readFile(reportPath, "utf8"));
  report.metrics.idleRss = summary(values);
  report.adapters = [...new Set([...report.adapters, "ipc-local"])];
  await writeFile(reportPath, JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ idleRss: report.metrics.idleRss }));
} finally {
  child.kill();
  await Promise.race([closed, delay(3000)]);
  if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGKILL");
    await closed;
  }
  await rm(state, { recursive: true, force: true });
}
