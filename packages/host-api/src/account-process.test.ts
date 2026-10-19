// TEST DATA: executable fixtures use Node only, never a keychain or account service.
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { accountRunner } from "./native-account";

const directories: string[] = [];
afterEach(async () => {
  await Promise.all(
    directories.splice(0).map((path) => rm(path, { recursive: true, force: true })),
  );
});
async function executable(source: string) {
  const directory = await mkdtemp(join(tmpdir(), "vellum account "));
  directories.push(directory);
  const file = join(directory, "fixture.mjs");
  await writeFile(file, `#!${process.execPath}\n${source}\n`);
  await chmod(file, 0o700);
  return file;
}
it.skipIf(process.platform === "win32")(
  "streams fragmented NDJSON from a fixed executable with spaces and fixed argv",
  async () => {
    const file = await executable(`
    if (process.argv.slice(2).join(',') !== 'login,--json') process.exit(2);
    process.stderr.write('private diagnostic');
    process.stdout.write('{"ok":true,"command":"lo');
    setTimeout(() => process.stdout.write('gin","signed_in":true}\\n'), 5);
  `);
    const records: unknown[] = [];
    expect(
      await accountRunner(file)("login", new AbortController().signal, (record) =>
        records.push(record),
      ),
    ).toBe(0);
    expect(records).toEqual([{ ok: true, command: "login", signed_in: true }]);
  },
);
it.skipIf(process.platform === "win32")(
  "kills a stalled child on cancellation and waits for closed stdio",
  async () => {
    const file = await executable(
      `process.stdout.write('{"ready":true}\\n'); setInterval(() => {}, 1000);`,
    );
    const abort = new AbortController();
    const result = accountRunner(file)("login", abort.signal, () => abort.abort());
    expect(await result).toBe(1);
  },
);
it.skipIf(process.platform === "win32")(
  "rejects malformed, oversized and truncated output without reflecting diagnostics",
  async () => {
    for (const source of [
      `process.stdout.write('not JSON\\n');`,
      `process.stdout.write('x'.repeat(70 * 1024)); setInterval(() => {}, 1000);`,
      `process.stdout.write('{"truncated":');`,
    ]) {
      const file = await executable(source);
      await expect(
        accountRunner(file)("whoami", new AbortController().signal, () => {}),
      ).rejects.toThrow("Account request failed");
    }
  },
);
it("sanitizes a missing executable", async () => {
  await expect(
    accountRunner("/missing/private-path")("whoami", new AbortController().signal, () => {}),
  ).rejects.toThrow("Account request failed");
});
