#!/usr/bin/env node
// Release orchestration is deliberately separate from builds. No mode publishes a release.
import { createPrivateKey, createPublicKey } from "node:crypto";
import { lstatSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkCoverage } from "./lib/parity.mjs";
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
  snapshotNames,
  unpackSnapshotBundle,
  verifyArtifacts,
} from "./lib/release.mjs";
import { requireDistribution } from "./release-packages.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const read = (p) => readFileSync(join(root, p), "utf8");
export const policy = JSON.parse(read("scripts/release-policy.json"));

/** Inject the API boundary in tests. All pages are required; API errors/truncation fail closed. */
export async function collectRuns(p, repository, sha, request) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository) || !/^[a-f0-9]{40}$/.test(sha))
    throw Error("Invalid candidate");
  const pageAll = async (path, field) => {
    const all = [];
    for (let page = 1; page <= 100; page++) {
      const data = await request(
        `${path}${path.includes("?") ? "&" : "?"}per_page=100&page=${page}`,
      );
      if (
        !Array.isArray(data[field]) ||
        !Number.isInteger(data.total_count) ||
        data.total_count < 0
      )
        throw Error("Incomplete GitHub evidence");
      all.push(...data[field]);
      if (data[field].length < 100) {
        if (all.length !== data.total_count) throw Error("Truncated GitHub evidence");
        return all;
      }
    }
    throw Error("GitHub evidence pagination limit exceeded");
  };
  const all = [];
  for (const lane of p.lanes) {
    const runs = await pageAll(
      `/repos/${repository}/actions/workflows/${lane.workflow}/runs?head_sha=${sha}`,
      "workflow_runs",
    );
    // Select before fetching jobs, so a newer failure can never be replaced by an older pass.
    const r = runs
      .filter(
        (r) =>
          r.head_sha === sha &&
          r.path === `.github/workflows/${lane.workflow}` &&
          r.head_repository?.full_name === repository &&
          ["push", "schedule", "workflow_dispatch"].includes(r.event),
      )
      .sort((a, b) => b.id - a.id || b.run_attempt - a.run_attempt)[0];
    if (!r) continue;
    if (!Number.isSafeInteger(r.id) || !Number.isSafeInteger(r.run_attempt) || r.run_attempt < 1)
      throw Error("Invalid GitHub run identity");
    const jobs = await pageAll(
      `/repos/${repository}/actions/runs/${r.id}/attempts/${r.run_attempt}/jobs`,
      "jobs",
    );
    all.push({ ...r, jobs });
  }
  return all;
}

export function payloadFiles(dir) {
  const files = new Map();
  for (const name of readdirSync(dir).sort()) {
    if (name === "release-inventory.json") continue;
    if (!lstatSync(join(dir, name)).isFile())
      throw Error("Release payload must be regular flat files");
    files.set(name, readFileSync(join(dir, name)));
  }
  return files;
}

function snapshots(mode, file) {
  const inventory = read("docs/design/snapshots/inventory.json");
  const names = snapshotNames(inventory);
  if (mode === "snapshot-pack") {
    const files = Object.fromEntries(
      names.map((name) => [
        name,
        readFileSync(join(root, "docs/design/snapshots", name)).toString("base64"),
      ]),
    );
    const bytes = Buffer.from(JSON.stringify({ inventory, files }));
    writeFileSync(file, bytes, { flag: "wx" });
    console.log(digest(bytes));
    return;
  }
  const bytes = readFileSync(file);
  for (const [name, png] of unpackSnapshotBundle(bytes, inventory, policy.designBundleSha256)) {
    const dest = join(root, "docs/design/snapshots", name);
    mkdirSync(dirname(dest), { recursive: true });
    if (lstatSync(dirname(dest)).isSymbolicLink()) throw Error("Snapshot directory is a symlink");
    writeFileSync(dest, png, { flag: "wx" });
  }
}

