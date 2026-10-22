// Explicit publication entrypoint, called only after the protected Release workflow verifies evidence.
import { spawnSync } from "node:child_process";
import { createHash, createPublicKey } from "node:crypto";
import { lstatSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyRelease } from "../apps/desktop/src/main/release-signature.ts";
import { releaseVersion, verifyArtifacts } from "./lib/release.mjs";
import { payloadFiles } from "./release.mjs";
import { artifactNames, requireDistribution, targets } from "./release-packages.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
export function requireRemoteTag(object, sha, getTag) {
  for (let depth = 0; depth < 8; depth++) {
    if (!object || !/^[a-f0-9]{40}$/.test(object.sha)) throw Error("Invalid remote release tag");
    if (object.type === "commit") {
      if (object.sha !== sha) throw Error("Remote release tag moved");
      return;
    }
    if (object.type !== "tag") throw Error("Remote release tag is not a commit");
    object = getTag(object.sha).object;
  }
  throw Error("Remote tag chain exceeds limit");
}
export function feedOrder(target, names, destination) {
  if (
    !targets.some((t) => t.id === target) ||
    !/^s3:\/\/[a-z0-9][a-z0-9.-]{1,61}[a-z0-9](?:\/[\w.-]+)*\/$/.test(destination ?? "") ||
    destination.split("/").some((s) => s === "..") ||
    new Set(names).size !== names.length ||
    names.some((n) => !/^[\w.-]+$/.test(n) || n.startsWith("."))
  )
    throw Error("Invalid update feed destination/files");
  const t = targets.find((t) => t.id === target),
    envelope = `release-${t.platform}-${t.arch}.json`;
  const rank = (name) => (/^latest.*\.yml$/.test(name) ? 2 : name === envelope ? 1 : 0);
  if (
    !names.includes(envelope) ||
    !names.some((n) => rank(n) === 2) ||
    !names.some((n) => rank(n) === 0)
  )
    throw Error("Incomplete updater feed");
  return [...names]
    .sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))
    .map((name) => ({
      name,
      destination: `${destination}${target}/${name}`,
      cache: rank(name) === 0 ? "public,max-age=31536000,immutable" : "no-store",
    }));
}
function execute(command, args, env, input) {
  const result = spawnSync(command, args, {
    cwd: root,
    env,
    input,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  });
  // Provider output can contain request credentials. Reconstruct errors instead of echoing it.
  if (result.status !== 0 || result.error || result.signal)
    throw Error("Distribution publication failed");
  return result.stdout;
}
function main([mode, version]) {
  if (process.env.GITHUB_ACTIONS !== "true" || process.env.GITHUB_WORKFLOW !== "Release")
    throw Error("Publication requires the protected Release workflow");
  releaseVersion(process.env.GITHUB_REF, [version]);
  const dir = join(root, "target/release"),
    sha = process.env.GITHUB_SHA;
  const files = payloadFiles(dir);
  requireDistribution(files, version);
  verifyArtifacts(
    JSON.parse(readFileSync(join(dir, "release-inventory.json"))),
    createPublicKey(readFileSync(join(root, "target/release-public.pem"))),
    files,
    { version, sha },
  );
  const candidate = JSON.parse(files.get("candidate.json"));
  if (!candidate.ok || candidate.sha !== sha) throw Error("Candidate evidence mismatch");
  const base = { ...process.env };
  for (const key of ["VSCE_PAT", "OVSX_PAT", "HOMEBREW_TAP_TOKEN", "VELLUM_RELEASE_PRIVATE_KEY"])
    delete base[key];
  const awsEnv = { ...base };
  for (const key of ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN"])
    delete base[key];
  const gh = (args, env = base, input) => execute("gh", args, env, input);
  const tag = `v${version}`;
  const repo = process.env.GITHUB_REPOSITORY;
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo ?? "")) throw Error("Invalid publishing repository");
  const remote = JSON.parse(gh(["api", `repos/${repo}/git/ref/tags/${tag}`]));
  requireRemoteTag(remote.object, sha, (objectSha) =>
    JSON.parse(gh(["api", `repos/${repo}/git/tags/${objectSha}`])),
  );
  if (mode === "preflight") {
    if (
      !process.env.VSCE_PAT ||
      !process.env.OVSX_PAT ||
      !process.env.HOMEBREW_TAP_TOKEN ||
      !/^[\w.-]+\/[\w.-]+$/.test(process.env.HOMEBREW_TAP ?? "")
    )
      throw Error("Publishing credentials and owned tap must be provisioned before any write");
  }
  if (mode === "draft") {
    gh([
      "release",
      "create",
      tag,
      "--repo",
      process.env.GITHUB_REPOSITORY,
      "--verify-tag",
      "--draft",
      "--title",
      tag,
      "--generate-notes",
      ...[...files.keys(), "release-inventory.json"].map((name) => join(dir, name)),
    ]);
  } else if (mode === "npm") {
    for (const name of [
      ...targets.map((t) => artifactNames(version, t.id)[2]),
      `vellum-cli-${version}.tgz`,
    ])
      execute(
        "npm",
        [
          "publish",
          join(dir, name),
          "--access",
          "public",
          "--provenance",
          "--ignore-scripts",
          "--registry",
          "https://registry.npmjs.org",
        ],
        base,
      );
  } else if (mode === "extensions") {
    if (!process.env.VSCE_PAT || !process.env.OVSX_PAT)
      throw Error("Both extension registries require provisioned publishing credentials");
    for (const t of targets)
      for (const registry of ["marketplace", "open-vsx"]) {
        const key = registry === "marketplace" ? "VSCE_PAT" : "OVSX_PAT";
        execute(
          process.execPath,
          [
            join(root, "apps/extension/packaging.mjs"),
            "publish",
            registry,
            join(dir, artifactNames(version, t.id)[1]),
          ],
          { ...base, [key]: process.env[key] },
        );
      }
  } else if (mode === "feed" || mode === "preflight") {
    const feedRoot = join(
      root,
      mode === "preflight" ? "target/release-feed-preflight" : "target/release-feed",
    );
    mkdirSync(feedRoot, { recursive: true });
    for (const t of targets) {
      const archive = join(dir, artifactNames(version, t.id)[3]);
      const names = execute("tar", ["-tzf", archive], base).trim().split("\n");
      const order = feedOrder(t.id, names, process.env.VELLUM_UPDATE_BUCKET);
      const listing = execute("tar", ["-tvzf", archive], base).trim().split("\n");
      if (listing.length !== names.length || listing.some((line) => !line.startsWith("-")))
        throw Error("Updater archive must contain only regular files");
      const staging = join(feedRoot, t.id);
      mkdirSync(staging, { recursive: false });
      execute("tar", ["-xzf", archive, "-C", staging], base);
      const envelope = `release-${t.platform}-${t.arch}.json`;
      const release = verifyRelease(
        JSON.parse(readFileSync(join(staging, envelope))),
        readFileSync(join(root, "target/release-public.pem"), "utf8"),
        t.platform,
        t.arch,
      );
      if (release.version !== version) throw Error("Updater version mismatch");
      for (const name of names) {
        if (!lstatSync(join(staging, name)).isFile()) throw Error("Invalid updater file");
        if (name === envelope || /^latest.*\.yml$/.test(name)) continue;
        const bytes = readFileSync(join(staging, name)),
          signed = release.files.find((f) => f.url === name);
        if (
          !signed ||
          signed.size !== bytes.length ||
          signed.sha512 !== createHash("sha512").update(bytes).digest("base64")
        )
          throw Error("Updater artifact mismatch");
      }
      if (release.files.some((f) => !names.includes(f.url)))
        throw Error("Updater payload incomplete");
      if (mode === "feed")
        for (const file of order)
          execute(
            "aws",
            [
              "s3",
              "cp",
              join(staging, file.name),
              file.destination,
              "--only-show-errors",
              "--cache-control",
              file.cache,
            ],
            awsEnv,
          );
    }
  } else if (mode === "homebrew") {
    const tap = process.env.HOMEBREW_TAP;
    if (!/^[\w.-]+\/[\w.-]+$/.test(tap ?? "") || !process.env.HOMEBREW_TAP_TOKEN)
      throw Error("Owned Homebrew tap and publishing credential required");
    const env = { ...base, GH_TOKEN: process.env.HOMEBREW_TAP_TOKEN };
    // GET the directory rather than swallowing a failed GET for a missing file: auth/network failures stay fatal.
    const entries = JSON.parse(gh(["api", `repos/${tap}/contents/Formula`], env));
    if (!Array.isArray(entries)) throw Error("Invalid tap contents");
    const previous = entries.find((e) => e.name === "vellum.rb");
    const body = {
      message: `Release ${tag}`,
      content: files.get("vellum.rb").toString("base64"),
      ...(previous ? { sha: previous.sha } : {}),
    };
    gh(
      ["api", "--method", "PUT", `repos/${tap}/contents/Formula/vellum.rb`, "--input", "-"],
      env,
      JSON.stringify(body),
    );
  } else if (mode === "finalize")
    gh(["release", "edit", tag, "--repo", process.env.GITHUB_REPOSITORY, "--draft=false"]);
  else throw Error("Use preflight|draft|npm|extensions|feed|homebrew|finalize");
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch {
    console.error(
      "release-publish: blocked or failed; inspect the signed candidate and registry state before recovery",
    );
    process.exitCode = 1;
  }
}
