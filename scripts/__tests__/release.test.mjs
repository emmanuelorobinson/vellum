// TEST DATA. No provider, signing service, registry or real release is contacted.
import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  assessRuns,
  blockingDesignGaps,
  canonicalBlockers,
  checkPolicy,
  digest,
  releaseVersion,
  requireParity,
  requirePlaywright,
  sealArtifacts,
  unpackSnapshotBundle,
  verifyArtifacts,
} from "../lib/release.mjs";
import { collectRuns } from "../release.mjs";

const sha = "a".repeat(40);
const now = Date.parse("2026-10-22T00:00:00Z");
const policy = {
  version: 1,
  maxAgeHours: 168,
  lanes: [{ workflow: "ci.yml", jobs: ["Rust", "UI"] }],
  blockers: [
    { id: "test", statement: "TEST DATA", lanes: ["ci.yml"], evidence: ["tests.rs"], open: [] },
  ],
};
const run = (overrides = {}) => ({
  id: 42,
  run_attempt: 1,
  head_sha: sha,
  path: ".github/workflows/ci.yml",
  event: "push",
  status: "completed",
  conclusion: "success",
  created_at: "2026-10-21T00:00:00Z",
  head_repository: { full_name: "test/repo" },
  jobs: ["Rust", "UI"].map((name) => ({ name, status: "completed", conclusion: "success" })),
  ...overrides,
});
const assess = (runs, p = policy) => assessRuns(p, runs, { sha, repository: "test/repo", now });

test("candidate gate needs every exact-commit trusted job and refuses stale, skipped or failed evidence", () => {
  assert.equal(assess([run()]).ok, true);
  for (const runs of [
    [],
    [run({ head_sha: "b".repeat(40) })],
    [run({ event: "pull_request" })],
    [run({ head_repository: { full_name: "fork/repo" } })],
    [run({ status: "in_progress" })],
    [run({ conclusion: "failure" })],
    [run({ created_at: "2026-10-01T00:00:00Z" })],
    [run({ created_at: "2026-10-23T00:00:00Z" })],
    [run({ jobs: run().jobs.slice(1) })],
    [run({ jobs: [run().jobs[0], { ...run().jobs[1], conclusion: "skipped" }] })],
    [run({ jobs: [...run().jobs, run().jobs[0]] })],
  ])
    assert.equal(assess(runs).ok, false, JSON.stringify(runs));
  assert.equal(assess([run(), run({ id: 43, conclusion: "failure" })]).ok, false);
  assert.equal(assess([run(), run({ id: 43, status: "in_progress" })]).ok, false);
  assert.equal(
    assess([run()], {
      ...policy,
      blockers: [{ ...policy.blockers[0], open: ["Needs native acceptance"] }],
    }).ok,
    false,
  );
});

test("policy checks real evidence and maps all sixteen canonical blockers", () => {
  const actual = JSON.parse(readFileSync(new URL("../release-policy.json", import.meta.url)));
  const read = (path) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
  assert.deepEqual(checkPolicy(actual, read), []);
  assert.equal(actual.blockers.length, 16);
  assert.deepEqual(
    actual.blockers.map((b) => b.statement),
    canonicalBlockers(read("docs/implementation/vellum-implementation-guide.md")),
  );
  assert.ok(actual.blockers.some((b) => b.open.length));
  assert.ok(
    checkPolicy({ ...policy, blockers: [policy.blockers[0], policy.blockers[0]] }, () => "test")
      .length,
  );
  assert.ok(
    checkPolicy(
      { ...policy, blockers: [{ ...policy.blockers[0], evidence: ["../escape"] }] },
      () => "test",
    ).length,
  );
  assert.ok(
    checkPolicy(
      { ...policy, blockers: [{ ...policy.blockers[0], lanes: ["missing.yml"] }] },
      () => "test",
    ).length,
  );
  assert.ok(
    checkPolicy(policy, () => {
      throw Error("absent");
    }).length,
  );
});

test("blocking design gaps cannot be excused by prose saying they are not resolved", () => {
  assert.equal(blockingDesignGaps("- 2026-10-22 · blocking: not resolved yet").length, 1);
  assert.equal(blockingDesignGaps("- 2026-10-22 · non-blocking mismatch").length, 0);
  assert.equal(blockingDesignGaps("- 2026-10-22 · blocking mismatch · status=closed").length, 0);
  assert.equal(
    blockingDesignGaps(
      "- 2026-10-22 · blocking mismatch · check=test\n- 2026-10-22 · check=test · status=closed",
    ).length,
    0,
  );
});

