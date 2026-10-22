// Native signing uses identities already provisioned in the runner's OS keychain/certificate store.
import { spawnSync } from "node:child_process";
import { createPrivateKey, createPublicKey } from "node:crypto";
import { copyFileSync, mkdirSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateBinary } from "../apps/extension/packaging.mjs";
import { releaseVersion } from "./lib/release.mjs";
import { artifactNames, packNpm, stageNpm, targets } from "./release-packages.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
export function signingPlan(target, binary, env) {
  const t = targets.find((t) => t.id === target);
  if (!t) throw Error("Unsupported signing target");
  if (t.platform === "darwin") {
    if (!env.CSC_NAME || !env.APPLE_KEYCHAIN_PROFILE)
      throw Error("macOS OS keychain identity and notarization profile required");
    return [
      [
        "codesign",
        ["--force", "--options", "runtime", "--timestamp", "--sign", env.CSC_NAME, binary],
      ],
      ["codesign", ["--verify", "--strict", binary]],
    ];
  }
  if (t.platform === "win32") {
    if (!/^[a-fA-F0-9]{40}$/.test(env.VELLUM_WINDOWS_CERT_SHA1 ?? ""))
      throw Error("Windows certificate-store identity required");
    const url = new URL(env.VELLUM_TIMESTAMP_URL);
    if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash)
      throw Error("Invalid timestamp service");
    return [
      [
        "signtool",
        [
          "sign",
          "/sha1",
          env.VELLUM_WINDOWS_CERT_SHA1,
          "/fd",
          "SHA256",
          "/tr",
          url.href,
          "/td",
          "SHA256",
          binary,
        ],
      ],
      ["signtool", ["verify", "/pa", binary]],
    ];
  }
  return []; // Linux payloads are signed by the required Ed25519 inventory and update envelope.
}
function run(command, args, options = {}) {
  const env = { ...process.env, COPYFILE_DISABLE: "1" };
  delete env.VELLUM_RELEASE_PRIVATE_KEY;
  const result = spawnSync(command, args, { cwd: root, env, stdio: "pipe", ...options });
  if (result.status !== 0 || result.error || result.signal)
    throw Error("Native signing/packaging command failed");
  return result.stdout;
}
async function main([mode, version, target, binaryArg]) {
  releaseVersion(`refs/tags/v${version}`, [version]);
  const t = targets.find((t) => t.id === target);
  if (!t || process.platform !== t.platform || process.arch !== t.arch)
    throw Error("Signing requires the matching native runner");
  const binary = resolve(binaryArg);
  validateBinary(readFileSync(binary), target);
  const output = join(root, "target/release-native", target);
  mkdirSync(output, { recursive: true });
  if (mode === "sign") {
    for (const [cmd, args] of signingPlan(target, binary, process.env)) run(cmd, args);
    if (t.platform === "darwin") {
      const zip = join(root, "target", `cli-notarization-${target}.zip`);
      run("ditto", ["-c", "-k", binary, zip]);
      const receipt = run("xcrun", [
        "notarytool",
        "submit",
        zip,
        "--keychain-profile",
        process.env.APPLE_KEYCHAIN_PROFILE,
        "--wait",
        "--output-format",
        "json",
      ]);
      if (JSON.parse(receipt).status !== "Accepted")
        throw Error("CLI notarization was not accepted");
    }
    return;
  }
  if (mode !== "bundle")
    throw Error("Use release-native.mjs sign|bundle <version> <target> <binary>");
  let pem = Buffer.alloc(0);
  try {
    for await (const chunk of process.stdin) {
      if (pem.length + chunk.length > 16384) throw Error("Signing input too large");
      pem = Buffer.concat([pem, chunk]);
    }
    const key = createPrivateKey(pem);
    const publicKey = createPublicKey(readFileSync(process.env.VELLUM_UPDATE_PUBLIC_KEY_FILE));
    if (
      key.asymmetricKeyType !== "ed25519" ||
      !createPublicKey(key)
        .export({ type: "spki", format: "der" })
        .equals(publicKey.export({ type: "spki", format: "der" }))
    )
      throw Error("Update signing identity mismatch");
    const desktop = join(root, "target/desktop-packages");
    const names = readdirSync(desktop).filter((n) =>
      /\.(?:dmg|zip|exe|AppImage|deb|tar\.gz|blockmap)$/.test(n),
    );
    const required = {
      darwin: [".dmg", ".zip"],
      win32: [".exe"],
      linux: [".AppImage", ".deb", ".tar.gz"],
    }[t.platform];
    if (required.some((ext) => !names.some((n) => n.endsWith(ext))) || names.length > 8)
      throw Error("Missing desktop distributions");
    const metadata = readdirSync(desktop).filter((n) => /^latest[^/]*\.yml$/.test(n));
    if (!metadata.length) throw Error("Missing updater metadata");
    const envelope = `release-${t.platform}-${t.arch}.json`;
    run(
      process.execPath,
      [
        join(root, "apps/desktop/sign-release.mjs"),
        "--version",
        version,
        "--platform",
        t.platform,
        "--arch",
        t.arch,
        "--out",
        join(desktop, envelope),
        ...names.flatMap((n) => ["--artifact", join(desktop, n)]),
      ],
      { input: pem },
    );
    const [cliArchive, , npmArchive, desktopArchive] = artifactNames(version, target);
    run("tar", [
      "-czf",
      join(output, desktopArchive),
      "-C",
      desktop,
      ...names,
      ...metadata,
      envelope,
    ]);
    const cli = join(root, "target/release-cli", target);
    mkdirSync(cli, { recursive: true });
    copyFileSync(binary, join(cli, t.platform === "win32" ? "vellum.exe" : "vellum"));
    for (const name of ["LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.txt"])
      copyFileSync(join(root, "apps/extension/dist/package", name), join(cli, name));
    run("tar", ["-czf", join(output, cliArchive), "-C", cli, "."]);
    const npm = join(root, "target/release-npm", target);
    stageNpm(npm, version, target, binary);
    if (packNpm(npm, output) !== npmArchive) throw Error("Unexpected native npm package name");
  } finally {
    pem.fill(0);
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main(process.argv.slice(2));
  } catch {
    console.error("release-native: native identity, signing or artifact validation failed");
    process.exitCode = 1;
  }
}
