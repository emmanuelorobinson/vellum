// Build/stage distribution packages. No registry writes or implicit downloads.
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brand } from "../packages/config/src/index.ts";
import { digest, releaseVersion } from "./lib/release.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
export const targets = [
  { id: "darwin-arm64", platform: "darwin", arch: "arm64", triple: "aarch64-apple-darwin" },
  { id: "darwin-x64", platform: "darwin", arch: "x64", triple: "x86_64-apple-darwin" },
  { id: "win32-x64", platform: "win32", arch: "x64", triple: "x86_64-pc-windows-msvc" },
  { id: "win32-arm64", platform: "win32", arch: "arm64", triple: "aarch64-pc-windows-msvc" },
  { id: "linux-x64", platform: "linux", arch: "x64", triple: "x86_64-unknown-linux-gnu" },
  { id: "linux-arm64", platform: "linux", arch: "arm64", triple: "aarch64-unknown-linux-gnu" },
];
export function artifactNames(version, target) {
  npmManifest(version, target);
  return [
    `vellum-${version}-${target}.tar.gz`,
    `vellum-${version}-${target}.vsix`,
    `vellum-cli-${target}-${version}.tgz`,
    `vellum-desktop-${version}-${target}.tar.gz`,
  ];
}
export function distributionNames(version) {
  return [
    ...targets.flatMap((t) => artifactNames(version, t.id)),
    `vellum-cli-${version}.tgz`,
    "vellum.rb",
    "cargo.cdx.json",
    "pnpm.cdx.json",
    "candidate.json",
  ].sort();
}
export function requireDistribution(files, version) {
  if ([...files.keys()].sort().join("\n") !== distributionNames(version).join("\n"))
    throw Error("Incomplete or unexpected release distribution");
}
export function npmManifest(version, target) {
  releaseVersion(`refs/tags/v${version}`, [version]);
  const repository = JSON.parse(readFileSync(join(root, "apps/extension/package.json"))).repository;
  const common = {
    version,
    license: "Apache-2.0",
    description: `${brand.name} CLI and local daemon`,
    repository: { type: "git", url: repository.url },
    files: ["bin", "LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.txt"],
  };
  if (target) {
    const t = targets.find((t) => t.id === target);
    if (!t) throw Error("Unsupported npm native target");
    return {
      ...common,
      name: `@vellum/cli-${target}`,
      os: [t.platform],
      cpu: [t.arch],
      ...(t.platform === "linux" ? { libc: ["glibc"] } : {}),
    };
  }
  return {
    ...common,
    name: "@vellum/cli",
    type: "module",
    bin: { vellum: "bin/vellum.mjs" },
    engines: { node: ">=24" },
    optionalDependencies: Object.fromEntries(targets.map((t) => [`@vellum/cli-${t.id}`, version])),
  };
}
export const wrapperSource = `#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
const require = createRequire(import.meta.url);
let binary;
try {
  const pkg = require.resolve("@vellum/cli-" + process.platform + "-" + process.arch + "/package.json");
  binary = join(dirname(pkg), "bin", process.platform === "win32" ? "vellum.exe" : "vellum");
} catch {
  console.error(${JSON.stringify(`${brand.name} platform package is missing. Install with optional dependencies enabled on a supported x64 or arm64 platform.`)});
  process.exit(1);
}
const child = spawn(binary, process.argv.slice(2), { stdio: "inherit", shell: false });
const handlers = new Map(["SIGINT", "SIGTERM", "SIGHUP"].map((signal) => {
  const handler = () => child.kill(signal);
  process.on(signal, handler);
  return [signal, handler];
}));
child.on("error", () => { console.error(${JSON.stringify(`${brand.name} native executable could not start.`)}); process.exitCode = 1; });
child.on("exit", (code, signal) => {
  for (const [name, handler] of handlers) process.removeListener(name, handler);
  if (signal) process.kill(process.pid, signal);
  else process.exitCode = code ?? 1;
});
`;

