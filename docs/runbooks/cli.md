# CLI and interactive shell (Prompt 59)

Build the single native binary with `node scripts/skia-build.mjs build -p vellum`.
Use `target/debug/vellum` below, or put the built binary on PATH. Local commands need no Vellum
account. The daemon uses the existing authenticated user-only socket/named pipe; commands never
opt into a TCP listener. `--run-dir DIR` selects an isolated daemon and session store for CI.

## Commands and output

```sh
vellum version --json
vellum status --json
vellum new design/app.vellum
vellum export design/app.vellum --frame Login --format png --scale 2 -o login.png
vellum export design/app.vellum --frame Login --format html-tailwind --scaffold -o login.html
vellum tokens export design/app.vellum --format tailwind-v4 > app/theme.css
vellum tokens import design/app.vellum app/theme.css --format tailwind-v4 --dry-run --json
vellum tokens import design/app.vellum app/theme.css --format tailwind-v4 --json
```

Export formats are `png`, `jpeg`, `webp`, `svg`, `pdf`, `html-css`, `html-tailwind`. Scale is 1–3;
vector/code exports keep canvas units. Repeat `--frame` to select layers/pages in order; names must
match exactly and be unique, or use IDs. Omit it to export the document's pages. Multi-image exports
may produce a ZIP; the JSON result reports its MIME. Choose an appropriate destination extension.
Use `--scaffold` for standalone HTML; without it the CLI requests a fragment.
Exports use the existing bounded synchronous tool API: fewer layers/lower scale may be needed for
large files. Missing assets/fonts/libraries, unsupported formats, ambiguous names and destination
write failures exit nonzero. Export does not edit the document.

Token formats are `css-vars`, `tailwind-v4`, `dtcg-json`. Export writes only the token text to stdout
unless `--json` or `-o FILE` is present. Import supports `--dry-run` and `--remove-missing`, and uses
one attributed core transaction. `new` refuses an existing destination; its parent folder must exist.
`--system` currently fails explicitly because the installed design-system catalog from Prompt 58
has no production service. It never substitutes a blank file for a requested system.

Every command accepts `--json`. Single results/errors are JSON objects. `prompt` and `shell` emit
newline-delimited JSON; `prompt` ends every task with `event: "task_end"`, `task`, `ok`, `usage`,
`error`. `tool --json` retains its existing MCP result shape. MCP's stdout remains protocol data.
Exit 0 means success, 1 means a runtime/tool/export/save failure, 2 means invalid CLI arguments.
Text results use ✓, warnings !, errors ✗. The CLI emits no ANSI colour, including with `NO_COLOR`;
non-TTY output has no prompt or spinner. Unknown token counts and unpriced costs stay unavailable.

## Agent mode and CI

```sh
# Inject this variable through the CI secret store; never echo it or put it in a task file.
export CI=1
export VELLUM_OPENROUTER_API_KEY
vellum --run-dir "$RUNNER_TEMP/vellum/run" prompt design/app.vellum \
  "Add a settings screen" --engine builtin \
  --model openrouter/deepseek/deepseek-v4.1-flash --effort medium \
  --attach DESIGN.md --repo . --usage usage.json --preview-each-change --json
vellum --run-dir "$RUNNER_TEMP/vellum/run" daemon stop --json
```

The default builtin model is the bundled OpenRouter hint. Set `--model provider/model` to choose
another. OpenAI uses Responses, Anthropic uses Messages, Google uses GenerateContent, and the
existing compatible presets use Chat Completions. `VELLUM_MODEL_ENDPOINT` overrides the full
endpoint for a configured provider or a custom compatible provider ID. The transport permits HTTPS
or loopback HTTP and refuses embedded URL credentials/credential query parameters. Local
`ollama`/`lmstudio` presets require no key. Supply `--context-window` when known; `--vision` explicitly
declares image support. Attachments accept UTF-8 text, PNG, JPEG and WEBP; oversized inputs fail.
The daemon's request/result budgets also apply to prompts and recorded messages.

