#!/usr/bin/env node
// Writes the generated pages of the documentation site from the repository's own sources, then checks every page.
//
// Generated (ignored by Git, rebuilt on every docs:prepare):
//   content/docs/operations/runbooks/   every runbook in docs/runbooks, with its title and description
//   content/docs/reference/decisions/   every decision record in docs/architecture/decisions
//   content/docs/reference/mcp-tools/   every MCP tool in the committed schema snapshot, grouped
//   content/docs/reference/script-api-declarations.md   the generated declarations of the document API
//   content/docs/reference/file-format-spec.md, file-layout.md   the format's generated reference pages
//   .generated/agent-docs.ts            the index served at /llms.txt
//
// Hand-written pages live beside them in content/docs and are never touched here. Edit the repository source,
// never a generated copy. Run `node scripts/docs-content.mjs` (or `pnpm docs:prepare`) after changing a source.

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { brand } from "../../../packages/config/src/index.ts";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(app, "../..");
const content = join(app, "content/docs");
const BRAND_TOKEN = "{{brand}}";

/** Runbook slug, description, and the group it appears under in the runbook index. */
export const RUNBOOKS = [
  [
    "accessibility-i18n",
    "Accessibility, language and direction",
    "Bundled languages, what is still English, and the accessibility checks.",
    "Product and quality",
  ],
  [
    "accounts",
    "Accounts and cloud services",
    "The account service, device sign-in, Pro entitlements, cloud jobs and the settings an operator supplies.",
    "Accounts and sharing",
  ],
  [
    "authoring",
    "Editor authoring ports",
    "Scripts, design-system guidelines, slots and pictures as editor ports.",
    "Editor hosts",
  ],
  [
    "browser-import",
    "Browser import and embeds",
    "The browser capture tool, element import, embedded websites and the Chromium it uses.",
    "Imports and images",
  ],
  [
    "cli",
    "Command-line tool",
    "The vellum command: exports, tokens, agent tasks, the shell, models and status.",
    "Runtime and agents",
  ],
  [
    "connect-agents",
    "Connect agents and extension distribution",
    "The VS Code command that registers the tool with supported agent clients, and extension packaging.",
    "Editor hosts",
  ],
  [
    "daemon",
    "Local daemon and IPC",
    "The local daemon, its private run directory, the MCP bridge and the opt-in TCP listener.",
    "Runtime and agents",
  ],
  [
    "design-mcp",
    "Reading the design through the design tool",
    "How the repository reads its design file through the design tool, read-only.",
    "Design source",
  ],
  [
    "design-tokens",
    "Design tokens",
    "How the shared tokens are generated from the design file and checked.",
    "Design source",
  ],
  [
    "desktop-home",
    "Desktop Home, drafts and recovery",
    "The desktop Home screen, drafts, recovery files and what is still unavailable.",
    "Editor hosts",
  ],
  [
    "desktop-shell",
    "Desktop shell",
    "Windows, tabs, menus, the sidecar and the security boundaries of the desktop app.",
    "Editor hosts",
  ],
  [
    "display-list",
    "Display list",
    "The versioned display list that the renderers consume.",
    "Rendering",
  ],
  [
    "doc-api",
    "Document programs and script nodes",
    "The execute API, its limits, and the header of a script node.",
    "Document and scripts",
  ],
  [
    "engine-spike",
    "Engine spike",
    "The research spike behind the document-store decision.",
    "Research spikes",
  ],
  [
    "exports",
    "Export backends",
    "Export formats, HTML options, limits and the verification lanes.",
    "Imports and images",
  ],
  [
    "files",
    "Files on disk",
    "How hosts keep documents, assets, fonts and libraries on disk.",
    "Document and scripts",
  ],
  [
    "git-previews",
    "Git review and merging",
    "Review diffs as images, the merge driver and its setup.",
    "Document and scripts",
  ],
  [
    "handoff",
    "Ledger and hand-off ports",
    "History, revert, conflicts, and copy as image or HTML.",
    "Editor hosts",
  ],
  [
    "image-work",
    "Figma import and image work",
    "Figma REST import, generated images, local background removal and tracing, and their settings.",
    "Imports and images",
  ],
  [
    "imports",
    "Icons, pictures and imports",
    "The icon, picture and import tools, the importers and what they leave out.",
    "Imports and images",
  ],
  [
    "installed-engines",
    "Installed engines",
    "Claude Code and Codex as installed agent engines: versions, replay lanes and recordings.",
    "Runtime and agents",
  ],
  [
    "layout-oracle",
    "Layout oracle",
    "The Chrome layout oracle that writes the expected boxes for layout fixtures.",
    "Rendering",
  ],
  [
    "model-providers",
    "Model providers and secret stores",
    "Model providers, their endpoints, and how credentials are stored and read.",
    "Runtime and agents",
  ],
  [
    "multiplayer",
    "Multiplayer, comments and self-hosting",
    "Live collaboration, comments and self-hosting the room service.",
    "Accounts and sharing",
  ],
  ["pen-import", "Importing .pen files", "Importing the public .pen format.", "Imports and images"],
  [
    "performance-observability",
    "Performance and observability",
    "Performance gates, budgets and minimal opt-in observability.",
    "Product and quality",
  ],
  [
    "release",
    "Release pipelines",
    "The release gate, the evidence it checks, and what remains before a release.",
    "Product and quality",
  ],
  [
    "renderer",
    "Renderers",
    "The shared Skia renderer, native builds and the fonts it bundles.",
    "Rendering",
  ],
  [
    "renderer-spike",
    "Renderer spike",
    "The research spike behind the renderer decision.",
    "Research spikes",
  ],
  [
    "scripts",
    "Script sandbox",
    "The QuickJS sandbox: what a script can and cannot do.",
    "Document and scripts",
  ],
  [
    "security",
    "Security checks and release acceptance",
    "Security checks, fuzzing, the trust boundaries and what release acceptance still needs.",
    "Product and quality",
  ],
  [
    "shaders",
    "Shaders and examples",
    "Shader fills and effects: the dialect, the limits and the compiler.",
    "Document and scripts",
  ],
  [
    "shell",
    "Palette, context menu and settings",
    "The command palette, the context menu, the settings dialog and the edge states.",
    "Editor hosts",
  ],
  [
    "skills-and-parity",
    "Skills, styles and parity",
    "Skills, styles and the parity inventory that maps architecture rows to tests.",
    "Runtime and agents",
  ],
  [
    "slides",
    "Slides and presenting",
    "Slide order, presenting and PDF export.",
    "Imports and images",
  ],
  [
    "sharing",
    "Share snapshots",
    "The Pro sharing flow, its limits, the portable ZIP and what the server needs.",
    "Accounts and sharing",
  ],
  [
    "sync",
    "Sync service",
    "The optional sync service: rooms, capabilities, origins and self-hosting.",
    "Accounts and sharing",
  ],
  ["text", "Text layout tests", "Text layout tests and the fonts lane.", "Rendering"],
  [
    "tools",
    "Tool registry",
    "The tool registry, its MCP and CLI front ends, and how a tool is added.",
    "Runtime and agents",
  ],
  [
    "vscode-host",
    "VS Code host",
    "The VS Code custom editor, its editing model, its build and its tests.",
    "Editor hosts",
  ],
];