export function homebrewFormula(repository, version, hashes) {
  releaseVersion(`refs/tags/v${version}`, [version]);
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw Error("Invalid repository");
  const section = (platform) =>
    `  on_${platform === "darwin" ? "macos" : "linux"} do\n${["arm64", "x64"]
      .map((arch) => {
        const id = `${platform}-${arch}`,
          hash = hashes[id];
        if (!/^[a-f0-9]{64}$/.test(hash)) throw Error("Missing Homebrew archive digest");
        return `    on_${arch === "arm64" ? "arm" : "intel"} do\n      url "https://github.com/${repository}/releases/download/v${version}/vellum-${version}-${id}.tar.gz"\n      sha256 "${hash}"\n    end`;
      })
      .join("\n")}\n  end`;
  return `class Vellum < Formula\n  desc ${JSON.stringify(brand.tagline)}\n  homepage "https://github.com/${repository}"\n  version "${version}"\n  license "Apache-2.0"\n${section("darwin")}\n${section("linux")}\n  def install\n    bin.install "vellum"\n  end\n  test do\n    assert_match version.to_s, shell_output("#{bin}/vellum --version")\n  end\nend\n`;
}

export function stageNpm(dir, version, target, binary) {
  mkdirSync(join(dir, "bin"), { recursive: true });
  writeFileSync(
    join(dir, "package.json"),
    `${JSON.stringify(npmManifest(version, target), null, 2)}\n`,
  );
  if (target) {
    const name = target.startsWith("win32") ? "vellum.exe" : "vellum";
    copyFileSync(binary, join(dir, "bin", name));
    chmodSync(join(dir, "bin", name), 0o755);
  } else {
    writeFileSync(join(dir, "bin/vellum.mjs"), wrapperSource, { mode: 0o755 });
  }
  for (const file of ["LICENSE", "THIRD_PARTY_NOTICES.md"])
    copyFileSync(join(root, file), join(dir, file));
  if (target)
    copyFileSync(
      join(root, "apps/extension/dist/package/THIRD_PARTY_LICENSES.txt"),
      join(dir, "THIRD_PARTY_LICENSES.txt"),
    );
}
export function packNpm(dir, output) {
  // No lifecycle scripts are present, and pack never contacts a registry.
  const args = [
    "pack",
    "--ignore-scripts",
    "--json",
    "--pack-destination",
    output,
    "--cache",
    resolve(output, "../npm-cache"),
  ];
  const env = { ...process.env };
  for (const name of ["VELLUM_RELEASE_PRIVATE_KEY", "VSCE_PAT", "OVSX_PAT", "HOMEBREW_TAP_TOKEN"])
    delete env[name];
  const result =
    process.platform === "win32"
      ? spawnSync(
          process.execPath,
          [join(dirname(process.execPath), "node_modules/npm/bin/npm-cli.js"), ...args],
          { cwd: dir, env, encoding: "utf8", shell: false },
        )
      : spawnSync("npm", args, { cwd: dir, env, encoding: "utf8", shell: false });
  if (result.status !== 0) throw Error("npm package staging failed");
  return JSON.parse(result.stdout)[0].filename;
}
function main([mode, version, input, output, repository]) {
  releaseVersion(`refs/tags/v${version}`, [version]);
  if (mode !== "assemble")
    throw Error(
      "Use release-packages.mjs assemble <version> <download-dir> <output-dir> <repository>",
    );
  mkdirSync(output, { recursive: true });
  const expected = targets.flatMap((t) => artifactNames(version, t.id));
  const found = [];
  for (const dir of readdirSync(input)) {
    if (!/^native-/.test(dir)) continue;
    for (const name of readdirSync(join(input, dir))) {
      if (!expected.includes(name) || found.includes(name))
        throw Error("Unexpected/duplicate native output");
      found.push(name);
      copyFileSync(join(input, dir, name), join(output, name));
    }
  }
  if (found.length !== expected.length) throw Error("Six-target release output is incomplete");
  const hashes = Object.fromEntries(
    targets
      .filter((t) => t.platform !== "win32")
      .map((t) => [t.id, digest(readFileSync(join(output, artifactNames(version, t.id)[0])))]),
  );
  writeFileSync(join(output, "vellum.rb"), homebrewFormula(repository, version, hashes));
  const wrapper = join(output, "../npm-wrapper");
  stageNpm(wrapper, version);
  packNpm(wrapper, resolve(output));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch {
    console.error("release-packages: incomplete or invalid release inputs");
    process.exitCode = 1;
  }
}
