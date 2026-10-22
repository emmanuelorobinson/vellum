// Dependency rules (guide Prompt 2, ARCHITECTURE §2.1). The pure Rust crates keep an allowlist of
// direct dependencies and no vendor crate may reach them; dependency-cruiser keeps editor-core headless
// and vendor SDKs behind the wrapper packages. Each negative case runs against a scratch workspace whose
// vendor crates are path stubs, so nothing here needs the network.
import assert from "node:assert/strict";
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { at, bash, discard, put, run, scratch } from "./helpers.mjs";

const crate = (name, deps = "") =>
  `[package]\nname = "${name}"\nversion = "0.0.0"\nedition = "2024"\n\n[dependencies]\n${deps}`;

// Creates a workspace: the two pure crates (core's dependencies are `coreDeps`) plus path stubs
// named like vendor crates. Copies the repository's allowlist in, so the rules are the real ones.
function rustWorkspace(coreDeps = "") {
  const root = scratch("deps-rust");
  put(
    root,
    "Cargo.toml",
    [
      "[workspace]",
      'resolver = "3"',
      'members = ["crates/vellum-types", "crates/vellum-core", "stubs/loro", "stubs/tokio", "stubs/helper"]',
      "",
    ].join("\n"),
  );
  put(root, "crates/vellum-types/Cargo.toml", crate("vellum-types"));
  put(root, "crates/vellum-types/src/lib.rs", "");
  put(root, "crates/vellum-core/Cargo.toml", crate("vellum-core", coreDeps));
  put(root, "crates/vellum-core/src/lib.rs", "");
  put(root, "stubs/loro/Cargo.toml", crate("loro"));
  put(root, "stubs/loro/src/lib.rs", "");
  put(root, "stubs/tokio/Cargo.toml", crate("tokio"));
  put(root, "stubs/tokio/src/lib.rs", "");
  put(root, "stubs/helper/Cargo.toml", crate("helper", 'tokio = { path = "../tokio" }\n'));
  put(root, "stubs/helper/src/lib.rs", "");
  mkdirSync(join(root, "scripts"), { recursive: true });
  copyFileSync(
    at("scripts", "rust-deps-allowlist.txt"),
    join(root, "scripts", "rust-deps-allowlist.txt"),
  );
  const lock = run("cargo", ["generate-lockfile", "--manifest-path", join(root, "Cargo.toml")]);
  assert.equal(lock.status, 0, lock.output);
  return root;
}

// A tree with packages/ and apps/ and stub copies of the vendor modules dependency-cruiser resolves.
function tsTree(files) {
  const root = scratch("deps-ts");
  copyFileSync(at(".dependency-cruiser.cjs"), join(root, ".dependency-cruiser.cjs"));
  for (const name of ["react", "better-auth"]) {
    put(root, `node_modules/${name}/package.json`, JSON.stringify({ name, main: "index.js" }));
    put(root, `node_modules/${name}/index.js`, "module.exports = {};\n");
  }
  for (const [file, text] of Object.entries(files)) put(root, file, text);
  return root;
}

const REACT_IMPORT = 'import { useState } from "react";\n\nexport const state = useState;\n';
const AUTH_IMPORT =
  'import { betterAuth } from "better-auth";\n\nexport const auth = betterAuth;\n';

test("the real repository satisfies the Rust dependency allowlist", () => {
  const result = bash("check-deps.sh", ["rust"]);
  assert.equal(result.status, 0, result.output);
});

test("cargo-deny failure propagates instead of printing success", () => {
  const root = scratch("deny-exit");
  try {
    put(root, "bin/cargo", '#!/bin/sh\nif [ "$2" = "--version" ]; then exit 0; fi\nexit 42\n');
    chmodSync(join(root, "bin/cargo"), 0o755);
    const result = bash("check-deps.sh", ["deny", "--root", root], {
      env: { ...process.env, PATH: `${join(root, "bin")}:${process.env.PATH}` },
    });
    assert.notEqual(result.status, 0, "a failed policy check must fail the gate");
    assert.doesNotMatch(result.output, /sources pass/);
  } finally {
    discard(root);
  }
});

test("the real repository satisfies the dependency-cruiser rules", () => {
  const result = bash("check-deps.sh", ["ts"]);
  assert.equal(result.status, 0, result.output);
});

test("a pure crate with no dependencies passes", () => {
  const root = rustWorkspace();
  try {
    const result = bash("check-deps.sh", ["rust", "--root", root]);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(root);
  }
});

test("adding loro to vellum-core fails and names both crates", () => {
  const root = rustWorkspace('loro = { path = "../../stubs/loro" }\n');
  try {
    const result = bash("check-deps.sh", ["rust", "--root", root]);
    assert.notEqual(result.status, 0, "loro in vellum-core must fail the build");
    assert.match(result.output, /vellum-core/);
    assert.match(result.output, /loro/);
  } finally {
    discard(root);
  }
});

test("a vendor crate reached through another dependency fails too", () => {
  const root = rustWorkspace('helper = { path = "../../stubs/helper" }\n');
  try {
    const result = bash("check-deps.sh", ["rust", "--root", root]);
    assert.notEqual(result.status, 0, "tokio in the closure of vellum-core must fail");
    assert.match(result.output, /tokio/);
  } finally {
    discard(root);
  }
});

test("editor-core importing React fails the dependency-cruiser rules", () => {
  const root = tsTree({
    "packages/editor-core/src/bad.ts": REACT_IMPORT,
    "packages/editor-core/src/ok.ts": "export const ok = 1;\n",
  });
  try {
    const result = bash("check-deps.sh", ["ts", "--root", root]);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /editor-core-stays-headless/);
    assert.match(result.output, /packages\/editor-core\/src\/bad\.ts/);
  } finally {
    discard(root);
  }
});

test("an app importing better-auth directly fails; the auth wrapper may import it", () => {
  const app = tsTree({ "apps/web/app/auth.ts": AUTH_IMPORT });
  try {
    const result = bash("check-deps.sh", ["ts", "--root", app]);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /apps\/web\/app\/auth\.ts/);
  } finally {
    discard(app);
  }

  const wrapper = tsTree({ "packages/auth/src/index.ts": AUTH_IMPORT });
  try {
    const result = bash("check-deps.sh", ["ts", "--root", wrapper]);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(wrapper);
  }
});