/** Every MCP tool, by the group it appears under. The check fails when a tool in the snapshot has no group. */
export const TOOL_GROUPS = [
  [
    "context",
    "Context",
    "What is open and the document the next calls use.",
    ["get_app_state", "open_document"],
  ],
  [
    "reading",
    "Reading the design",
    "Look at nodes, layout, variables, components and fonts before changing them.",
    [
      "get_nodes",
      "query_nodes",
      "find_properties",
      "snapshot_layout",
      "find_space",
      "screenshot",
      "get_variables",
      "list_components",
      "list_fonts",
    ],
  ],
  [
    "changing",
    "Changing the design",
    "Write changes in batches. Each call is one transaction, or nothing.",
    [
      "execute",
      "insert_nodes",
      "update_nodes",
      "copy_nodes",
      "replace_nodes",
      "move_nodes",
      "delete_nodes",
      "replace_properties",
      "set_variables",
      "create_component",
      "create_instance",
      "make_slot",
      "manage_components",
      "manage_pages",
      "instance_nodes",
      "arrange_nodes",
    ],
  ],
  [
    "assets",
    "Icons, pictures and imports",
    "Put outside content on the canvas as ordinary changes.",
    ["search_icons", "insert_icon", "fill_image", "image_ops", "import"],
  ],
  [
    "tokens-and-code",
    "Tokens, code and specs",
    "Export tokens and layouts, and read a frame as a spec for code.",
    ["export", "export_tokens", "import_tokens", "get_spec"],
  ],
  [
    "coordination",
    "Coordination",
    "Hold frames against other agents and report progress to the person. The spawn_agents tool, which starts helper agents, is offered only where the host allows it, so it is not in the default list.",
    ["claim_frame", "release_frame", "report_progress"],
  ],
  [
    "skills-and-styles",
    "Skills and styles",
    "Instructions and visual styles that agents read before designing.",
    ["read_skill", "list_skills", "get_style"],
  ],
  ["browser", "Browser", "Capture a web page, import its layers, or embed it.", ["browser"]],
];

