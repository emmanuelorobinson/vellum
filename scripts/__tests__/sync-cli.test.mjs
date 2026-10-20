// TEST DATA. @network/@unix: built service binary, local TCP listener and process signals required.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = fileURLToPath(new URL("../../", import.meta.url));
const binary = process.env.VELLUM_TEST_SYNC_BINARY;
function deadline(promise, ms) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("Sync CLI timed out")), ms);
      timer.unref();
    }),
  ]).finally(() => clearTimeout(timer));
}

test("@network @unix sync CLI seeds private data, requires --listen, serves and shuts down", {
  skip: !binary || process.platform === "win32",
  timeout: 45_000,
}, async () => {
  const executable = resolve(binary);
  const root = await mkdtemp(join(tmpdir(), "vellum-sync-cli-"));
  const data = join(root, "data");
  let child;
  let closed;
  const command = (...args) =>
    spawnSync(executable, ["--data", data, ...args], {
      cwd: repo,
      encoding: "utf8",
      timeout: 10_000,
      maxBuffer: 2 * 1024 * 1024,
    });
  try {
    const schema = command("schema");
    assert.equal(schema.status, 0, schema.stderr);
    const parsed = JSON.parse(schema.stdout);
    assert.ok(parsed.client.$schema && parsed.server.$schema);
    assert.equal(existsSync(data), false);
    const seeded = command(
      "seed",
      "--room",
      "test-cli",
      "--file",
      join(repo, "fixtures/documents/minimal.vellum"),
    );
    assert.equal(seeded.status, 0, seeded.stderr);
    assert.equal((await stat(data)).mode & 0o777, 0o700);
    assert.equal(command("serve", "--instance", "test-cli").status, 2);
    child = spawn(
      executable,
      ["--data", data, "serve", "--listen", "127.0.0.1:0", "--instance", "test-cli"],
      {
        cwd: repo,
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    closed = new Promise((resolveExit) =>
      child.once("close", (code, signal) => resolveExit({ code, signal })),
    );
    const ready = new Promise((resolveReady, reject) => {
      let stdout = "";
      child.stdout.on("data", (chunk) => {
        stdout += chunk.toString();
        if (stdout.length > 16_384) reject(new Error("Sync CLI output limit"));
        const port = /Sync listening on 127\.0\.0\.1:(\d+)/.exec(stdout)?.[1];
        if (port) resolveReady(port);
      });
      child.stderr.resume();
      child.once("error", reject);
      child.once("exit", () => reject(new Error("Sync CLI exited before listening")));
    });
    const port = await deadline(ready, 10_000);
    const response = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: AbortSignal.timeout(5_000),
    });
    assert.equal(response.status, 200);
    assert.equal(await response.text(), "ok");
    child.kill("SIGTERM");
    assert.deepEqual(await deadline(closed, 10_000), { code: 0, signal: null });
  } finally {
    if (child?.pid && child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    if (closed) await deadline(closed, 10_000);
    await rm(root, { recursive: true, force: true });
  }
});