test("design bundles require exact approved bytes, all 1x/2x PNGs and safe inventory names", () => {
  const inventory = JSON.stringify({
    frames: [{ snapshots: { "1x": "test.png", "2x": "test@2x.png" } }],
  });
  const png = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).toString("base64");
  const bytes = Buffer.from(
    JSON.stringify({ inventory, files: { "test.png": png, "test@2x.png": png } }),
  );
  assert.equal(unpackSnapshotBundle(bytes, inventory, digest(bytes)).size, 2);
  assert.throws(() => unpackSnapshotBundle(bytes, inventory, null));
  assert.throws(() =>
    unpackSnapshotBundle(Buffer.concat([bytes, Buffer.from(" ")]), inventory, digest(bytes)),
  );
  assert.throws(() => unpackSnapshotBundle(bytes, `${inventory} `, digest(bytes)));
  const missing = Buffer.from(JSON.stringify({ inventory, files: { "test.png": png } }));
  assert.throws(() => unpackSnapshotBundle(missing, inventory, digest(missing)));
  const badInventory = JSON.stringify({ frames: [{ snapshots: { "1x": "../escape.png" } }] });
  const bad = Buffer.from(
    JSON.stringify({ inventory: badInventory, files: { "../escape.png": png } }),
  );
  assert.throws(() => unpackSnapshotBundle(bad, badInventory, digest(bad)));
});

test("release identities reject placeholders, mismatched tags and unsafe versions", () => {
  assert.equal(releaseVersion("refs/tags/v1.2.3", ["1.2.3", "1.2.3"]), "1.2.3");
  for (const [ref, versions] of [
    ["refs/heads/main", ["1.2.3"]],
    ["refs/tags/v0.0.0", ["0.0.0"]],
    ["refs/tags/v1.2.3", ["1.2.4"]],
    ["refs/tags/v01.2.3", ["01.2.3"]],
    ["refs/tags/v1.2.3-beta", ["1.2.3-beta"]],
    ["refs/tags/v1.2.3", []],
  ])
    assert.throws(() => releaseVersion(ref, versions));
});

test("GitHub evidence uses exact-SHA workflows, all pages and the latest run attempt", async () => {
  const paths = [];
  const result = await collectRuns(policy, "test/repo", sha, async (path) => {
    paths.push(path);
    if (path.includes("/workflows/"))
      return { total_count: 2, workflow_runs: [run(), run({ id: 44, run_attempt: 2 })] };
    return path.endsWith("page=1")
      ? { total_count: 101, jobs: Array.from({ length: 100 }, (_, i) => ({ name: String(i) })) }
      : { total_count: 101, jobs: [{ name: "last" }] };
  });
  assert.equal(result[0].jobs.length, 101);
  assert.ok(paths[0].includes(`head_sha=${sha}`));
  assert.ok(paths[1].includes("/44/attempts/2/jobs"));
  assert.equal(paths.length, 3);
  await assert.rejects(
    () =>
      collectRuns(policy, "test/repo", sha, async () => ({
        total_count: 2,
        workflow_runs: [run()],
      })),
    /Truncated/,
  );
  await assert.rejects(() =>
    collectRuns(policy, "test/repo", sha, async () => {
      throw Error("API unavailable");
    }),
  );
});

test("parity and browser acceptance cannot turn skipped, flaky or missing tests green", () => {
  requireParity({ total: 42, covered: 42, pending: [] });
  for (const result of [
    { total: 42, covered: 41, pending: [{}] },
    { total: 0, covered: 0, pending: [] },
    {},
  ])
    assert.throws(() => requireParity(result));
  requirePlaywright({ stats: { expected: 10, unexpected: 0, flaky: 0, skipped: 0 }, errors: [] });
  for (const stats of [
    { expected: 0, unexpected: 0, flaky: 0, skipped: 0 },
    { expected: 10, unexpected: 0, flaky: 0, skipped: 1 },
    { expected: 10, unexpected: 0, flaky: 1, skipped: 0 },
    {},
  ])
    assert.throws(() => requirePlaywright({ stats, errors: [] }));
});

test("signed inventory binds every payload, target, version and source; tampering and extra files fail", () => {
  const key = generateKeyPairSync("ed25519");
  const files = new Map([["vellum-linux-x64.tar.gz", Buffer.from("TEST DATA")]]);
  const envelope = sealArtifacts({ version: "1.2.3", sha, files }, key.privateKey);
  assert.equal(
    verifyArtifacts(envelope, key.publicKey, files, { version: "1.2.3", sha }).version,
    "1.2.3",
  );
  assert.throws(() =>
    verifyArtifacts(envelope, key.publicKey, new Map([...files, ["extra", Buffer.from("x")]]), {
      version: "1.2.3",
      sha,
    }),
  );
  assert.throws(() =>
    verifyArtifacts(
      envelope,
      key.publicKey,
      new Map([["vellum-linux-x64.tar.gz", Buffer.from("changed")]]),
      { version: "1.2.3", sha },
    ),
  );
  assert.throws(() => verifyArtifacts(envelope, key.publicKey, files, { version: "1.2.4", sha }));
  assert.throws(() =>
    verifyArtifacts(envelope, generateKeyPairSync("ed25519").publicKey, files, {
      version: "1.2.3",
      sha,
    }),
  );
  assert.throws(() =>
    sealArtifacts(
      { version: "1.2.3", sha, files: new Map([["../bad", Buffer.from("x")]]) },
      key.privateKey,
    ),
  );
  const changed = { ...envelope, payload: Buffer.from("{}").toString("base64") };
  assert.throws(() => verifyArtifacts(changed, key.publicKey, files, { version: "1.2.3", sha }));
});