// ---- small text helpers ----------------------------------------------------------------------------------------

const yaml = (value) => JSON.stringify(value);

export const frontmatter = (title, description) =>
  `---\ntitle: ${yaml(title)}\ndescription: ${yaml(description)}\n---\n`;

/** The first sentence, or the first 200 characters at a word boundary, of a paragraph. */
export function briefly(text, limit = 200) {
  const clean = text.replace(/\s+/g, " ").trim();
  const sentence = clean.match(/^.*?[.!?](?=\s|$)/);
  if (sentence && sentence[0].length <= limit) return sentence[0];
  if (clean.length <= limit) return clean;
  const cut = clean.slice(0, limit);
  return `${cut.slice(0, cut.lastIndexOf(" ")).replace(/[,;:]$/, "")}.`;
}

/** Removes the first top-level heading (the page title moves to frontmatter) and returns it with the body. */
export function splitTitle(markdown) {
  const lines = markdown.split("\n");
  const index = lines.findIndex((line) => /^# /.test(line));
  if (index === -1) throw new Error("A source page has no top-level heading.");
  const title = lines[index].replace(/^# /, "").trim();
  const body = [...lines.slice(0, index), ...lines.slice(index + 1)].join("\n").replace(/^\n+/, "");
  return { title, body };
}

/** Runs `transform` on prose only: never on fenced code or inline code. */
export function mapProse(markdown, transform) {
  return markdown
    .split(/(^```[\s\S]*?^```[^\n]*$)/m)
    .map((piece, index) => {
      if (index % 2 === 1) return piece;
      return piece
        .split(/(`[^`\n]*`)/)
        .map((part, inner) => (inner % 2 === 1 ? part : transform(part)))
        .join("");
    })
    .join("");
}

/**
 * Writes the product's name as the brand token in prose, so a rename is one edit in @vellum/config. Link targets
 * keep their text: a URL is not prose.
 */
export const brandProse = (markdown) =>
  mapProse(markdown, (prose) =>
    prose.replace(/(\]\([^)\s]*\))|\bVellum\b/g, (_match, target) => target ?? BRAND_TOKEN),
  );

// ---- link rewriting ----------------------------------------------------------------------------------------

const ADR_FILE = /^ADR-\d{3}[a-z]?-[a-z0-9-]+\.md$/;

/**
 * Turns the links of a repository page into links on this site. Runbooks, decision records and format pages get
 * their site address; any other repository file becomes its path in code, because no website address exists for it.
 */
export function rewriteLinks(markdown, sourceRel) {
  return mapProse(markdown, (prose) =>
    prose.replace(/\[([^\]]*)\]\(([^)\s]*)\)/g, (whole, text, target) => {
      if (/^(https?:|mailto:)/.test(target) || target.startsWith("#")) return whole;
      const [pathPart, anchor = ""] = target.split("#");
      const suffix = anchor ? `#${anchor}` : "";
      const repoPath = posix.normalize(posix.join(posix.dirname(sourceRel), pathPart));
      const runbook = repoPath.match(/^docs\/runbooks\/([a-z0-9-]+)\.md$/);
      if (runbook && RUNBOOKS.some(([slug]) => slug === runbook[1])) {
        return `[${text}](/docs/operations/runbooks/${runbook[1]}${suffix})`;
      }
      const adr = repoPath.match(/^docs\/architecture\/decisions\/(ADR-[^/]+)\.md$/);
      if (adr && ADR_FILE.test(`${adr[1]}.md`)) {
        return `[${text}](/docs/reference/decisions/${adr[1].toLowerCase()}${suffix})`;
      }
      if (repoPath === "docs/architecture/threat-model.md")
        return `[${text}](/docs/architecture/threat-model${suffix})`;
      if (repoPath === "docs/format/README.md")
        return `[${text}](/docs/reference/file-format-spec${suffix})`;
      if (repoPath === "docs/format/files.md")
        return `[${text}](/docs/reference/file-layout${suffix})`;
      return `${text} (\`${repoPath}\`)`;
    }),
  );
}

// ---- the decision records ----------------------------------------------------------------------------------

export function decisionPages(decisionsDir = join(root, "docs/architecture/decisions")) {
  return readdirSync(decisionsDir)
    .filter((name) => ADR_FILE.test(name))
    .sort()
    .map((name) => {
      const source = `docs/architecture/decisions/${name}`;
      const raw = readFileSync(join(root, source), "utf8");
      const { title, body } = splitTitle(raw);
      const slug = name.replace(/\.md$/, "").toLowerCase();
      const firstParagraph = body
        .split(/\n\s*\n/)
        .map((block) => block.trim())
        .find((block) => block && !block.startsWith("#") && !/^Status:/i.test(block));
      const status = body.match(/^Status:\s*(.+)$/im)?.[1] ?? "";
      const description = briefly(firstParagraph ?? title, 200);
      return { slug, source, title, description, status, body };
    });
}

// ---- the keyboard ------------------------------------------------------------------------------------------

const KEY_NAMES = {
  Escape: "Esc",
  Period: ".",
  Comma: ",",
  Slash: "/",
  Backslash: "\\",
  Quote: "'",
  Equal: "=",
  Minus: "-",
  BracketLeft: "[",
  BracketRight: "]",
  Enter: "Enter",
  Space: "Space",
  Backspace: "Backspace",
  Delete: "Delete",
  Tab: "Tab",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Click: "click",
  Scroll: "scroll",
  Drag: "drag",
  MiddleDrag: "middle-drag",
  Pinch: "pinch",
};

/** A binding as a reader sees it: KeyF is F, Digit1 is 1, Mod+Shift+KeyS is Mod+Shift+S. */
export function readableKey(binding) {
  return binding
    .split("+")
    .map((part) => {
      if (part in KEY_NAMES) return KEY_NAMES[part];
      if (/^Key[A-Z]$/.test(part)) return part.slice(3);
      if (/^Digit\d$/.test(part)) return part.slice(5);
      return part;
    })
    .join("+");
}

const SHORTCUT_SECTIONS = {
  agent: "Agent",
  session: "Sessions",
  panel: "Panels",
  palette: "Command palette",
  composer: "Composer",
  tool: "Tools",
  edit: "Editing",
  arrange: "Arranging",
  select: "Selection",
  component: "Components",
  path: "Path editing",
  view: "View and zoom",
  present: "Presenting",
  file: "Files and hosts",
};

/** Every default binding in the editor, read from its keybinding table. */
export function keyboardRows(source = join(root, "packages/editor-core/src/keybindings.ts")) {
  const text = readFileSync(source, "utf8");
  const rows = [...text.matchAll(/\["([a-zA-Z.]+)", "([^"]*)"((?:, "[^"]*")+)\]/g)].map(
    (match) => ({
      id: match[1],
      label: match[2],
      keys: [...match[3].matchAll(/"([^"]*)"/g)].map((key) => key[1]),
    }),
  );
  if (rows.length === 0)
    throw new Error("No keybindings were found in packages/editor-core/src/keybindings.ts.");
  return rows;
}

