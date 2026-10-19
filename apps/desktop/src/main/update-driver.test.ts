// TEST DATA. The real signature/digest verifier runs; native, network and installer boundaries are mocked.
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { Readable } from "node:stream";
import { afterEach, expect, it, vi } from "vitest";

const boundary = vi.hoisted(() => ({
  config: "",
  bytes: Buffer.from("TEST DATA artifact"),
  info: {} as unknown,
  install: vi.fn(),
  download: vi.fn(async () => ["/cache/update.zip"]),
}));
vi.mock("electron", () => ({ app: { isPackaged: true } }));
vi.mock("node:fs/promises", () => ({
  readFile: async () => boundary.config,
  stat: async () => ({ size: boundary.bytes.length }),
}));
vi.mock("node:fs", () => ({ createReadStream: () => Readable.from([boundary.bytes]) }));
vi.mock("electron-updater", () => ({
  default: {
    autoUpdater: {
      on() {},
      setFeedURL() {},
      checkForUpdates: async () => ({ updateInfo: boundary.info }),
      downloadUpdate: boundary.download,
      quitAndInstall: boundary.install,
    },
  },
}));

import { updateDriver } from "./update-driver";
import { Updates } from "./updates";

const original = Object.getOwnPropertyDescriptor(process, "resourcesPath");
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  if (original) Object.defineProperty(process, "resourcesPath", original);
  else Reflect.deleteProperty(process, "resourcesPath");
});
async function setup(tampered = false) {
  Object.defineProperty(process, "resourcesPath", {
    value: "/bundle/resources",
    configurable: true,
  });
  const key = generateKeyPairSync("ed25519");
  boundary.bytes = Buffer.from("TEST DATA artifact");
  boundary.config = JSON.stringify({
    feed: "https://updates.example.test/",
    publicKey: key.publicKey.export({ type: "spki", format: "pem" }),
    publisher: "TEST DATA",
  });
  const files = [
    {
      url: "app.zip",
      sha512: createHash("sha512").update(boundary.bytes).digest("base64"),
      size: boundary.bytes.length,
    },
  ];
  boundary.info = { version: "1.1.0", files };
  const payload = Buffer.from(
    JSON.stringify({ version: "1.1.0", platform: process.platform, arch: process.arch, files }),
  );
  const signature = sign(null, payload, key.privateKey).toString("base64");
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            payload: (tampered ? Buffer.from("TEST DATA tampered manifest") : payload).toString(
              "base64",
            ),
            signature,
          }),
        ),
    ),
  );
  const driver = await updateDriver();
  expect(driver).not.toBeNull();
  return new Updates(driver, "1.0.0");
}
it("rejects bad manifest signatures before any artifact download and keeps the running version", async () => {
  const updates = await setup(true);
  await updates.check();
  expect(updates.state.kind).toBe("failed");
  await updates.download();
  expect(updates.install()).toBe(false);
  expect(boundary.download).not.toHaveBeenCalled();
  expect(boundary.install).not.toHaveBeenCalled();
});
it("rejects changed artifact bytes even when updater download itself resolves", async () => {
  const updates = await setup();
  await updates.check();
  expect(updates.state.kind).toBe("available");
  boundary.bytes = Buffer.from("TEST DATA artifacX");
  await updates.download();
  expect(updates.state.kind).toBe("failed");
  expect(updates.install()).toBe(false);
  expect(boundary.install).not.toHaveBeenCalled();
});
it("installs only the exact signed artifact after an explicit download", async () => {
  const updates = await setup();
  await updates.check();
  await updates.download();
  expect(updates.state.kind).toBe("ready");
  expect(updates.install()).toBe(true);
  expect(boundary.install).toHaveBeenCalledExactlyOnceWith(false, true);
});
