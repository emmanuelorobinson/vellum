// TEST DATA: binary headers are synthetic, never executed.

import assert from "node:assert/strict";
import { test } from "node:test";
import { packageArgs, publishArgs, targetInfo, validateBinary } from "./packaging.mjs";

test("target matrix maps VSIX to native triples and rejects universal/wrong binaries", () => {
  assert.equal(targetInfo("darwin-arm64").triple, "aarch64-apple-darwin");
  assert.equal(targetInfo("win32-x64").binary, "vellum.exe");
  assert.throws(() => targetInfo("universal"));
  const mach = Buffer.alloc(32);
  mach.writeUInt32LE(0xfeedfacf);
  mach.writeUInt32LE(0x100000c, 4);
  validateBinary(mach, "darwin-arm64");
  assert.throws(() => validateBinary(mach, "darwin-x64"));
  assert.throws(() => validateBinary(mach, "linux-arm64"));
  const elf = Buffer.alloc(64);
  elf.set([0x7f, 69, 76, 70, 2, 1]);
  elf.writeUInt16LE(183, 18);
  validateBinary(elf, "linux-arm64");
  assert.throws(() => validateBinary(elf, "linux-x64"));
  const pe = Buffer.alloc(128);
  pe.write("MZ");
  pe.writeUInt32LE(64, 60);
  pe.write("PE\0\0", 64, "binary");
  pe.writeUInt16LE(0x8664, 68);
  validateBinary(pe, "win32-x64");
  assert.throws(() => validateBinary(pe, "win32-arm64"));
});
test("packaging does not publish and publishing uses the exact reviewed VSIX", () => {
  assert.deepEqual(packageArgs("darwin-arm64", "/tmp/test.vsix"), [
    "package",
    "--target",
    "darwin-arm64",
    "--no-dependencies",
    "--out",
    "/tmp/test.vsix",
  ]);
  assert.deepEqual(publishArgs("marketplace", "/tmp/test.vsix"), [
    "publish",
    "--packagePath",
    "/tmp/test.vsix",
  ]);
  assert.deepEqual(publishArgs("open-vsx", "/tmp/test.vsix"), ["publish", "/tmp/test.vsix"]);
  assert.throws(() => publishArgs("anything", "/tmp/test.vsix"));
});
