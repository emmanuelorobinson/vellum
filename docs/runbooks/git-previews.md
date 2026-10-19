# Git review and merging (Prompt 60)

Local Git features work signed out. Install Git supporting `--no-lazy-fetch` (tested with 2.46.2)
and a native `vellum` binary built with the default
`scene` feature. They do not start the daemon, run agents, or change the checkout/index/branch.
The `git setup` command intentionally changes `.gitattributes` and repository-local Git configuration.

```sh
vellum git setup
vellum diff main..HEAD --png --output review-images
vellum diff main..HEAD design/app.vellum --output one-design --json
```

`BASE..HEAD` compares exactly the two commits, not their merge base or the working tree. Both revisions
and their objects must exist locally. Automatic partial-clone lazy fetches are disabled; fetch any missing
objects explicitly before retrying. For a PR, use `git merge-base BASE HEAD` as BASE; the workflow does this automatically.
The optional document path is relative to the repo root, even when invoked from a subdirectory.
Single-file and directory `.vellum` documents work. Commit the referenced libraries and assets too.
A dirty/deleted working-tree asset does not affect either image.

The output folder must be new and its parent must exist. A failure removes the new incomplete output;
existing folders and symlinks are refused. Successful runs write `manifest.json` and numbered
`0001-before.png` / `0001-after.png` files. Names from the document never become output paths.
The manifest has `version: 1`, resolved `base` and `head` SHAs, and sorted `frames` records with
`file`, `id`, `name`, `change` (added/modified/removed), `before` and `after` (relative PNG path or null).
Added/removed frames have one side. Hidden sides have a null image and `beforeState` / `afterState`
set to `hidden`. An empty list is a successful no-change result, not a rendering failure.
`--json` adds `ok`, `command: "diff"`, and `output` to the same data on stdout. Text has one status
line per changed frame and a summary; neither emits ANSI colour, including under `NO_COLOR`.

Targets are outermost frames on each page (including frames inside groups), plus component-definition
roots. Nested frames are covered by their enclosing frame's image. Loose nodes outside frames have
no separate preview. Frame structure, placement/order and cropped PNG bytes are compared: renaming a
frame or changing its internals is reportable even when pixels match. A renamed document is reported
as removal/addition. Metadata with no effect on frames produces no frame previews.
Document changes and transitive imported-library/asset changes select affected documents; unchanged,
unrelated unreadable documents are skipped. Within an affected document all target images are compared,
so variable, style, component and imported-library edits reach their consumers. Only default theme values
are rendered. Scripts are not executed, and missing script output/dependencies/fonts or incomplete renders
fail explicitly. UTF-8 relative paths are required; symlinks/submodules are never followed, Git LFS pointer
files are not downloaded, and network content is never fetched. Limits: 64 MiB per blob, 1,000 transitive
library dependencies, 1,000 changed frames and the shared export bounds/pixel limits. The Action also has
a job timeout. No full-repo materialization, filters or hooks are run by the preview command.

## Merge driver

`vellum git setup` preserves other attribute rules, appends the Vellum rules and sets only local
`merge.vellum.name`, `merge.vellum.driver`, and `merge.vellum.recursive=binary`. Repeating setup without
intervening changes is a no-op (`changed:false` in JSON). The driver command quotes the current executable
path and Git's temporary ancestor/current/other inputs. Re-run setup if the binary moves. Concurrent/stale temporary files are refused; inspect them before retrying. Commit the
`.gitattributes` change; each collaborator must run setup because Git does not transfer local config.
Rules cover `*.vellum` and the index/page JSON files of directory-format documents. More specific
nested attributes can override the root rules; inspect with `git check-attr merge -- design/app.vellum`.

Git invokes `vellum merge-driver BASE OURS THEIRS`. Core merges by ID/property, then the canonical codec
atomically replaces OURS. A clean merge exits 0. A conflict exits 1 and leaves valid JSON with conflict
nodes preserving both versions. Conflicts that cannot fit in a node keep ours and are reported (see
ADR-010); Git retains the other side in the unmerged index. Invalid/newer input or invalid output fails
before replacing OURS. Resolve conflicts in the editor/core, then explicitly `git add` the file. For a
directory document, also resolve any whole-document invariant conflict caused by independent page merges;
a Git low-level driver sees one file at a time and cannot validate the assembled folder as a whole.

`--json` returns `ok:false` and `conflicts` for an unresolved merge, or the standard CLI error for invalid
input. Syntax errors exit 2; operational failures and unresolved merges exit 1. These commands are native
user actions, not model tools; an agent invoking file/config writes must use its permission broker.
To uninstall, remove the Vellum rules from `.gitattributes` and run `git config --local --remove-section
merge.vellum`. This does not remove documents or previews.

## GitHub Action and PR comments

The reusable composite Action is `.github/actions/pr-preview/action.yml`. Supply `binary` (absolute
trusted executable path), `base`, `head` (full commit SHAs), and optionally `repository` (checked-out repo).
It renders into a new runner-temp directory and uploads `vellum-pr-preview` with PNGs and manifest for
14 days. Its `artifact-url` output links to GitHub's artifact download page; images are inside the ZIP,
not public image URLs. An empty diff still uploads the manifest. Rendering failure uploads no preview.
No Vellum account, API key, hosted image service or Pro entitlement is needed.

The supplied `pr-previews.yml` runs on `pull_request` with `contents:read`. It checks out and builds the
trusted base commit, fetches the PR ref as data, calculates the merge base, and invokes the local Action.
It never checks out or executes PR code. Both the CLI and Action must already exist on the base branch;
the PR introducing this workflow needs a subsequent run after merging. The fetch supports private repos
with the read-scoped job token passed through an in-memory Git credential helper for that step only;
credentials are neither persisted nor passed in command-line arguments or exposed to PR code.

`pr-preview-comment.yml` uses `workflow_run` from the default branch with `actions:read` and
`pull-requests:write`. It re-reads run/artifact metadata from GitHub, finds open PRs associated with the
exact head SHA/repository (including forks), and posts or updates its marked bot comment. Stale heads,
failed runs and expired/missing artifacts are skipped. The comment links to the workflow artifact.
The privileged job never downloads artifacts, reads manifest text or executes PR code. It requires GitHub
Actions to be allowed to write PR comments; fork workflows may require the repository owner's approval.
Neither workflow uses `pull_request_target`, repository secrets, or a hosted-preview upload endpoint.
Hosted previews remain a Pro service for Prompts 61–62; this Action does not claim to serve them.

Local validation uses `git_preview` (Git on PATH, native CPU renderer; no daemon/network/GPU), script tests
with a mocked GitHub REST client, and `actionlint`. The export CI lane runs the real Git fixture too.
Actual GitHub upload/comment delivery, fork permission policy and Windows/Linux native execution still
need their remote/platform runs; mocked API success is not proof of remote delivery.
