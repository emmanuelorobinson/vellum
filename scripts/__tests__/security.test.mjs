// TEST DATA. The gate must keep every failure and produce dependency graphs without host paths.
import assert from "node:assert/strict";
import { test } from "node:test";
import { cargoBom, fuzzTargets, runSteps } from "../security.mjs";

test("security gate fails closed on nonzero, signal or missing executable and runs later checks", () => {
  const seen = [];
  const results = runSteps(
    ["deny", "audit", "canary"].map((name) => ({ name, command: name, args: [] })),
    (step) => {
      seen.push(step.name);
      return step.name === "deny" ? { status: 42 } : { status: 0 };
    },
  );
  assert.equal(results.ok, false);
  assert.deepEqual(seen, ["deny", "audit", "canary"]);
  for (const result of [
    { status: null, signal: "SIGTERM" },
    { error: new Error("missing") },
    { status: 0, error: new Error("spawn") },
  ])
    assert.equal(runSteps([{ name: "missing" }], () => result).ok, false);
});

test("fuzz gate covers the codecs, byte merge driver, op application, header and daemon frames", () => {
  assert.deepEqual(fuzzTargets.map((t) => t.name).sort(), [
    "daemon_frames",
    "decode",
    "merge",
    "merge_driver",
    "op_application",
    "script_header",
  ]);
});

test("Cargo CycloneDX SBOM represents every resolved package and edge without absolute paths", () => {
  const metadata = {
    packages: [
      {
        id: "path+file:///private/project#app@0.0.0",
        name: "app",
        version: "0.0.0",
        license: "Apache-2.0",
        source: null,
      },
      {
        id: "registry+https://github.com/rust-lang/crates.io-index#dep@1.0.0",
        name: "dep",
        version: "1.0.0",
        license: "MIT OR Apache-2.0",
        source: "registry+https://github.com/rust-lang/crates.io-index",
      },
    ],
    resolve: {
      nodes: [
        {
          id: "path+file:///private/project#app@0.0.0",
          dependencies: ["registry+https://github.com/rust-lang/crates.io-index#dep@1.0.0"],
        },
        { id: "registry+https://github.com/rust-lang/crates.io-index#dep@1.0.0", dependencies: [] },
      ],
    },
  };
  const bom = cargoBom(metadata);
  assert.equal(bom.bomFormat, "CycloneDX");
  assert.equal(bom.specVersion, "1.6");
  assert.equal(bom.components.length, 2);
  const dep = bom.components.find((c) => c.name === "dep");
  assert.equal(dep.purl, "pkg:cargo/dep@1.0.0");
  assert.deepEqual(
    bom.dependencies.find((d) => d.ref === bom.components.find((c) => c.name === "app")["bom-ref"])
      .dependsOn,
    [dep["bom-ref"]],
  );
  assert.doesNotMatch(JSON.stringify(bom), /private\/project|file:\/\//);
  assert.throws(() => cargoBom({ ...metadata, resolve: null }));
  assert.throws(() =>
    cargoBom({ ...metadata, resolve: { nodes: [{ id: "missing", dependencies: [] }] } }),
  );
});