async function main([mode, ...args]) {
  const errors = checkPolicy(policy, read);
  if (errors.length) throw Error(errors.join("; "));
  if (
    JSON.stringify(policy.blockers.map((b) => b.statement)) !==
    JSON.stringify(canonicalBlockers(read("docs/implementation/vellum-implementation-guide.md")))
  )
    throw Error("Release policy differs from canonical blockers");
  if (mode === "status") {
    console.log(
      JSON.stringify(
        {
          releaseReady: false,
          note: "Local inventory is not candidate CI acceptance",
          blockers: policy.blockers,
        },
        null,
        2,
      ),
    );
  } else if (mode === "gate") {
    const sha = process.env.GITHUB_SHA,
      repository = process.env.GITHUB_REPOSITORY;
    const request = async (path) => {
      const response = await fetch(`https://api.github.com${path}`, {
        headers: {
          Authorization: `Bearer ${process.env.GH_TOKEN ?? ""}`,
          Accept: "application/vnd.github+json",
          "X-GitHub-Api-Version": "2022-11-28",
        },
        signal: AbortSignal.timeout(30000),
        redirect: "error",
      });
      if (!response.ok) throw Error("GitHub release evidence request failed");
      return response.json();
    };
    const result = assessRuns(policy, await collectRuns(policy, repository, sha, request), {
      sha,
      repository,
      now: Date.now(),
    });
    console.log(JSON.stringify(result, null, 2));
    if (!result.ok) process.exitCode = 1;
  } else if (mode === "identity") {
    const cargo = read("Cargo.toml").match(/\[workspace.package\][\s\S]*?\nversion = "([^"]+)"/)[1];
    const versions = [
      cargo,
      ...["desktop", "extension"].map(
        (app) => JSON.parse(read(`apps/${app}/package.json`)).version,
      ),
    ];
    console.log(releaseVersion(process.env.GITHUB_REF, versions));
  } else if (mode === "parity") {
    requireParity(
      checkCoverage(
        read("docs/architecture/ARCHITECTURE.md"),
        JSON.parse(read("fixtures/parity/coverage.json")),
        read,
      ),
    );
  } else if (mode === "browser") {
    requirePlaywright(JSON.parse(readFileSync(args[0], "utf8")));
  } else if (["snapshot-pack", "snapshot-unpack"].includes(mode)) snapshots(mode, args[0]);
  else if (mode === "design-gaps") {
    const blocking = blockingDesignGaps(read("docs/design/DESIGN-GAPS.md"));
    if (blocking.length) throw Error("Blocking design gaps remain");
  } else if (mode === "seal" || mode === "verify") {
    const [dir, version, sha, publicKeyFile] = args;
    const files = payloadFiles(dir),
      path = join(dir, "release-inventory.json");
    requireDistribution(files, version);
    if (mode === "seal") {
      let pem = Buffer.alloc(0);
      try {
        for await (const chunk of process.stdin) {
          if (pem.length + chunk.length > 16384) throw Error("Signing input too large");
          pem = Buffer.concat([pem, chunk]);
        }
        const envelope = sealArtifacts({ version, sha, files }, createPrivateKey(pem));
        // The installed/pinned public trust root must match before any artifact is published.
        verifyArtifacts(envelope, createPublicKey(readFileSync(publicKeyFile)), files, {
          version,
          sha,
        });
        writeFileSync(path, JSON.stringify(envelope), { flag: "wx" });
      } finally {
        pem.fill(0);
      }
    } else
      verifyArtifacts(
        JSON.parse(readFileSync(path)),
        createPublicKey(readFileSync(publicKeyFile)),
        files,
        { version, sha },
      );
  } else
    throw Error(
      "Use status|gate|identity|parity|browser|design-gaps|snapshot-pack|snapshot-unpack|seal|verify",
    );
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main(process.argv.slice(2));
  } catch {
    console.error(
      "release: check failed; review policy, candidate evidence and required inputs (docs/runbooks/release.md)",
    );
    process.exitCode = 1;
  }
}
