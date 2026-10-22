// TEST DATA. Stage packages locally; never invoke a registry.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { signingPlan } from "../release-native.mjs";
import {
  artifactNames,
  distributionNames,
  homebrewFormula,
  npmManifest,
  packNpm,
  requireDistribution,
  stageNpm,
  targets,
  wrapperSource,
} from "../release-packages.mjs";
import { feedOrder, requireRemoteTag } from "../release-publish.mjs";

test("npm wrapper pins every native optional package and has no installer or network fallback", () => {
  const pkg = npmManifest("1.2.3");
  assert.equal(pkg.name, "@vellum/cli");
  assert.equal(Object.keys(pkg.optionalDependencies).length, 6);
  assert.ok(Object.values(pkg.optionalDependencies).every((v) => v === "1.2.3"));
  assert.equal(pkg.scripts, undefined);
  for (const target of targets) {
    const native = npmManifest("1.2.3", target.id);
    assert.deepEqual(native.os, [target.platform]);
    assert.deepEqual(native.cpu, [target.arch]);
    assert.equal(native.scripts, undefined);
    assert.equal(artifactNames("1.2.3", target.id).length, 4);
  }
  assert.throws(() => npmManifest("0.0.0"));
  assert.throws(() => npmManifest("1.2.3", "bad"));
});

test("wrapper forwards arguments and exit status; missing platform package fails explicitly", () => {
  const dir = mkdtempSync(join(tmpdir(), "vellum-wrapper-"));
  try {
    writeFileSync(join(dir, "cli.mjs"), wrapperSource);
    const missing = spawnSync(process.execPath, [join(dir, "cli.mjs")], { encoding: "utf8" });
    assert.equal(missing.status, 1);
    assert.match(missing.stderr, /platform package/);
    const pkg = join(dir, "node_modules/@vellum", `cli-${process.platform}-${process.arch}`);
    mkdirSync(join(pkg, "bin"), { recursive: true });
    writeFileSync(join(pkg, "package.json"), '{"name":"TEST DATA","version":"1.2.3"}');
    symlinkSync(
      process.execPath,
      join(pkg, "bin", process.platform === "win32" ? "vellum.exe" : "vellum"),
    );
    chmodSync(join(dir, "cli.mjs"), 0o755);
    const result = spawnSync(process.execPath, [join(dir, "cli.mjs"), "-e", "process.exit(7)"], {
      encoding: "utf8",
    });
    assert.equal(result.status, 7, result.stderr);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("Homebrew pins all four Darwin/Linux archives to reviewed exact hashes", () => {
  const hashes = Object.fromEntries(
    targets.filter((t) => t.platform !== "win32").map((t) => [t.id, "a".repeat(64)]),
  );
  const formula = homebrewFormula("test/repo", "1.2.3", hashes);
  assert.equal((formula.match(/sha256 "a{64}"/g) ?? []).length, 4);
  assert.match(formula, /releases\/download\/v1.2.3\/vellum-1.2.3-darwin-arm64.tar.gz/);
  assert.match(formula, /bin.install "vellum"/);
  assert.throws(() => homebrewFormula("test/repo", "1.2.3", {}));
  assert.throws(() => homebrewFormula('test/repo"\n', "1.2.3", hashes));
});

test("a release needs all six target distributions, wrapper, formula, SBOMs and candidate evidence", () => {
  const files = new Map(distributionNames("1.2.3").map((name) => [name, Buffer.from("TEST DATA")]));
  requireDistribution(files, "1.2.3");
  assert.equal(files.size, 29);
  const missing = new Map(files);
  missing.delete("vellum-1.2.3-win32-arm64.vsix");
  assert.throws(() => requireDistribution(missing, "1.2.3"));
  assert.throws(() =>
    requireDistribution(new Map([...files, ["extra", Buffer.from("x")]]), "1.2.3"),
  );
});

test("native signing refuses missing OS identities, and verifies the signed binary", () => {
  assert.throws(() => signingPlan("darwin-arm64", "binary", {}));
  const mac = signingPlan("darwin-arm64", "binary", {
    CSC_NAME: "TEST DATA",
    APPLE_KEYCHAIN_PROFILE: "TEST DATA",
  });
  assert.deepEqual(mac[1], ["codesign", ["--verify", "--strict", "binary"]]);
  assert.throws(() => signingPlan("win32-x64", "binary", {}));
  assert.throws(() =>
    signingPlan("win32-x64", "binary", {
      VELLUM_WINDOWS_CERT_SHA1: "a".repeat(40),
      VELLUM_TIMESTAMP_URL: "http://example.invalid",
    }),
  );
  const win = signingPlan("win32-x64", "binary", {
    VELLUM_WINDOWS_CERT_SHA1: "a".repeat(40),
    VELLUM_TIMESTAMP_URL: "https://example.invalid",
  });
  assert.deepEqual(win[1], ["signtool", ["verify", "/pa", "binary"]]);
  assert.deepEqual(signingPlan("linux-arm64", "binary", {}), []);
});

test("publication refuses moved tags and resolves annotated tags to the signed candidate", () => {
  const sha = "a".repeat(40),
    tag = "b".repeat(40);
  requireRemoteTag({ type: "commit", sha }, sha, () => assert.fail());
  requireRemoteTag({ type: "tag", sha: tag }, sha, () => ({ object: { type: "commit", sha } }));
  assert.throws(() => requireRemoteTag({ type: "commit", sha: tag }, sha, () => assert.fail()));
  assert.throws(() =>
    requireRemoteTag({ type: "tag", sha: tag }, sha, () => ({ object: { type: "tag", sha: tag } })),
  );
});

test("update feeds publish payloads before signed envelopes and latest pointers, without deletion", () => {
  const names = ["latest.yml", "release-linux-arm64.json", "Vellum-1.2.3.AppImage"];
  const plan = feedOrder("linux-arm64", names, "s3://test-bucket/updates/");
  assert.deepEqual(
    plan.map((p) => p.name),
    [names[2], names[1], names[0]],
  );
  assert.equal(plan[0].destination, "s3://test-bucket/updates/linux-arm64/Vellum-1.2.3.AppImage");
  assert.equal(plan[2].cache, "no-store");
  for (const bad of [
    "s3://test-bucket/../",
    "s3://test-bucket/updates",
    "https://example.invalid/",
  ])
    assert.throws(() => feedOrder("linux-arm64", names, bad));
  assert.throws(() =>
    feedOrder("linux-arm64", ["../escape", ...names], "s3://test-bucket/updates/"),
  );
  assert.throws(() => feedOrder("linux-arm64", [names[0]], "s3://test-bucket/updates/"));
});

test("generated wrapper packs offline into the actual npm artifact with its executable and licence", () => {
  const dir = mkdtempSync(join(tmpdir(), "vellum-npm-pack-"));
  try {
    const pkg = join(dir, "package"),
      out = join(dir, "out");
    mkdirSync(out);
    stageNpm(pkg, "1.2.3");
    const filename = packNpm(pkg, out);
    assert.equal(filename, "vellum-cli-1.2.3.tgz");
    const listing = spawnSync("tar", ["-tzf", join(out, filename)], { encoding: "utf8" });
    assert.equal(listing.status, 0);
    assert.match(listing.stdout, /package\/bin\/vellum.mjs/);
    assert.match(listing.stdout, /package\/LICENSE/);
    assert.doesNotMatch(listing.stdout, /node_modules|\.env/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
