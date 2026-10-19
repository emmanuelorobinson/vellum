// TEST DATA. Configuration and release gating without signing credentials or publishing.

import assert from "node:assert/strict";
import { test } from "node:test";
import { builderConfig } from "./packaging.mjs";

test("all six native targets and artifact formats are explicit; unsigned releases fail closed", () => {
  const config = builderConfig({ preview: true, version: "0.0.0", productName: "TEST DATA" });
  assert.deepEqual(config.mac.target, [
    { target: "dmg", arch: ["arm64", "x64"] },
    { target: "zip", arch: ["arm64", "x64"] },
  ]);
  assert.deepEqual(config.win.target, [{ target: "nsis", arch: ["x64", "arm64"] }]);
  assert.deepEqual(
    config.linux.target.map((t) => t.target),
    ["AppImage", "deb", "tar.gz"],
  );
  assert.ok(config.linux.target.every((t) => t.arch.join() === "x64,arm64"));
  assert.equal(config.win.verifyUpdateCodeSignature, true);
  assert.equal(config.forceCodeSigning, false);
  assert.throws(
    () => builderConfig({ preview: false, version: "0.0.0", productName: "TEST DATA" }),
    /release/,
  );
  assert.throws(
    () => builderConfig({ preview: false, version: "1.0.0", productName: "TEST DATA" }),
    /release/,
  );
});

test("release signer binds the exact artifact bytes without saving the private key", async () => {
  const { createHash, generateKeyPairSync, verify } = await import("node:crypto");
  const { mkdtemp, writeFile, readFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { spawnSync } = await import("node:child_process");
  const dir = await mkdtemp(join(tmpdir(), "vellum-release-test-"));
  try {
    const artifact = join(dir, "TEST-DATA.zip"),
      output = join(dir, "release.json");
    const bytes = Buffer.from("TEST DATA signed artifact");
    await writeFile(artifact, bytes);
    const key = generateKeyPairSync("ed25519");
    const result = spawnSync(
      process.execPath,
      [
        new URL("sign-release.mjs", import.meta.url).pathname,
        "--version",
        "1.2.0",
        "--platform",
        "darwin",
        "--arch",
        "arm64",
        "--artifact",
        artifact,
        "--out",
        output,
      ],
      { input: key.privateKey.export({ type: "pkcs8", format: "pem" }), encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const envelope = JSON.parse(await readFile(output, "utf8"));
    const payload = Buffer.from(envelope.payload, "base64");
    assert.ok(verify(null, payload, key.publicKey, Buffer.from(envelope.signature, "base64")));
    assert.equal(
      JSON.parse(payload).files[0].sha512,
      createHash("sha512").update(bytes).digest("base64"),
    );
    assert.equal(result.stdout, "");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("one packaging invocation builds only the validated daemon architecture", async () => {
  const { nativeConfig } = await import("./packaging.mjs");
  const config = nativeConfig(
    builderConfig({ preview: true, version: "0.0.0", productName: "TEST DATA" }),
    "darwin",
    "arm64",
  );
  assert.ok(config.mac.target.every((target) => target.arch.join() === "arm64"));
  assert.throws(() => nativeConfig(config, "other", "arm64"));
});
