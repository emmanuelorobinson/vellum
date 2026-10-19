import { spawnSync } from "node:child_process";
import { createPublicKey } from "node:crypto";
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brand } from "@vellum/config";
import { validateBinary } from "../extension/packaging.mjs";

const root = fileURLToPath(new URL(".", import.meta.url));
export function builderConfig({
  preview,
  version,
  productName,
  feed,
  publicKey,
  publisher,
  signed,
  notarize,
  accepted,
}) {
  if (!preview) {
    if (
      !/^\d+\.\d+\.\d+$/.test(version) ||
      version === "0.0.0" ||
      !feed ||
      !publicKey ||
      !publisher ||
      !signed ||
      !notarize ||
      !accepted
    )
      throw Error(
        "A release needs a version, update feed, trusted key, publisher, signing/notarization credentials and completed runtime/visual acceptance",
      );
    const url = new URL(feed);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      !url.pathname.endsWith("/") ||
      createPublicKey(publicKey).asymmetricKeyType !== "ed25519"
    )
      throw Error("Invalid release trust configuration");
  }
  return {
    appId: brand.desktopAppId,
    productName,
    executableName: "vellum-desktop",
    directories: { app: "dist/package", output: "../../target/desktop-packages" },
    files: [
      "main/**",
      "preload/**",
      "renderer/**",
      "LICENSE",
      "THIRD_PARTY_NOTICES.md",
      "THIRD_PARTY_LICENSES.txt",
      "!node_modules/**",
    ],
    npmRebuild: false,
    asar: true,
    extraResources: [
      { from: "dist/bin", to: "bin" },
      ...(!preview ? [{ from: "dist/desktop-release.json", to: "desktop-release.json" }] : []),
    ],
    forceCodeSigning: !preview,
    // biome-ignore lint/suspicious/noTemplateCurlyInString: electron-builder expands these placeholders.
    artifactName: "${productName}-${version}-${os}-${arch}.${ext}",
    publish: preview ? null : { provider: "generic", url: feed },
    mac: {
      identity: preview ? null : undefined,
      category: "public.app-category.graphics-design",
      hardenedRuntime: true,
      notarize: !preview,
      target: [
        { target: "dmg", arch: ["arm64", "x64"] },
        { target: "zip", arch: ["arm64", "x64"] },
      ],
      signIgnore: [],
    },
    win: {
      verifyUpdateCodeSignature: true,
      signtoolOptions: { publisherName: publisher },
      target: [{ target: "nsis", arch: ["x64", "arm64"] }],
    },
    linux: {
      category: "Graphics",
      target: ["AppImage", "deb", "tar.gz"].map((target) => ({ target, arch: ["x64", "arm64"] })),
    },
  };
}
function run(script, args) {
  const result = spawnSync(process.execPath, [script, ...args], {
    cwd: root,
    stdio: "inherit",
    shell: false,
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
  if (result.error || result.status !== 0) throw Error("Desktop packaging failed");
}
export function nativeConfig(config, platform, arch) {
  const key = { darwin: "mac", win32: "win", linux: "linux" }[platform];
  if (!key || !["arm64", "x64"].includes(arch)) throw Error("Unsupported native target");
  const result = structuredClone(config);
  for (const target of result[key].target) target.arch = [arch];
  return result;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const value = (name) => args[args.indexOf(name) + 1];
  const preview = args.includes("--development-preview");
  const platform = value("--platform"),
    arch = value("--arch"),
    daemon = value("--daemon");
  if (
    !args.includes("--platform") ||
    !args.includes("--arch") ||
    !args.includes("--daemon") ||
    !["darwin", "win32", "linux"].includes(platform) ||
    !["x64", "arm64"].includes(arch)
  )
    throw Error(
      "Usage: packaging.mjs --platform darwin|win32|linux --arch x64|arm64 --daemon <native binary> [--development-preview] [--dir]",
    );
  validateBinary(await readFile(resolve(daemon)), `${platform}-${arch}`);
  const manifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
  const publicKey = process.env.VELLUM_UPDATE_PUBLIC_KEY_FILE
    ? await readFile(process.env.VELLUM_UPDATE_PUBLIC_KEY_FILE, "utf8")
    : undefined;
  // No credential values are printed or written into packaged resources.
  const config = builderConfig({
    preview,
    version: manifest.version,
    productName: brand.name,
    feed: process.env.VELLUM_UPDATE_URL,
    publicKey,
    publisher: process.env.VELLUM_WINDOWS_PUBLISHER,
    signed: !!process.env.CSC_LINK || !!process.env.CSC_NAME,
    notarize:
      !!process.env.APPLE_API_KEY ||
      (!!process.env.APPLE_ID &&
        !!process.env.APPLE_APP_SPECIFIC_PASSWORD &&
        !!process.env.APPLE_TEAM_ID),
    accepted: process.env.VELLUM_DESKTOP_ACCEPTANCE === "complete",
  });
  run(resolve(root, "build.mjs"), ["--target", `${platform}-${arch}`, "--daemon", resolve(daemon)]);
  await mkdir(resolve(root, "dist/package"), { recursive: true });
  for (const name of ["main", "preload", "renderer"])
    await cp(resolve(root, "dist", name), resolve(root, "dist/package", name), { recursive: true });
  for (const name of ["LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.txt"])
    await cp(resolve(root, "dist", name), resolve(root, "dist/package", name));
  await writeFile(
    resolve(root, "dist/package/package.json"),
    JSON.stringify({
      name: "vellum-desktop",
      productName: config.productName,
      version: manifest.version,
      description: manifest.description,
      license: "Apache-2.0",
      main: "main/index.cjs",
      author: manifest.author ?? config.productName,
    }),
  );
  if (!preview)
    await writeFile(
      resolve(root, "dist/desktop-release.json"),
      JSON.stringify({
        feed: process.env.VELLUM_UPDATE_URL,
        publicKey,
        publisher: process.env.VELLUM_WINDOWS_PUBLISHER,
      }),
    );
  await writeFile(
    resolve(root, "dist/builder.json"),
    JSON.stringify(nativeConfig(config, platform, arch)),
  );
  const cli = fileURLToPath(import.meta.resolve("electron-builder/cli.js"));
  run(cli, [
    "--config",
    "dist/builder.json",
    `--${platform === "darwin" ? "mac" : platform === "win32" ? "win" : "linux"}`,
    `--${arch}`,
    "--publish",
    "never",
    ...(args.includes("--dir") ? ["--dir"] : []),
  ]);
}
