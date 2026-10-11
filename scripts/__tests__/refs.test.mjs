// Reference clones (guide Prompt 2): refs/ is never committed, scripts/refs.sh checks each repository
// out at its pinned SHA (blob-less and shallow), and running it again changes nothing.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  bash,
  discard,
  fileUrl,
  guideText,
  makeOrigin,
  parseTsv,
  put,
  read,
  repo,
  run,
  scratch,
} from "./helpers.mjs";

const HEADER = "name\turl\tsha\tlicence\tuse\n";

// A scratch project whose scripts/refs.tsv pins one repository, "alpha", to `sha` at `url`.
function project(sha, url) {
  const root = scratch("refs");
  put(root, "scripts/refs.tsv", `${HEADER}alpha\t${url}\t${sha}\tMIT\t-\n`);
  return root;
}

// Two commits in an origin repository: a.txt says "one", then "two".
function twoCommitOrigin(origin) {
  const [first, second] = makeOrigin(join(origin, "alpha"), [
    { file: "a.txt", text: "one\n" },
    { file: "a.txt", text: "two\n" },
  ]);
  return { first, second, url: fileUrl(join(origin, "alpha")) };
}

const head = (clone) => run("git", ["-C", clone, "rev-parse", "HEAD"]).stdout.trim();

test("refs/ is ignored by git, so reference clones are never committed", () => {
  const ignored = run("git", ["check-ignore", "-q", "refs/penpot/README.md"], { cwd: repo });
  assert.equal(ignored.status, 0, "refs/ must be listed in .gitignore");
});

test("scripts/refs.tsv pins the guide §3 repositories at the SHAs the guide records", () => {
  const fromGuide = [
    ...guideText().matchAll(/^refs\/(\S+)\s+(https:\/\/\S+)\s+([0-9a-f]{40})\s/gm),
  ].map((m) => [m[1], m[2], m[3]]);
  assert.equal(fromGuide.length, 15, "guide §3 lists fifteen repositories");
  const fromManifest = parseTsv(read("scripts", "refs.tsv")).map((r) => [r.name, r.url, r.sha]);
  assert.deepEqual(fromManifest, fromGuide);
});

test("refs.sh clones a repository at its pinned SHA, blob-less and shallow", () => {
  const origin = scratch("origin");
  try {
    const { first, url } = twoCommitOrigin(origin);
    const root = project(first, url);
    try {
      const result = bash("refs.sh", ["--root", root]);
      assert.equal(result.status, 0, result.output);

      const clone = join(root, "refs", "alpha");
      assert.equal(head(clone), first);
      assert.equal(run("git", ["-C", clone, "rev-list", "--count", "HEAD"]).stdout.trim(), "1");
      assert.equal(
        run("git", [
          "-C",
          clone,
          "config",
          "--get",
          "remote.origin.partialclonefilter",
        ]).stdout.trim(),
        "blob:none",
      );
      assert.equal(readFileSync(join(clone, "a.txt"), "utf8"), "one\n");
    } finally {
      discard(root);
    }
  } finally {
    discard(origin);
  }
});

test("refs.sh is idempotent: a second run leaves an up-to-date clone alone", () => {
  const origin = scratch("origin");
  try {
    const { first, url } = twoCommitOrigin(origin);
    const root = project(first, url);
    try {
      assert.equal(bash("refs.sh", ["--root", root]).status, 0);
      const again = bash("refs.sh", ["--root", root]);
      assert.equal(again.status, 0, again.output);
      assert.match(again.output, /already at/);
      assert.equal(head(join(root, "refs", "alpha")), first);
    } finally {
      discard(root);
    }
  } finally {
    discard(origin);
  }
});

test("changing a pin moves an existing clone to the new SHA", () => {
  const origin = scratch("origin");
  try {
    const { first, second, url } = twoCommitOrigin(origin);
    const root = project(first, url);
    try {
      assert.equal(bash("refs.sh", ["--root", root]).status, 0);
      put(root, "scripts/refs.tsv", `${HEADER}alpha\t${url}\t${second}\tMIT\t-\n`);
      const moved = bash("refs.sh", ["--root", root]);
      assert.equal(moved.status, 0, moved.output);
      const clone = join(root, "refs", "alpha");
      assert.equal(head(clone), second);
      assert.equal(readFileSync(join(clone, "a.txt"), "utf8"), "two\n");
    } finally {
      discard(root);
    }
  } finally {
    discard(origin);
  }
});

test("refs.sh refuses a pin that is not a full 40-character SHA", () => {
  const origin = scratch("origin");
  try {
    const { url } = twoCommitOrigin(origin);
    const root = project("main", url);
    try {
      const result = bash("refs.sh", ["--root", root]);
      assert.notEqual(result.status, 0);
      assert.match(result.output, /alpha/);
    } finally {
      discard(root);
    }
  } finally {
    discard(origin);
  }
});

test("refs.sh fails loudly when the origin does not have the pinned commit", () => {
  const origin = scratch("origin");
  try {
    const { url } = twoCommitOrigin(origin);
    const root = project("0".repeat(40), url);
    try {
      assert.notEqual(bash("refs.sh", ["--root", root]).status, 0);
    } finally {
      discard(root);
    }
  } finally {
    discard(origin);
  }
});