export function keyboardPage() {
  const rows = keyboardRows();
  const sections = new Map();
  for (const row of rows) {
    const section = row.id.split(".")[0];
    const label = SHORTCUT_SECTIONS[section] ?? section;
    if (!sections.has(section)) sections.set(section, { label, actions: new Map() });
    const actions = sections.get(section).actions;
    const action = actions.get(row.label) ?? [];
    for (const key of row.keys) if (!action.includes(key)) action.push(key);
    actions.set(row.label, action);
  }
  const lines = [
    frontmatter(
      "Keyboard shortcuts",
      "Every default keyboard binding and gesture in the editor, generated from its keybinding table.",
    ),
    SOURCE_NOTE("packages/editor-core/src/keybindings.ts"),
    "These are the editor’s default bindings. `Mod` is Command on macOS and Control on Windows and Linux. `Alt` is Option on macOS. A host can change some bindings, and a host may keep a key for itself: the VS Code extension uses the editor’s VS Code shortcut policy, so a key that VS Code reserves keeps its VS Code meaning.",
    "",
  ];
  for (const { label, actions } of sections.values()) {
    lines.push(`## ${label}`, "", "| Action | Keys |", "| --- | --- |");
    for (const [action, keys] of actions) {
      lines.push(
        `| ${cell(action)} | ${keys.map((key) => `\`${readableKey(key)}\``).join(", ")} |`,
      );
    }
    lines.push("");
  }
  return lines.join("\n");
}

// ---- the runbooks ------------------------------------------------------------------------------------------

export function runbookPages(runbooksDir = join(root, "docs/runbooks")) {
  const files = readdirSync(runbooksDir).filter((name) => name.endsWith(".md"));
  const known = new Set(RUNBOOKS.map(([slug]) => slug));
  const missing = files.map((name) => name.replace(/\.md$/, "")).filter((slug) => !known.has(slug));
  if (missing.length > 0) {
    throw new Error(
      `Runbooks without a title and description in scripts/docs-content.mjs: ${missing.join(", ")}`,
    );
  }
  return RUNBOOKS.map(([slug, title, description, group]) => {
    const source = `docs/runbooks/${slug}.md`;
    const raw = readFileSync(join(root, source), "utf8");
    return { slug, source, title, description, group, body: splitTitle(raw).body };
  });
}

// ---- the MCP tools -----------------------------------------------------------------------------------------

/** The JSON type of a schema property, as one readable phrase. */
export function describeType(schema, defs = {}) {
  if (!schema || typeof schema !== "object") return "any";
  if (schema.$ref) {
    const name = schema.$ref.split("/").pop();
    return defs[name] ? describeType(defs[name], defs) : name;
  }
  if (schema.enum) return `one of ${schema.enum.map((value) => `\`${value}\``).join(", ")}`;
  if (schema.anyOf || schema.oneOf) {
    const options = (schema.anyOf ?? schema.oneOf).filter((option) => option.type !== "null");
    const nullable = (schema.anyOf ?? schema.oneOf).length !== options.length;
    const phrase = options.map((option) => describeType(option, defs)).join(" or ") || "any";
    return nullable ? `${phrase} (optional)` : phrase;
  }
  if (schema.type === "array") return `array of ${describeType(schema.items, defs)}`;
  if (Array.isArray(schema.type)) return schema.type.filter((t) => t !== "null").join(" or ");
  if (schema.type === "object") return "object";
  return schema.type ?? "any";
}

const cell = (text) =>
  String(text ?? "")
    .replace(/\|/g, "\\|")
    .replace(/\s+/g, " ")
    .trim();

export function toolPage(tool, title) {
  const schema = tool.inputSchema ?? { type: "object", properties: {} };
  const defs = { ...(schema.$defs ?? {}), ...(schema.definitions ?? {}) };
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {});
  const rows = properties.map(([name, property]) => {
    const text =
      property.description ??
      property.anyOf?.find((option) => option.description)?.description ??
      "";
    return `| \`${name}\` | ${cell(describeType(property, defs))} | ${required.has(name) ? "yes" : "no"} | ${cell(text)} |`;
  });
  const annotations = tool.annotations ?? {};
  const lines = [
    `## \`${tool.name}\``,
    "",
    tool.description,
    "",
    `Read only: ${annotations.readOnlyHint ? "yes" : "no"}. Destructive: ${annotations.destructiveHint ? "yes" : "no"}. Version ${tool._meta?.["vellum/version"] ?? 1}.`,
    "",
  ];
  if (rows.length > 0) {
    lines.push(
      "| Parameter | Type | Required | Meaning |",
      "| --- | --- | --- | --- |",
      ...rows,
      "",
    );
  } else {
    lines.push("This tool takes no parameters.", "");
  }
  lines.push("The full input schema:", "", "```json", JSON.stringify(schema, null, 2), "```", "");
  return { title, text: lines.join("\n") };
}

