// TEST DATA. The commenter has only a mocked REST client; never posts to GitHub.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { publish } from "../../.github/actions/pr-preview/comment.mjs";

function fake({ fork = false, stale = false, existing = false, expired = false } = {}) {
  const calls = [];
  const sha = "a".repeat(40);
  const run = {
    id: 123,
    event: "pull_request",
    conclusion: "success",
    head_sha: sha,
    repository: { full_name: "owner/repo" },
    head_repository: { full_name: fork ? "fork/repo" : "owner/repo" },
  };
  const pr = {
    number: 4,
    state: "open",
    head: { sha: stale ? "b".repeat(40) : sha, repo: { full_name: run.head_repository.full_name } },
    base: { repo: { full_name: "owner/repo" } },
  };
  const github = {
    rest: {
      actions: {
        getWorkflowRun: async () => ({ data: run }),
        listWorkflowRunArtifacts: async () => ({
          data: { artifacts: [{ id: 12, name: "vellum-pr-preview", expired }] },
        }),
      },
      repos: { listPullRequestsAssociatedWithCommit: async () => ({ data: [pr] }) },
      issues: {
        listComments: async () => ({
          data: existing
            ? [
                {
                  id: 10,
                  body: "<!-- vellum-pr-preview --> old",
                  user: { type: "Bot", login: "github-actions[bot]" },
                },
              ]
            : [],
        }),
        createComment: async (args) => {
          calls.push(["create", args]);
        },
        updateComment: async (args) => {
          calls.push(["update", args]);
        },
      },
    },
  };
  return {
    github,
    context: {
      repo: { owner: "owner", repo: "repo" },
      payload: { workflow_run: run },
      serverUrl: "https://github.com",
    },
    calls,
    run,
  };
}
test("comments with the artifact download and exact commit, including forks", async () => {
  for (const fork of [false, true]) {
    const f = fake({ fork });
    await publish(f);
    assert.equal(f.calls.length, 1);
    const [, args] = f.calls[0];
    assert.equal(args.issue_number, 4);
    assert.match(args.body, /actions\/runs\/123\/artifacts\/12/);
    assert.match(args.body, /aaaaaaaaaaaa/);
    assert.match(args.body, /Before\/after PNGs/);
    assert.doesNotMatch(args.body, /https?:\/\/(?!github.com)/);
  }
});
test("updates only its own marked bot comment and skips stale or expired results", async () => {
  const f = fake({ existing: true });
  await publish(f);
  assert.equal(f.calls[0][0], "update");
  assert.equal(f.calls[0][1].comment_id, 10);
  for (const flags of [{ stale: true }, { expired: true }]) {
    const f = fake(flags);
    await publish(f);
    assert.equal(f.calls.length, 0);
  }
});
test("does not trust event-supplied repository, conclusion or unrelated PRs", async () => {
  const f = fake();
  f.context.payload.workflow_run = { ...f.run, head_sha: "untrusted", conclusion: "success" };
  f.run.conclusion = "failure";
  await publish(f);
  assert.equal(f.calls.length, 0);
  const g = fake();
  g.github.rest.repos.listPullRequestsAssociatedWithCommit = async () => ({
    data: [
      {
        number: 5,
        state: "open",
        head: { sha: g.run.head_sha, repo: { full_name: "evil/repo" } },
        base: { repo: { full_name: "owner/repo" } },
      },
    ],
  });
  await publish(g);
  assert.equal(g.calls.length, 0);
});
test("render and write-token workflows stay separate; no privileged fork checkout", () => {
  const render = readFileSync(".github/workflows/pr-previews.yml", "utf8");
  const comment = readFileSync(".github/workflows/pr-preview-comment.yml", "utf8");
  assert.match(render, /pull_request:/);
  assert.match(render, /contents: read/);
  assert.doesNotMatch(render, /pull-requests: write|pull_request_target|secrets\./);
  assert.match(render, /persist-credentials: false/);
  assert.match(render, /base\.sha/);
  assert.match(comment, /workflow_run:/);
  assert.match(comment, /pull-requests: write/);
  assert.doesNotMatch(comment, /head\.sha|head_sha.*ref:|download-artifact|pull_request_target/);
  const action = readFileSync(".github/actions/pr-preview/action.yml", "utf8");
  assert.match(action, /upload-artifact@/);
  assert.match(action, /artifact-url/);
  assert.doesNotMatch(action, /run:.*\$\{\{ inputs\./);
});
