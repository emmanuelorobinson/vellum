// TEST DATA. Native process fixture; no keychain, listener or account access.
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { NativeSyncWire } from "./native-sync";

it.skipIf(process.platform === "win32")(
  "keeps credentials in the native bridge and closes child processes",
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "vellum sync fixture "));
    const path = join(dir, "fixture.mjs");
    try {
      await writeFile(
        path,
        `#!${process.execPath}\nif(process.argv.slice(2).join(',')!=='sync,--endpoint,ws://127.0.0.1:7310,--room,test-room,--key,test-key-name')process.exit(2);\nprocess.stderr.write('private diagnostic');\nprocess.stdin.on('data',()=>process.stdout.write(JSON.stringify({v:1,adapter:'loro-1',epoch:1,revision:1,body:{type:'ack'}})+'\\n'));`,
      );
      await chmod(path, 0o700);
      const wire = new NativeSyncWire({
        binary: path,
        endpoint: "ws://127.0.0.1:7310",
        room: "test-room",
        key: "test-key-name",
      });
      const result = new Promise<unknown>((resolve) => {
        void wire.open(resolve, () => {});
      });
      wire.send({ v: 1, adapter: "loro-1", epoch: 0, request: { type: "pull", after: 0 } });
      expect(await result).toMatchObject({ body: { type: "ack" } });
      wire.close();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  },
);
