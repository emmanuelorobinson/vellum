#!/usr/bin/env node
// Prompt 69 release checks. No publishing, installation, credentials or advisory exceptions.
import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
export const fuzzTargets = [
  { name: "decode", dir: "crates/adapters/codec-json/fuzz" },
  { name: "merge_driver", dir: "crates/adapters/codec-json/fuzz" },
  { name: "merge", dir: "crates/vellum-core/fuzz" },
  { name: "op_application", dir: "crates/vellum-core/fuzz" },
  { name: "script_header", dir: "crates/vellum-core/fuzz" },
  { name: "daemon_frames", dir: "crates/adapters/ipc-local/fuzz" },
];

export function runSteps(steps, execute) {
  const results = steps.map((step) => {
    let result;
    try {
      result = execute(step);
    } catch {
      result = { status: null };
    }
    return { name: step.name, ok: result.status === 0 && !result.error && !result.signal };
  });
  return { ok: results.every((r) => r.ok), results };
}

/** Cargo's locked resolved graph, including workspace/build/dev/target dependencies. Not a binary attestation. */
export function cargoBom(metadata) {
  if (!metadata.resolve?.nodes?.length || !metadata.packages?.length)
    throw new Error("Cargo graph is incomplete");
  const packages = [...metadata.packages].sort((a, b) => a.id.localeCompare(b.id));
  const refs = new Map(packages.map((p, i) => [p.id, `cargo:${i}`]));
  const components = packages.map((p) => ({
    type: "library",
    "bom-ref": refs.get(p.id),
    name: p.name,
    version: p.version,
    ...(p.license ? { licenses: [{ expression: p.license }] } : {}),
    ...(p.source?.startsWith("registry+")
      ? { purl: `pkg:cargo/${encodeURIComponent(p.name)}@${encodeURIComponent(p.version)}` }
      : {}),
  }));
  const dependencies = metadata.resolve.nodes
    .map((n) => {
      if (!refs.has(n.id) || n.dependencies.some((id) => !refs.has(id)))
        throw new Error("Cargo graph has unknown dependencies");
      return {
        ref: refs.get(n.id),
        dependsOn: [...new Set(n.dependencies.map((id) => refs.get(id)))].sort(),
      };
    })
    .sort((a, b) => a.ref.localeCompare(b.ref));
  if (
    dependencies.length !== components.length ||
    new Set(dependencies.map((d) => d.ref)).size !== components.length
  )
    throw new Error("Cargo graph has missing or duplicate nodes");
  return { bomFormat: "CycloneDX", specVersion: "1.6", version: 1, components, dependencies };
}

function main(mode, toolchain = "nightly", runsArgument = "1000") {
  const out = join(root, "target/security");
  mkdirSync(out, { recursive: true });
  const env = { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" };
  const step = (name, command, args, extra = {}) => ({ name, command, args, ...extra });
  const cargo = (name, args) => step(name, "cargo", args);
  let steps;
  if (mode === "check") {
    steps = [
      cargo("rust-boundaries", [
        "test",
        "--locked",
        "-p",
        "vellum-engine-builtin",
        "-p",
        "vellum-script-quickjs",
        "-p",
        "vellum-ipc-local",
        "-p",
        "vellum-testkit",
      ]),
      cargo("op-corpus", ["test", "--locked", "-p", "vellum-core", "--test", "security_fuzz"]),
      step("host-boundaries", "pnpm", [
        "--filter",
        "@vellum/desktop",
        "--filter",
        "@vellum/extension",
        "test",
      ]),
      step("dependency-rules", "bash", ["scripts/check-deps.sh"]),
      cargo("cargo-deny", ["deny", "check"]),
      step("pnpm-audit", "pnpm", ["audit", "--json", "--audit-level", "low"]),
      step("checksum", "pnpm", ["checksum"]),
    ];
  } else if (mode === "sbom") {
    for (const name of ["cargo.cdx.json", "pnpm.cdx.json"])
      rmSync(join(out, name), { force: true });
    steps = [
      cargo("cargo-sbom", ["metadata", "--locked", "--offline", "--format-version", "1"]),
      step("pnpm-sbom", "pnpm", [
        "sbom",
        "--sbom-format",
        "cyclonedx",
        "--lockfile-only",
        "--out",
        join(out, "pnpm.cdx.json"),
      ]),
    ];
  } else if (mode === "fuzz") {
    if (!/^nightly(?:-\d{4}-\d{2}-\d{2})?$/.test(toolchain))
      throw new Error("Use a nightly fuzz toolchain");
    const runs = Number(runsArgument);
    if (!Number.isSafeInteger(runs) || runs < 1 || runs > 10_000_000)
      throw new Error("Fuzz runs must be an integer from 1 to 10000000");
    steps = fuzzTargets.map((t) => {
      const corpus = join(out, "corpus", t.name);
      const artifacts = join(out, "artifacts", t.name);
      mkdirSync(corpus, { recursive: true });
      mkdirSync(artifacts, { recursive: true });
      writeFileSync(
        join(corpus, "seed"),
        t.name === "daemon_frames"
          ? '\x01{}\n{"v":1,"token":"TEST DATA","channel":"mcp"}\n'
          : t.name === "script_header"
            ? "// @vellum\n// TEST DATA\nreturn [];\n"
            : "TEST DATA",
      );
      if (["decode", "merge_driver"].includes(t.name)) {
        for (const name of readdirSync(join(root, "fixtures/documents")).filter((n) =>
          n.endsWith(".vellum"),
        )) {
          const bytes = readFileSync(join(root, "fixtures/documents", name));
          writeFileSync(
            join(corpus, name),
            t.name === "merge_driver" ? Buffer.concat([Buffer.from([1]), bytes]) : bytes,
          );
        }
      }
      return cargo(t.name, [
        `+${toolchain}`,
        "fuzz",
        "run",
        "--fuzz-dir",
        t.dir,
        t.name,
        corpus,
        "--",
        `-runs=${runs}`,
        "-seed=69",
        "-max_len=8192",
        "-max_total_time=20",
        "-timeout=10",
        "-rss_limit_mb=2048",
        `-artifact_prefix=${artifacts}/`,
      ]);
    });
    env.CARGO_TARGET_DIR = join(root, "target/security-fuzz");
  } else {
    throw new Error("Use security.mjs check|sbom|fuzz");
  }
  const result = runSteps(steps, (s) => {
    process.stdout.write(`security: ${s.name}\n`);
    const r = spawnSync(s.command, s.args, {
      cwd: root,
      env,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      timeout: 15 * 60 * 1000,
    });
    // Persist even failed checks for review. Never run this against real provider credentials.
    if (mode === "sbom" && s.name === "cargo-sbom" && r.status === 0) {
      writeFileSync(
        join(out, "cargo.cdx.json"),
        `${JSON.stringify(cargoBom(JSON.parse(r.stdout)), null, 2)}\n`,
      );
    } else
      writeFileSync(
        join(out, `${s.name}.log`),
        `${r.stdout ?? ""}${r.stderr ?? ""}${r.error ? "\nExecution failed or timed out\n" : ""}`,
      );
    process.stdout.write(
      `security: ${s.name} ${r.status === 0 && !r.error ? "passed" : "FAILED"}\n`,
    );
    return r;
  });
  writeFileSync(join(out, `${mode}.json`), `${JSON.stringify(result, null, 2)}\n`);
  process.exitCode = result.ok ? 0 : 1;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv[2], process.argv[3], process.argv[4]);
  } catch (e) {
    process.stderr.write(`security: ${e.message}\n`);
    process.exitCode = 1;
  }
}