/** Groups the snapshot's tools by TOOL_GROUPS. Throws for a tool no group names, so a new tool cannot go undocumented. */
export function groupTools(tools) {
  const placed = new Map();
  for (const [slug, , , names] of TOOL_GROUPS) {
    for (const name of names) {
      if (placed.has(name)) throw new Error(`The MCP tool ${name} is in two groups.`);
      placed.set(name, slug);
    }
  }
  const unplaced = tools.map((tool) => tool.name).filter((name) => !placed.has(name));
  if (unplaced.length > 0) {
    throw new Error(
      `MCP tools without a documentation group in scripts/docs-content.mjs: ${unplaced.join(", ")}`,
    );
  }
  const named = new Set(tools.map((tool) => tool.name));
  const absent = [...placed.keys()].filter((name) => !named.has(name));
  if (absent.length > 0) {
    throw new Error(
      `Documentation groups name tools the snapshot does not have: ${absent.join(", ")}`,
    );
  }
  return TOOL_GROUPS.map(([slug, title, description, names]) => ({
    slug,
    title,
    description,
    tools: names.map((name) => tools.find((tool) => tool.name === name)),
  }));
}

export function loadToolSnapshot(
  path = join(root, "crates/vellum/tests/snapshots/mcp-tools.json"),
) {
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  return parsed.tools ?? parsed;
}

// ---- writing the pages ---------------------------------------------------------------------------------------

const SOURCE_NOTE = (source) =>
  `> Generated from \`${source}\` in the repository on every documentation build. Edit that source, not this copy. ` +
  `Status statements come from the source as it stood at the build; check the [availability](/docs/getting-started/availability) page before you rely on one.\n`;

function write(path, text) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

