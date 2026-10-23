---
title: "Command line"
description: "Every command of the vellum program: documents, exports, tokens, agent tasks, the shell, the daemon, accounts, Git review and the image settings."
---

The command line is the `vellum` program. It is also the daemon and the MCP server, so one build gives you all three. Build it from
the [install](/docs/getting-started/install) page. Local commands need no account. Every command accepts `--json`, which prints a
single JSON object for a result or an error.

## Global options

| Option | Meaning |
| --- | --- |
| `--run-dir DIR` | Use an isolated daemon and session store in `DIR`. The folder must exist, and it is created privately |
| `--json` | Print machine-readable output. Commands that stream print one JSON object per line |
| `--help` | Print the help for a command. `vellum tool <name> --help` lists a tool's flags |

Output is plain text unless you ask for JSON. Text results start with `✓`, warnings with `!`, and errors with `✗`. The program
prints no colour codes at all, including when `NO_COLOR` is set, and it prints no spinner when its output is not a terminal.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | A runtime, tool, export or save failure |
| 2 | Invalid command-line arguments |

## Versions and status

```sh
vellum version --json
vellum status --json
```

`status` checks the daemon and reports the installed agent engines. It does not start the daemon, and it does not check whether an
installed engine is signed in.

## Documents

```sh
vellum new design/app.vellum
```

`new` creates an empty document. The folder must exist, and an existing file is never overwritten.

## Exports

```sh
vellum export design/app.vellum --frame Home --format png --scale 2 -o home.png
vellum export design/app.vellum --frame Home --format html-tailwind --scaffold -o home.html
```

| Option | Meaning |
| --- | --- |
| `--frame NAME` | A frame or page to export, by exact name or id. Repeat it for several, in order. Without it, every page is exported |
| `--format` | `png`, `jpeg`, `webp`, `svg`, `pdf`, `html-css` or `html-tailwind` |
| `--scale` | 1, 2 or 3, for raster formats. Vector and code formats keep the canvas units |
| `--scaffold` | Write a complete HTML page, not a fragment, for the HTML formats |
| `-o FILE` | The file to write. A multi-image export can write a ZIP |

An export never changes the document. A missing name, an ambiguous name, an unsupported format or a failed write exits with a
non-zero code, and the message says which one it was.

## Design tokens

```sh
vellum tokens export design/app.vellum --format tailwind-v4 > app/theme.css
vellum tokens import design/app.vellum app/theme.css --format tailwind-v4 --dry-run --json
```

| Format | Meaning |
| --- | --- |
| `css-vars` | CSS custom properties |
| `tailwind-v4` | A Tailwind v4 `@theme` block |
| `dtcg-json` | Design Tokens (DTCG) JSON |

`tokens export` writes the token text to standard output, or to `-o FILE` with `--json`. `tokens import` takes `--dry-run` to show the
change without making it, and `--remove-missing` to remove the variables the token file no longer names. The import is one change.
`--system` is not available yet, because no design-system catalogue service exists.

## Agent tasks

```sh
vellum prompt design/app.vellum "Add a settings screen" --engine builtin --model <provider/model> --json
vellum prompt --tasks tasks.json --usage usage.json --json
```

| Option | Meaning |
| --- | --- |
| `--engine` | `builtin`, `claude-code` or `codex` |
| `--model` | The model, as `provider/model` for the built-in engine |
| `--effort` | How much the model thinks before it answers |
| `--attach PATH` | A text, PNG, JPEG or WEBP file to give the agent. Repeat it for more |
| `--repo DIR` | The folder that relative paths resolve from. Defaults to the folder you run the command in |
| `--usage FILE` | Write the usage report, including the usage of failed turns |
| `--preview-each-change` | Save a PNG of each changed frame under the run folder's `previews`, and print its path |
| `--export FILE` and `--export-scale N` | Export after a successful turn |
| `--tasks FILE` | Run several tasks from a JSON file. Each task has a `document` or an `out`, and a `prompt` |
| `--parallel N` and `--mode` | Run `N` lanes, from 2 to 6, in one of `split`, `side-by-side`, `explore-layout` or `explore-style` |
| `--width` and `--height` | The size of the option frames a parallel run builds. Defaults to 1440 by 900 |
| `--context-window`, `--vision` | Tell the built-in engine the model's context size, and whether it accepts images |

Keep provider keys out of the command line. The [providers and keys](/docs/reference/providers-and-keys) page says where each key is read from.
A parallel run's lanes share one turn, so one undo reverses them all.

## Models

```sh
vellum models --engine builtin --json
```

Lists the models the bundled catalogue knows for an engine. It does not fetch a live list from the provider, and its output says so.

## The shell

```sh
vellum shell design/app.vellum
vellum shell --headless -i design/app.vellum -o design/copy.vellum --json <<'CALLS'
read_skill({path: 'execute.md'})
execute({code: "return doc.get('frame00001');"})
save()
exit()
CALLS
```

The shell reads one tool call per line, in the form `tool_name({arguments})`, and it also takes `save()` and `exit()`. A failed line is
reported, the later lines still run, and the exit code is non-zero. `--headless` refuses to write to a document that a live editor holds.

## Tools

```sh
vellum tool get_app_state --json
vellum tool insert_nodes --help
```

Every MCP tool is also a command: `vellum tool <name>`. Its flags come from the tool's input schema, and `--help` lists them. The
[MCP tools](/docs/reference/mcp-tools) reference describes each tool.

## The daemon and the MCP server

```sh
vellum daemon
vellum daemon stop
vellum daemon --listen 127.0.0.1:8000 --allow-origin https://editor.example
vellum mcp
```

`daemon` runs the daemon in the foreground. `daemon stop` asks it to stop cleanly. `--listen` opens a TCP port, which is off unless you ask
for it. `mcp` is the stdio server that an agent client starts, and its output is protocol data only.

The [running the daemon](/docs/operations/running-the-daemon) page explains the private folder and the TCP rules.

## Accounts

```sh
vellum login --account-url https://your-account-service.example
vellum whoami
vellum logout
```

`login` uses a device code. The first sign-in needs the account service's address, and later ones reuse it. `logout` removes the local
sign-in even when you are offline. See [accounts and Pro](/docs/operations/accounts-and-pro).

## Git review

```sh
vellum git setup
vellum diff main..HEAD --png --output review-images
vellum diff main..HEAD design/app.vellum --output one-design --json
```

`git setup` writes the repository's `.gitattributes` and sets the Git configuration the design files need. Review it before you commit it.
`diff` compares exactly the two commits you name. `--png` writes an image of each changed frame. The merge driver runs as
`vellum merge-driver`, which Git calls for you after `setup`.

## Image settings and keys

```sh
vellum asset-settings --provider openai --endpoint https://api.openai.com/v1/images/generations --model MODEL_ID
vellum asset-settings --segmentation-model /absolute/path/to/model.onnx
printf '%s' "$FIGMA_TOKEN" | vellum asset-key figma
```

`asset-settings` saves the non-secret image settings beside the daemon's state, and it takes all three provider values together. The
`asset-key` command reads a key from standard input, and refuses to read it from a terminal, so a key is never echoed or put in your shell
history. It saves the key to the keychain and prints `stored: true`.
