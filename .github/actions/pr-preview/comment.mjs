// Runs only from the default branch in the privileged workflow. No artifact contents, PR checkout,
// subprocesses or document-supplied text enter the comment. All IDs/URLs come from GitHub's API.
const marker = "<!-- vellum-pr-preview -->";
export async function publish({ github, context }) {
  const { owner, repo } = context.repo;
  const runId = context.payload.workflow_run.id;
  const { data: run } = await github.rest.actions.getWorkflowRun({ owner, repo, run_id: runId });
  if (
    run.event !== "pull_request" ||
    run.conclusion !== "success" ||
    run.repository.full_name !== `${owner}/${repo}` ||
    !/^[a-f0-9]{40,64}$/.test(run.head_sha)
  )
    return;
  const {
    data: { artifacts },
  } = await github.rest.actions.listWorkflowRunArtifacts({ owner, repo, run_id: runId });
  const artifact = artifacts.find((a) => a.name === "vellum-pr-preview" && !a.expired);
  if (!artifact) return;
  const { data: prs } = await github.rest.repos.listPullRequestsAssociatedWithCommit({
    owner,
    repo,
    commit_sha: run.head_sha,
  });
  // Associated PRs include forks even where workflow_run.pull_requests is empty. Match both the exact
  // commit and its repository, and never replace a newer preview with a stale completion.
  for (const pr of prs) {
    if (
      pr.state !== "open" ||
      pr.head.sha !== run.head_sha ||
      pr.head.repo?.full_name !== run.head_repository?.full_name ||
      pr.base.repo.full_name !== `${owner}/${repo}`
    )
      continue;
    const url = `${context.serverUrl}/${owner}/${repo}/actions/runs/${runId}/artifacts/${artifact.id}`;
    const body = `${marker}\nBefore/after PNGs for changed Vellum frames at \`${run.head_sha.slice(0, 12)}\`.\n\n[Download the preview images and manifest](${url}). Added or removed frames have one image. The manifest lists each changed frame; an empty list means no changed frames.\n\nWorkflow artifacts require GitHub access and expire under the repository's retention policy. Hosted previews require Pro and are not uploaded by this Action.`;
    const comments = await github.rest.issues.listComments({
      owner,
      repo,
      issue_number: pr.number,
      per_page: 100,
    });
    const old = comments.data.find(
      (c) =>
        c.user?.type === "Bot" &&
        c.user?.login === "github-actions[bot]" &&
        c.body?.startsWith(marker),
    );
    if (old) await github.rest.issues.updateComment({ owner, repo, comment_id: old.id, body });
    else await github.rest.issues.createComment({ owner, repo, issue_number: pr.number, body });
  }
}
