import type { EngineClient, EngineSnapshot, ToolTransaction } from "@vellum/engine-client";
import type { DaemonClient, DaemonEvent, Host, SyncClient, Telemetry } from "@vellum/host-api";

function assert(value: unknown, message: string): asserts value {
  if (!value) throw Error(`Editor port contract: ${message}`);
}
async function refuses(run: () => unknown, message: string) {
  let refused = false;
  try {
    await run();
  } catch {
    refused = true;
  }
  assert(refused, message);
}
export async function hostContract({
  host,
  failNextWrite,
  emitTheme,
}: {
  host: Host;
  failNextWrite(): void;
  emitTheme(): void;
}) {
  await refuses(() => host.readFile("missing"), "missing file is explicit");
  const uri = "contract.vellum";
  const bytes = new Uint8Array([1, 2]);
  let changes = 0;
  const watcher = host.watch(uri, () => {
    changes++;
  });
  await host.writeFile(uri, bytes);
  bytes[0] = 9;
  const read = await host.readFile(uri);
  assert(read[0] === 1, "write owns a copy");
  read[0] = 8;
  assert((await host.readFile(uri))[0] === 1, "read owns a copy");
  failNextWrite();
  await refuses(() => host.writeFile(uri, new Uint8Array([3])), "write failure is explicit");
  assert(
    (await host.readFile(uri))[0] === 1 && changes === 1,
    "failed replacement is atomic and sends no change",
  );
  watcher.dispose();
  watcher.dispose();
  await host.writeFile(uri, new Uint8Array([4]));
  assert(changes === 1, "watch disposal");
  const clip = { items: { "text/plain": new Uint8Array([65]) } };
  await host.clipboard.write(clip);
  clip.items["text/plain"][0] = 66;
  const result = await host.clipboard.read();
  assert(result.items["text/plain"]?.[0] === 65, "clipboard owns a copy");
  result.items["text/plain"]?.fill(9);
  assert(
    (await host.clipboard.read()).items["text/plain"]?.[0] === 65,
    "clipboard read owns a copy",
  );
  assert((await host.discoverDaemon()) === null, "offline daemon is explicit");
  for (const url of [
    "javascript:alert(1)",
    "file:///private/file",
    "https://user:password@example.test/",
  ])
    await refuses(() => host.openExternal(url), "unsafe external URL is refused");
  await host.openExternal("https://example.test/docs");
  let themes = 0;
  host.onThemeChange(() => {
    themes++;
  });
  emitTheme();
  assert(themes === 1 && host.theme().appearance === "dark", "live theme");
  host.dispose();
  host.dispose();
  await refuses(() => host.readFile(uri), "disposed host is explicit");
}
export function engineContract(
  engine: EngineClient,
  script: (snapshot: EngineSnapshot, operation?: "apply" | "undo" | "redo") => void,
) {
  assert(engine.state === "ready", "engine starts ready");
  const before = engine.snapshot();
  let events = 0;
  const d = engine.subscribe(() => {
    events++;
  });
  const transaction: ToolTransaction = {
    origin: { actor: { kind: "person", device: "contract-device" } },
    ops: [{ type: "scale", id: "contract01", factor: 2, anchor: { x: 0, y: 0 } }],
  };
  assert(
    engine.apply({ ...transaction, ops: [] }).status === "rejected",
    "empty transaction is rejected",
  );
  assert(
    engine.snapshot().revision === before.revision && events === 0,
    "rejection preserves state",
  );
  script({ ...before, revision: before.revision + 1 });
  assert(engine.apply(transaction).status === "applied", "one commit reaches engine");
  assert(
    Number(events) === 1 && engine.snapshot().revision === before.revision + 1,
    "commit emits one revision",
  );
  const actor = transaction.origin.actor;
  script({ ...before, revision: before.revision + 2 }, "undo");
  assert(engine.undo(actor).status === "applied", "undo is actor scoped");
  script({ ...before, revision: before.revision + 3 }, "redo");
  assert(engine.redo(actor).status === "applied", "redo is actor scoped");
  d.dispose();
  script({ ...before, revision: before.revision + 4, readOnly: true });
  engine.apply(transaction);
  assert(engine.apply(transaction).status === "rejected", "read-only engine refuses writes");
  engine.dispose();
  engine.dispose();
  assert((engine.state as string) === "disposed", "engine disposal");
  let refused = false;
  try {
    engine.snapshot();
  } catch {
    refused = true;
  }
  assert(refused, "disposed engine does not serve stale data");
}
export async function daemonContract(
  client: DaemonClient,
  source: { emit(event: DaemonEvent): void },
) {
  assert(client.state === "offline", "daemon starts offline");
  await refuses(() => client.request({ type: "pauseAll" }), "offline request refuses");
  let count = 0;
  const sub = client.subscribe(() => {
    count++;
  });
  await client.connect();
  assert((client.state as string) === "ready", "daemon connects");
  source.emit({ type: "invalidate", document: "contract.vellum" });
  assert(count === 1, "relay events delivered");
  sub.dispose();
  source.emit({ type: "invalidate", document: "contract.vellum" });
  assert(count === 1, "relay disposal");
  await client.request({ type: "pauseAll" });
  client.disconnect();
  await refuses(() => client.request({ type: "pauseAll" }), "disconnect prevents commands");
  await client.connect();
  client.dispose();
  client.dispose();
  await refuses(() => client.connect(), "disposed relay cannot reconnect");
}
export async function syncContract(client: SyncClient) {
  await client.connect();
  assert(client.state === "offline", "no-op sync stays offline");
  await refuses(() => client.send(new Uint8Array([1])), "no-op sync cannot claim to send");
  const d = client.subscribe(() => {
    throw Error("no-op sync received data");
  });
  d.dispose();
  client.dispose();
  client.dispose();
}
export function telemetryContract(client: Telemetry) {
  assert(!client.enabled, "telemetry is opt-in");
  client.event("contract", { count: 1 });
  client.dispose();
  client.dispose();
}
