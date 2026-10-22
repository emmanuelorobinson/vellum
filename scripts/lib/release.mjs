// Pure release decisions. Time, GitHub results, file bytes and signing keys are supplied by callers.
import { createHash, sign, verify } from "node:crypto";

const stable = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const commit = /^[a-f0-9]{40}$/;
const safePath = (p) =>
  typeof p === "string" &&
  /^[\w./-]+$/.test(p) &&
  p.split("/").every((part) => part && part !== "." && part !== "..");
const safeName = (p) => safePath(p) && !p.includes("/") && !p.startsWith(".");
export const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function snapshotNames(inventory) {
  const names = JSON.parse(inventory)
    .frames.flatMap((f) => Object.values(f.snapshots ?? {}))
    .sort();
  if (
    !names.length ||
    new Set(names).size !== names.length ||
    names.some(
      (n) => !/^(?:(?:components|foundations)\/)?[\w.@-]+\.png$/.test(n) || n.includes(".."),
    )
  )
    throw Error("Invalid snapshot inventory");
  return names;
}
export function unpackSnapshotBundle(bytes, inventory, expectedHash) {
  const names = snapshotNames(inventory);
  if (!/^[a-f0-9]{64}$/.test(expectedHash) || digest(bytes) !== expectedHash)
    throw Error("Design bundle requires its reviewed SHA-256 in release-policy.json");
  const data = JSON.parse(bytes);
  if (
    data.inventory !== inventory ||
    Object.keys(data.files).sort().join("\n") !== names.join("\n")
  )
    throw Error("Design bundle inventory mismatch");
  return new Map(
    names.map((name) => {
      const png = Buffer.from(data.files[name], "base64");
      if (!png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])))
        throw Error("Invalid snapshot PNG");
      return [name, png];
    }),
  );
}

export function canonicalBlockers(guide) {
  const block = guide.split("## RELEASE BLOCKERS (all must be false)")[1]?.split("```")[0];
  if (!block) throw Error("Canonical release blockers missing");
  const statements = [];
  for (const line of block.split("\n")) {
    if (line.startsWith("- ")) statements.push(line.slice(2));
    else if (line.startsWith("  ") && statements.length)
      statements[statements.length - 1] += ` ${line.trim()}`;
  }
  if (statements.length !== 16) throw Error("Canonical release blocker count changed");
  return statements;
}

// Append-only gap entries require an explicit status field to close a blocking finding.
export function blockingDesignGaps(text) {
  const lines = text.split("\n").filter((line) => /^- /.test(line));
  const key = (line) => line.match(/\bcheck=([\w.-]+)/)?.[1];
  const closed = (line) => /(?:^|·)\s*status=(?:resolved|closed)\s*(?:·|$)/i.test(line);
  const latest = new Map(lines.filter(key).map((line) => [key(line), line]));
  return lines.filter(
    (line) =>
      /\bblocking\b/i.test(line) &&
      !/\bnon-blocking\b/i.test(line) &&
      !closed(key(line) ? latest.get(key(line)) : line),
  );
}

export function releaseVersion(ref, versions) {
  const version = ref?.replace(/^refs\/tags\/v/, "");
  if (
    !ref?.startsWith("refs/tags/v") ||
    !stable.test(version) ||
    version === "0.0.0" ||
    !versions.length ||
    versions.some((v) => v !== version)
  )
    throw Error("Release requires a stable tag and matching non-placeholder Cargo/host versions");
  return version;
}

export function checkPolicy(policy, read) {
  const errors = [];
  if (
    policy.version !== 1 ||
    !Number.isInteger(policy.maxAgeHours) ||
    policy.maxAgeHours < 1 ||
    policy.maxAgeHours > 168 ||
    !policy.lanes?.length ||
    !policy.blockers?.length
  )
    return ["Invalid release policy"];
  const workflows = new Set();
  for (const lane of policy.lanes) {
    if (
      !/^[\w-]+\.yml$/.test(lane.workflow) ||
      workflows.has(lane.workflow) ||
      !Array.isArray(lane.jobs) ||
      !lane.jobs.length ||
      lane.jobs.some((j) => typeof j !== "string" || !j.trim()) ||
      new Set(lane.jobs).size !== lane.jobs.length
    )
      errors.push("Invalid or duplicate lane");
    workflows.add(lane.workflow);
    try {
      read(`.github/workflows/${lane.workflow}`);
    } catch {
      errors.push(`Missing workflow ${lane.workflow}`);
    }
  }
  const ids = new Set();
  for (const b of policy.blockers) {
    if (
      !/^[a-z][a-z0-9-]+$/.test(b.id) ||
      ids.has(b.id) ||
      !b.statement?.trim() ||
      !Array.isArray(b.open) ||
      b.open.some((s) => typeof s !== "string" || !s.trim()) ||
      !b.lanes?.length ||
      b.lanes.some((l) => !workflows.has(l)) ||
      !b.evidence?.length
    )
      errors.push(`Invalid blocker ${b.id}`);
    ids.add(b.id);
    for (const file of b.evidence ?? []) {
      if (!safePath(file)) {
        errors.push(`Unsafe evidence ${b.id}`);
        continue;
      }
      try {
        if (!read(file).trim()) throw Error();
      } catch {
        errors.push(`Missing evidence ${file}`);
      }
    }
  }
  return errors;
}