function writeMeta(dir, title, pages) {
  write(join(dir, "meta.json"), `${JSON.stringify({ title, pages }, null, 2)}\n`);
}

/** Removes exactly what this script writes: the generated folders and the generated single pages. */
function clean() {
  for (const dir of ["operations/runbooks", "reference/decisions", "reference/mcp-tools"]) {
    rmSync(join(content, dir), { recursive: true, force: true });
  }
  for (const file of [
    "reference/script-api-declarations.md",
    "reference/file-format-spec.md",
    "reference/file-layout.md",
    "reference/keyboard-shortcuts.md",
    "architecture/threat-model.md",
  ]) {
    rmSync(join(content, file), { force: true });
  }
}

export function prepareContent({ logger = () => {} } = {}) {
  clean();
  const indexes = {};

  // Runbooks, grouped the way the index lists them.
  const runbookOut = join(content, "operations/runbooks");
  const runbooks = runbookPages();
  const groups = [...new Set(runbooks.map((page) => page.group))];
  for (const page of runbooks) {
    const body = rewriteLinks(brandProse(page.body), page.source);
    write(
      join(runbookOut, `${page.slug}.md`),
      `${frontmatter(brandProse(page.title), brandProse(page.description))}\n${SOURCE_NOTE(page.source)}\n${body}`,
    );
  }
  const runbookIndex = [
    "# Runbook library",
    "",
    "These pages are the repository\u2019s runbooks, copied from `docs/runbooks` on every build. A runbook records how a part of the product is built, verified and operated, and what it does not yet do. Where a runbook and the [availability](/docs/getting-started/availability) page disagree, the availability page is the summary and the runbook is the detail.",
    "",
    'Some runbooks name numbered build-plan prompts, such as "Prompt 59". Those labels are the repository\u2019s own build plan. [The build plan](/docs/development/build-plan) explains them.',
    "",
  ];
  for (const group of groups) {
    runbookIndex.push(`## ${group}`, "");
    for (const page of runbooks.filter((entry) => entry.group === group)) {
      runbookIndex.push(
        `- [${page.title}](/docs/operations/runbooks/${page.slug}): ${page.description}`,
      );
    }
    runbookIndex.push("");
  }
  write(
    join(runbookOut, "index.md"),
    `${frontmatter("Runbook library", "Every runbook in the repository, grouped by what it covers.")}\n${runbookIndex.join("\n")}`,
  );
  writeMeta(runbookOut, "Runbook library", [
    "index",
    ...groups.flatMap((group) => [
      `---${group}---`,
      ...runbooks.filter((page) => page.group === group).map((page) => page.slug),
    ]),
  ]);
  indexes.runbooks = runbooks.length;

  // Decision records.
  const decisionOut = join(content, "reference/decisions");
  const decisions = decisionPages();
  for (const page of decisions) {
    const body = rewriteLinks(brandProse(page.body), page.source);
    write(
      join(decisionOut, `${page.slug}.md`),
      `${frontmatter(brandProse(page.title), brandProse(page.description))}\n${SOURCE_NOTE(page.source)}\n${body}`,
    );
  }
  write(
    join(decisionOut, "index.md"),
    [
      frontmatter(
        "Decision records",
        "Every architecture decision record, with its status as recorded in the record.",
      ),
      "",
      "An architecture decision record states a decision, its context and its consequences. These pages are copied from `docs/architecture/decisions` on every build. A later record can supersede an earlier one, and each page keeps its own status line.",
      "",
      "| Record | Title | Status |",
      "| --- | --- | --- |",
      ...decisions.map(
        (page) =>
          `| [${page.slug.toUpperCase().slice(0, 7)}](/docs/reference/decisions/${page.slug}) | ${cell(brandProse(page.title))} | ${cell(page.status ? briefly(page.status, 90) : "See the record")} |`,
      ),
      "",
    ].join("\n"),
  );
  writeMeta(decisionOut, "Decision records", ["index", ...decisions.map((page) => page.slug)]);
  indexes.decisions = decisions.length;

  // MCP tools, grouped.
  const toolOut = join(content, "reference/mcp-tools");
  const tools = loadToolSnapshot();
  const grouped = groupTools(tools);
  for (const group of grouped) {
    const sections = group.tools.map((tool) => toolPage(tool, tool.name).text);
    write(
      join(toolOut, `${group.slug}.md`),
      `${frontmatter(group.title, `${group.description} ${group.tools.length} tools.`)}\n${SOURCE_NOTE("crates/vellum/tests/snapshots/mcp-tools.json")}\n${group.description}\n\n${sections.join("\n")}`,
    );
  }
  write(
    join(toolOut, "index.md"),
    [
      frontmatter(
        "MCP tools",
        `Every tool the MCP server offers: ${tools.length} tools in ${grouped.length} groups, with their parameters.`,
      ),
      SOURCE_NOTE("crates/vellum/tests/snapshots/mcp-tools.json"),
      `The MCP server offers ${tools.length} tools. Every client sees the same list. Each tool takes a JSON object whose fields are described on its page. The same tools run from the command line: \`vellum tool <name> --help\` prints a tool\u2019s flags, and \`vellum tool <name> --json\` prints the whole MCP result.`,
      "",
      "| Group | Tools |",
      "| --- | --- |",
      ...grouped.map(
        (group) =>
          `| [${group.title}](/docs/reference/mcp-tools/${group.slug}) | ${group.tools.map((tool) => `\`${tool.name}\``).join(", ")} |`,
      ),
      "",
    ].join("\n"),
  );
  writeMeta(toolOut, "MCP tools", ["index", ...grouped.map((group) => group.slug)]);
  indexes.tools = tools.length;

  // The script API declarations, and the format's generated pages.
  const declarations = readFileSync(
    join(root, "packages/skills/src/generated/vellum-doc.d.ts"),
    "utf8",
  );
  write(
    join(content, "reference/script-api-declarations.md"),
    `${frontmatter("Script API declarations", "The generated TypeScript declarations of the doc object that execute scripts call.")}\n${SOURCE_NOTE("packages/skills/src/generated/vellum-doc.d.ts")}\nThese declarations are generated from the document API registry, the same registry that runs the calls. A script can use only what is declared here.\n\n\`\`\`ts\n${declarations.trimEnd()}\n\`\`\`\n`,
  );
  const formatSpec = splitTitle(readFileSync(join(root, "docs/format/README.md"), "utf8"));
  write(
    join(content, "reference/file-format-spec.md"),
    `${frontmatter(brandProse(formatSpec.title), "The published specification of the document file format, generated from the document types.")}\n${SOURCE_NOTE("docs/format/README.md")}\n${rewriteLinks(brandProse(formatSpec.body), "docs/format/README.md")}`,
  );
  const layoutSpec = splitTitle(readFileSync(join(root, "docs/format/files.md"), "utf8"));
  write(
    join(content, "reference/file-layout.md"),
    `${frontmatter(brandProse(layoutSpec.title), "How a document is laid out on disk: one file, a folder of pages, and the assets beside it.")}\n${SOURCE_NOTE("docs/format/files.md")}\n${rewriteLinks(brandProse(layoutSpec.body), "docs/format/files.md")}`,
  );

  write(join(content, "reference/keyboard-shortcuts.md"), keyboardPage());

  const threat = splitTitle(readFileSync(join(root, "docs/architecture/threat-model.md"), "utf8"));
  const threatLead =
    threat.body
      .split(/\n\s*\n/)
      .map((block) => block.trim())
      .find((block) => block && !block.startsWith("#")) ?? threat.title;
  write(
    join(content, "architecture/threat-model.md"),
    `${frontmatter(brandProse(threat.title), briefly(brandProse(threatLead), 200))}\n${SOURCE_NOTE("docs/architecture/threat-model.md")}\n${rewriteLinks(brandProse(threat.body), "docs/architecture/threat-model.md")}`,
  );

  // The index for agents (served at /llms.txt).
  indexes.agentIndex = writeAgentIndex();
  logger(
    `docs-content: ${indexes.runbooks} runbooks, ${indexes.decisions} decision records, ${indexes.tools} MCP tools.`,
  );
  return indexes;
}

