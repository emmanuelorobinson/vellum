import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const targets = {
  "darwin-arm64": ["aarch64-apple-darwin", "vellum", "macho", 0x100000c],
  "darwin-x64": ["x86_64-apple-darwin", "vellum", "macho", 0x1000007],
  "linux-x64": ["x86_64-unknown-linux-gnu", "vellum", "elf", 62],
  "linux-arm64": ["aarch64-unknown-linux-gnu", "vellum", "elf", 183],
  "win32-x64": ["x86_64-pc-windows-msvc", "vellum.exe", "pe", 0x8664],
  "win32-arm64": ["aarch64-pc-windows-msvc", "vellum.exe", "pe", 0xaa64],
};
export function targetInfo(target) {
  if (!Object.hasOwn(targets, target)) throw Error(`Unsupported VSIX target: ${target}`);
  const [triple, binary, format, machine] = targets[target];
  return { triple, binary, format, machine };
}
export function validateBinary(bytes, target) {
  const info = targetInfo(target);
  let machine;
  if (info.format === "macho" && bytes.length >= 32 && bytes.readUInt32LE(0) === 0xfeedfacf)
    machine = bytes.readUInt32LE(4);
  if (
    info.format === "elf" &&
    bytes.length >= 64 &&
    bytes.subarray(0, 6).equals(Buffer.from([127, 69, 76, 70, 2, 1]))
  )
    machine = bytes.readUInt16LE(18);
  if (info.format === "pe" && bytes.length >= 64 && bytes.toString("ascii", 0, 2) === "MZ") {
    const offset = bytes.readUInt32LE(60);
    if (offset + 6 <= bytes.length && bytes.toString("ascii", offset, offset + 4) === "PE\0\0")
      machine = bytes.readUInt16LE(offset + 4);
  }
  if (machine !== info.machine) throw Error(`Daemon binary does not match ${target}`);
}
export function packageArgs(target, output, preview = false) {
  targetInfo(target);
  return [
    "package",
    "--target",
    target,
    "--no-dependencies",
    "--out",
    output,
    ...(preview ? ["--skip-license"] : []),
  ];
}
export function publishArgs(registry, vsix) {
  if (registry === "marketplace") return ["publish", "--packagePath", vsix];
  if (registry === "open-vsx") return ["publish", vsix];
  throw Error("Choose marketplace or open-vsx");
}
const root = dirname(fileURLToPath(import.meta.url));
export function runTool(name, args, cwd = root) {
  // Run the pinned JS entrypoint, avoiding shell interpolation and Windows .cmd launchers.
  const entry = name === "vsce" ? "node_modules/@vscode/vsce/vsce" : "node_modules/ovsx/lib/ovsx";
  const result = spawnSync(process.execPath, [resolve(root, entry), ...args], {
    cwd,
    stdio: "inherit",
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
  if (result.status !== 0) throw Error(`${name} failed`);
}
async function main() {
  const [action, ...args] = process.argv.slice(2);
  if (action === "package") {
    const [target, binary, output, preview] = args;
    if (args.length < 3 || args.length > 4 || (preview && preview !== "--development-preview"))
      throw Error(
        "Usage: packaging.mjs package <target> <release-daemon> <output.vsix> [--development-preview]",
      );
    const manifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
    if (manifest.license === "UNLICENSED" && !preview)
      throw Error(
        "Choose the project license before packaging a release; --development-preview permits a private test VSIX",
      );
    validateBinary(await readFile(binary), target);
    const result = spawnSync(
      process.execPath,
      [resolve(root, "build.mjs"), "--target", target, "--daemon", resolve(binary)],
      { cwd: root, stdio: "inherit" },
    );
    if (result.status !== 0) throw Error("Extension build failed");
    runTool("vsce", packageArgs(target, resolve(output), !!preview), resolve(root, "dist/package"));
  } else if (action === "publish") {
    const [registry, vsix] = args;
    if (args.length !== 2 || !vsix.endsWith(".vsix"))
      throw Error("Usage: packaging.mjs publish <marketplace|open-vsx> <reviewed.vsix>");
    // Publishing is a separate explicit action; packaging and normal builds never invoke it.
    await readFile(vsix);
    const manifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
    if (manifest.version === "0.0.0" || manifest.license === "UNLICENSED")
      throw Error(
        "Publishing needs a release version, a project license and completed editor acceptance",
      );
    runTool(registry === "marketplace" ? "vsce" : "ovsx", publishArgs(registry, resolve(vsix)));
  } else throw Error("Choose package or publish");
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