/** Latest run wins, including a failed or in-progress rerun. PR/fork runs cannot authorize release. */
export function assessRuns(policy, runs, { sha, repository, now }) {
  if (!commit.test(sha) || !/^[\w.-]+\/[\w.-]+$/.test(repository) || !Number.isFinite(now))
    throw Error("Invalid release candidate");
  const lanes = policy.lanes.map((lane) => {
    const run = runs
      .filter(
        (r) =>
          r.head_sha === sha &&
          r.path === `.github/workflows/${lane.workflow}` &&
          r.head_repository?.full_name === repository &&
          ["push", "schedule", "workflow_dispatch"].includes(r.event),
      )
      .sort((a, b) => b.id - a.id || b.run_attempt - a.run_attempt)[0];
    const age = now - Date.parse(run?.created_at);
    const problems = [];
    if (
      !run ||
      !Number.isFinite(age) ||
      age < 0 ||
      age > policy.maxAgeHours * 3600000 ||
      run.status !== "completed" ||
      run.conclusion !== "success"
    )
      problems.push("missing, stale or unsuccessful run");
    for (const name of lane.jobs) {
      const jobs = run?.jobs?.filter((j) => j.name === name) ?? [];
      if (jobs.length !== 1 || jobs[0].status !== "completed" || jobs[0].conclusion !== "success")
        problems.push(`required job: ${name}`);
    }
    return { workflow: lane.workflow, run: run?.id ?? null, ok: !problems.length, problems };
  });
  const blockers = policy.blockers.map((b) => ({
    id: b.id,
    reasons: [
      ...b.open,
      ...b.lanes
        .filter((l) => !lanes.find((lane) => lane.workflow === l)?.ok)
        .map((l) => `No passing candidate evidence: ${l}`),
    ],
  }));
  return {
    sha,
    ok: lanes.every((l) => l.ok) && blockers.every((b) => !b.reasons.length),
    lanes,
    blockers,
  };
}

export function requireParity(result) {
  if (
    !Number.isInteger(result.total) ||
    result.total < 1 ||
    result.covered !== result.total ||
    !Array.isArray(result.pending) ||
    result.pending.length
  )
    throw Error("Release parity has pending or missing v1 rows");
}
export function requirePlaywright(result) {
  const s = result.stats;
  if (
    !s ||
    !Number.isInteger(s.expected) ||
    s.expected < 1 ||
    s.unexpected !== 0 ||
    s.flaky !== 0 ||
    s.skipped !== 0 ||
    !Array.isArray(result.errors) ||
    result.errors.length
  )
    throw Error(
      "Release browser acceptance requires passing tests with zero skips, failures or flakes",
    );
}

export function sealArtifacts({ version, sha, files }, key) {
  releaseVersion(`refs/tags/v${version}`, [version]);
  if (!commit.test(sha) || !files.size || key.asymmetricKeyType !== "ed25519")
    throw Error("Invalid artifact identity/key");
  const entries = [...files]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, bytes]) => {
      if (!safeName(name) || !bytes.length) throw Error("Invalid artifact name or size");
      return { name, size: bytes.length, sha256: digest(bytes) };
    });
  const payload = Buffer.from(JSON.stringify({ version, sha, files: entries }));
  return {
    payload: payload.toString("base64"),
    signature: sign(null, payload, key).toString("base64"),
  };
}
export function verifyArtifacts(envelope, key, files, { version, sha }) {
  if (
    typeof envelope.payload !== "string" ||
    envelope.payload.length > 1024 * 1024 ||
    typeof envelope.signature !== "string" ||
    envelope.signature.length > 256 ||
    key.asymmetricKeyType !== "ed25519"
  )
    throw Error("Invalid artifact signature");
  const payload = Buffer.from(envelope.payload, "base64");
  if (!verify(null, payload, key, Buffer.from(envelope.signature, "base64")))
    throw Error("Invalid artifact signature");
  const data = JSON.parse(payload);
  if (
    data.sha !== sha ||
    data.version !== version ||
    !Array.isArray(data.files) ||
    !data.files.length ||
    data.files.length !== files.size ||
    new Set(data.files.map((f) => f.name)).size !== files.size
  )
    throw Error("Artifact identity/inventory mismatch");
  for (const f of data.files) {
    const bytes = files.get(f.name);
    if (
      !safeName(f.name) ||
      !bytes?.length ||
      bytes.length !== f.size ||
      digest(bytes) !== f.sha256
    )
      throw Error("Artifact digest mismatch");
  }
  return data;
}