/** A frontmatter value: a JSON string (what this script writes) or a plain YAML scalar. */
function parseScalar(value) {
  if (value === undefined) return undefined;
  try {
    return JSON.parse(value);
  } catch {
    return value.replace(/^(['"])(.*)\1$/, "$2");
  }
}

/** Every page of the site, with its address and its frontmatter. */
export function listPages(dir = content) {
  const pages = [];
  const walk = (folder) => {
    for (const entry of readdirSync(folder, { withFileTypes: true })) {
      const path = join(folder, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (/\.mdx?$/.test(entry.name)) pages.push(path);
    }
  };
  walk(dir);
  return pages.map((path) => {
    const rel = relative(dir, path).split("\\").join("/");
    const stem = rel.replace(/\.mdx?$/, "");
    const url = stem === "index" ? "/docs" : `/docs/${stem.replace(/\/index$/, "")}`;
    const text = readFileSync(path, "utf8");
    const fm = text.match(/^---\n([\s\S]*?)\n---\n/);
    const field = (name) => fm?.[1].match(new RegExp(`^${name}: (.*)$`, "m"))?.[1];
    return {
      path,
      rel,
      url,
      title: parseScalar(field("title")),
      description: parseScalar(field("description")),
      body: text.slice(fm ? fm[0].length : 0),
    };
  });
}

/** The index served at /llms.txt: one line per page, in section order. */
export function writeAgentIndex() {
  const pages = listPages().sort((a, b) => a.url.localeCompare(b.url));
  const lines = [
    `# ${brand.name} documentation`,
    "",
    `${brand.tagline} ${brand.name} is an open-source design canvas in the repository. These pages cover installing it, designing with agents, the CLI, every MCP tool, the file format, the runbooks, the decisions and the architecture.`,
    "",
    "## Pages",
    "",
    ...pages
      .filter((page) => page.title && page.description)
      .map((page) => `- [${page.title}](${page.url}): ${page.description}`),
    "",
  ];
  const text = lines.join("\n").split(BRAND_TOKEN).join(brand.name);
  mkdirSync(join(app, ".generated"), { recursive: true });
  writeFileSync(
    join(app, ".generated/agent-docs.ts"),
    `// Generated by scripts/docs-content.mjs. Do not edit.\nexport const documentationIndex = ${JSON.stringify(text)};\n`,
  );
  return pages.length;
}

// ---- checking the pages -------------------------------------------------------------------------------------

/** The problems in the site's pages. An empty list means the site is consistent. */
export function checkContent({ dir = content } = {}) {
  const pages = listPages(dir);
  const urls = new Set(pages.map((page) => page.url));
  const problems = [];
  for (const page of pages) {
    if (!page.title || !page.description)
      problems.push(`${page.rel}: frontmatter needs a title and a description`);
    const text = page.body.split(BRAND_TOKEN).join(brand.name);
    const prose = page.body.replace(/```[\s\S]*?```/g, "").replace(/`[^`\n]*`/g, "");
    if (/\{\{[a-z]+\}\}/.test(prose.replace(/\{\{brand\}\}/g, ""))) {
      problems.push(`${page.rel}: an unknown token is left in the page`);
    }
    for (const match of page.body.matchAll(/\]\((\/docs[^)\s#]*)(#[^)\s]*)?\)/g)) {
      const target = match[1].replace(/\/$/, "") || "/docs";
      if (!urls.has(target)) problems.push(`${page.rel}: the link ${target} has no page`);
    }
    for (const match of page.body.matchAll(/\[[^\]]*\]\((\/[^)\s]*)\)/g)) {
      if (!match[1].startsWith("/docs") && !match[1].startsWith("/llms")) {
        problems.push(`${page.rel}: the link ${match[1]} is outside the documentation`);
      }
    }
    if (
      !page.rel.startsWith("operations/runbooks/") &&
      !page.rel.startsWith("reference/decisions/") &&
      !page.rel.startsWith("reference/mcp-tools/")
    ) {
      if (/\bVellum\b/.test(prose))
        problems.push(`${page.rel}: write ${BRAND_TOKEN} for the product name in prose`);
    }
    if (/\b(magic|stunning|beautiful)\b/i.test(prose))
      problems.push(`${page.rel}: drop the claim-of-taste wording`);
    if (/TODO|TBD/.test(text))
      problems.push(`${page.rel}: a placeholder (TODO or TBD) is left in the page`);
    if (/\w!(\s|$)/m.test(prose))
      problems.push(`${page.rel}: remove the exclamation mark from prose`);
  }
  const meta = [...listMeta(dir)];
  for (const { file, pages: entries } of meta) {
    for (const entry of entries) {
      if (typeof entry !== "string" || entry.startsWith("---")) continue;
      const exists =
        existsSync(join(dirname(file), `${entry}.md`)) ||
        existsSync(join(dirname(file), `${entry}.mdx`)) ||
        existsSync(join(dirname(file), entry, "index.md"));
      if (!exists)
        problems.push(`${relative(dir, file)}: the navigation entry ${entry} has no page`);
    }
  }
  return problems;
}

function* listMeta(folder) {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) yield* listMeta(path);
    else if (entry.name === "meta.json") {
      yield { file: path, pages: JSON.parse(readFileSync(path, "utf8")).pages ?? [] };
    }
  }
}

// ---- the command --------------------------------------------------------------------------------------------

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  prepareContent({ logger: (line) => process.stdout.write(`${line}\n`) });
  const problems = checkContent();
  if (problems.length > 0) {
    process.stderr.write(`${problems.join("\n")}\n`);
    process.exit(1);
  }
}