Outside CI, the provider key is read from the OS keychain, service `vellum`, account equal to the
provider ID. With explicit `CI=true` or `CI=1`, credentials come from read-only environment variables:
`VELLUM_<PROVIDER>_API_KEY`, uppercased with hyphens replaced by underscores. Conventional unprefixed
names also work (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`). Environment credentials are
never written into the keychain, documents, usage reports or session logs. The existing provider
redactor also removes a key echoed by the model. Do not enable shell tracing around secret injection.

`--engine claude-code` and `--engine codex` use the existing installed-engine adapters and their
own credentials. Supply their model ID with `--model`. Vellum does not install, sign into or
reconfigure those programs. Their native file/command/network tools remain restricted by those
adapters; canvas work returns through the turn's tool bridge. Builtin agents receive canvas tools;
this CLI does not expose unrestricted filesystem/command/network tools to a model.

`--verbose` streams reasoning and full canvas tool arguments/results. `--max-failed-calls N` bounds
failed canvas calls reported by every engine; the builtin engine also enforces its own allowance.
`--preview-each-change` renders the changed step target (or the first page when it has no target),
saves PNGs under the private run directory's `previews/`, and prints their paths. A preview failure
fails the task while keeping already-applied edits. `--export FILE --export-scale N` exports after a
successful turn; its failure also fails the command. Ctrl-C interrupts the engine, stops the turn's
agents through the Guard, preserves completed edits and exits nonzero. A scoped native control
monitor also interrupts stalled provider requests after daemon Pause/Stop; it carries no document
contents or credentials.

```json
[
  {"document":"design/app.vellum","prompt":"Add a footer"},
  {"out":"design/pricing.vellum","prompt":"Create a pricing page","export":"pricing.png"}
]
```

Run it with `vellum prompt --tasks tasks.json --usage usage.json --json`. A task contains exactly
`document` or `out`, a nonempty `prompt`, and optional `attach` (paths), `engine`, `model`, `effort`,
`export`. CLI engine/model/effort/export settings provide defaults. Relative paths resolve under
`--repo` (or the invocation directory). Every row runs even after another row is malformed, a model
fails, or an export fails; the overall exit is nonzero if any row failed. Ctrl-C cancels remaining
rows. Usage reports retain measured usage from failed turns and one result for every row.

`--parallel 2..6 --mode split` first asks the engine for a plan, then partitions its items using the
existing core split policy. `side-by-side` reserves one area per lane for the same brief.
`explore-layout`/`explore-style` build successive variants, changing only the named axis. Optional
`--width`/`--height` set option-frame sizes (defaults 1440×900). Each lane has a distinct agent and
all lanes share one turn/session for undo. Reservations are daemon session state, never document
content. The CLI invocation authorizes dispatch of the proposed task; it has no separate plan-review
prompt. Model-requested helper spawning is not exposed by this remote CLI runner.

`models --engine builtin|claude-code|codex` reports bundled hints/catalog availability. It does not
fetch a live provider catalog, and its JSON marks `complete: false`. `status` checks the daemon and
reports installed engine binaries without claiming their authentication has been checked.

## Shell

```sh
vellum shell design/app.vellum
vellum shell --headless -i design/app.vellum -o design/copy.vellum --json <<'CALLS'
read_skill({path: 'execute.md'})
execute({code: "return doc.get('frame00001');"})
save()
exit()
CALLS
```

The shell accepts one `tool_name({arguments})` per line, plus `save()` and `exit()`/`quit()`.
JavaScript object literals are decoded in the existing sandbox with no document capabilities;
actual tool execution goes through the daemon. There is no host-language eval, filesystem or
network access in that parser. Lines are bounded; use escaped newlines inside script strings.

A single connection retains its attribution and active document. The default path routes to a live
writer when the daemon has one. Explicit `--headless` refuses an existing live writer. `-o` requires
a fresh `.vellum` destination inside the working directory; the input is copied before the output
opens as a core target. Edits affect that output. Headless transactions already auto-save;
`save()` and EOF also ask the authoritative target to save and propagate failures. A failed line
is reported, later lines still run, and the process exits nonzero. Text mode prints tool results;
JSON mode prints one result/error per line.

## Verification and remaining integration boundaries

`cli_complete.rs` is the `native-cli` lane: local sockets, the loopback model stub, the CPU renderer,
filesystem and subprocesses; no real model or account. Run:

```sh
node scripts/skia-build.mjs test -p vellum --test cli_complete -- --ignored
node scripts/skia-build.mjs test -p vellum --test cli --test cli_lifecycle --test mcp --test sessions
```

The export CI lane runs the new native CLI suite. `cli_lifecycle` uses a fake live editor to verify
turn attribution/save/Guard behavior. Real editor attachment still depends on Prompt 55's unserved
EditorLink wire boundary; the shipped editor is currently read-only. The CLI itself works headless.
Real installed engines, OS keychain use, Windows console interruption and other native platforms
need their respective native lanes. No remote CI or real paid provider run is implied by stub tests.
