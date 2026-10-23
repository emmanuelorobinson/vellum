/**
 * Step rows (DESIGN.md "Step Timeline"). A recorded step is data (a verb code, a target id and name, counts); the
 * line the panel shows is written here, in the panel's own copy, so the daemon never words the UI.
 */
import type { StepBody, StepDetail, Verb } from "./log";

export type StepState = "working" | "done" | "warning" | "failed";
export type StepKind =
  | "read"
  | "designed"
  | "screenshot"
  | "layout"
  | "script"
  | "claim"
  | "change"
  | "progress"
  | "other";

export interface StepView {
  /** Stable within a turn: the event's seq, or the live call id while it runs. */
  key: string;
  tool: string;
  verb: Verb | null;
  kind: StepKind;
  icon: string;
  line: string;
  state: StepState;
  duration: string;
  /** A working step past STUCK_AFTER_MS (S10): the row says so; nothing is stopped for it. */
  stuck: boolean;
  /** Designed: the frame after the step. Screenshot: the image the agent looked at. `image` is a cache hash. */
  thumbnail: { node: string | null; name: string | null; image: string | null } | null;
  verdict: string | null;
  problems: string | null;
  script: string | null;
  error: string | null;
  /** Expanded details: targets in mono, counts, raw output. */
  targets: string[];
  counts: { created: number; changed: number; removed: number; variables: number };
  arguments: string;
  output: string;
  seq: number | null;
}

/** As vellum-core `turns::STUCK_AFTER_MS`. */
export const STUCK_AFTER_MS = 120_000;

const ICONS: Record<Verb, string> = {
  read: "file-text",
  opened: "file",
  checked_layout: "ruler",
  checked_screenshot: "scan-eye",
  designed: "layout-template",
  added_asset: "image-plus",
  updated: "sliders-horizontal",
  copied: "copy",
  moved: "move",
  replaced: "replace",
  deleted: "trash",
  updated_variables: "variable",
  made_component: "component",
  made_slot: "square-dashed",
  claimed: "lock",
  released: "lock-open",
  ran_script: "square-terminal",
  progress: "list-checks",
};
/** The verb a tool's step has, for a step still running (its descriptor arrives when it finishes). */
const TOOL_VERBS: Record<string, Verb> = {
  read_file: "read",
  read_skill: "read",
  get_app_state: "read",
  get_nodes: "read",
  query_nodes: "read",
  find_properties: "read",
  get_variables: "read",
  get_spec: "read",
  get_style: "read",
  list_components: "read",
  list_fonts: "read",
  list_skills: "read",
  search_icons: "read",
  find_space: "read",
  open_document: "opened",
  snapshot_layout: "checked_layout",
  screenshot: "checked_screenshot",
  insert_nodes: "designed",
  execute: "ran_script",
  fill_image: "added_asset",
  insert_icon: "added_asset",
  import: "added_asset",
  update_nodes: "updated",
  replace_properties: "updated",
  copy_nodes: "copied",
  move_nodes: "moved",
  replace_nodes: "replaced",
  delete_nodes: "deleted",
  set_variables: "updated_variables",
  import_tokens: "updated_variables",
  create_component: "made_component",
  create_instance: "made_component",
  instance_nodes: "made_component",
  manage_components: "made_component",
  make_slot: "made_slot",
  arrange_nodes: "updated",
  manage_pages: "updated",
  claim_frame: "claimed",
  release_frame: "released",
  report_progress: "progress",
};
const KINDS: Partial<Record<Verb, StepKind>> = {
  read: "read",
  opened: "read",
  designed: "designed",
  checked_screenshot: "screenshot",
  checked_layout: "layout",
  ran_script: "script",
  claimed: "claim",
  released: "claim",
  progress: "progress",
};
/** What a running step is doing, before its target is known. */
const WORKING: Partial<Record<Verb, string>> = {
  read: "Reading",
  opened: "Opening a document",
  checked_layout: "Checking layout",
  checked_screenshot: "Checking a screenshot",
  designed: "Designing",
  added_asset: "Adding an asset",
  updated: "Updating layers",
  copied: "Copying layers",
  moved: "Moving layers",
  replaced: "Replacing layers",
  deleted: "Deleting layers",
  updated_variables: "Updating tokens",
  made_component: "Making a component",
  made_slot: "Making a slot",
  claimed: "Claiming a frame",
  released: "Releasing a frame",
  ran_script: "Running a script",
  progress: "Reporting progress",
};
/** Running steps whose line names their target ("Designing Step 1"). */
const TARGETED = new Set<Verb>([
  "designed",
  "checked_layout",
  "checked_screenshot",
  "claimed",
  "released",
]);
/** The node a running step works on, from its arguments. */
export function targetOf(args: unknown): string | null {
  if (!args || typeof args !== "object") return null;
  const a = args as Record<string, unknown>;
  for (const key of ["parent", "frame", "node"])
    if (typeof a[key] === "string") return a[key] as string;
  return null;
}
/** A failed request names what was asked for, in plain words. */
const FAILED: Record<string, string> = {
  fill_image: "Image request failed",
  insert_icon: "Icon request failed",
  import: "Import failed",
  execute: "Script failed",
  screenshot: "Screenshot failed",
  export: "Export failed",
};

/** MCP clients prefix their tool names ("mcp__vellum__insert_nodes"); the registry's name is the last part. */
export const toolName = (name: string) => {
  const at = name.lastIndexOf("__");
  return at < 0 ? name : name.slice(at + 2);
};
export const verbOf = (tool: string): Verb | null => TOOL_VERBS[toolName(tool)] ?? null;

export function formatDuration(ms: number): string {
  const s = Math.max(1, Math.round(ms / 1000));
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} m ${s % 60} s`;
}
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${Math.round(n / 1000)}k`;
  return String(n);
}
const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** The arguments preview is compact JSON, possibly cut; only whole JSON is read. */
function args(text: string): Record<string, unknown> {
  try {
    const v = JSON.parse(text);
    return v && typeof v === "object" && !Array.isArray(v) ? v : {};
  } catch {
    return {};
  }
}

function lineOf(
  tool: string,
  verb: Verb | null,
  d: StepDetail,
  a: Record<string, unknown>,
): string {
  const target = d.target?.name;
  const c = d.counts ?? { created: 0, changed: 0, removed: 0, variables: 0 };
  const changes = c.created + c.changed + c.removed + c.variables;
  const layers = (n: number) => plural(n, "layer");
  switch (verb) {
    case "read": {
      if (tool === "read_skill")
        return typeof a.path === "string" && !/guideline/i.test(a.path)
          ? `Read skill ${a.path}`
          : "Read design-system guidelines";
      if (typeof a.path === "string") return `Read ${a.path}`;
      if (tool === "get_variables") return "Read variables";
      if (tool === "get_spec") return target ? `Read the spec of ${target}` : "Read the spec";
      if (tool === "get_style")
        return typeof a.name === "string" ? `Read style ${a.name}` : "Read a style";
      if (tool === "list_components") return "Read components";
      if (tool === "list_fonts") return "Read fonts";
      if (tool === "list_skills") return "Read skills";
      if (tool === "search_icons") return "Searched icons";
      if (tool === "find_space") return "Found space on the canvas";
      return target ? `Read ${target}` : "Read the document";
    }
    case "opened":
      return d.document ? `Opened ${d.document}` : "Opened a document";
    case "checked_layout":
      return "Checked layout";
    case "checked_screenshot":
      return "Checked screenshot";
    case "designed":
      return target ? `Designed ${target} · ${layers(c.created)}` : `Designed ${layers(c.created)}`;
    case "added_asset":
      return target
        ? `Added ${tool === "insert_icon" ? "an icon" : "an image"} to ${target}`
        : "Added an asset";
    case "updated":
      return target ? `Updated ${target} · ${layers(c.changed)}` : `Updated ${layers(c.changed)}`;
    case "copied":
      return `Copied ${layers(c.created)}`;
    case "moved":
      return `Moved ${layers(c.changed)}`;
    case "replaced":
      return `Replaced ${layers(c.removed || c.changed)}`;
    case "deleted":
      return `Deleted ${layers(c.removed)}`;
    case "updated_variables":
      return `Updated ${plural(c.variables, "token")}`;
    case "made_component":
      return target ? `Made component ${target}` : "Made a component";
    case "made_slot":
      return target ? `Made a slot in ${target}` : "Made a slot";
    case "claimed":
      return target ? `Claimed frame ${target}` : "Claimed a frame";
    case "released":
      return target ? `Released frame ${target}` : "Released a frame";
    case "ran_script":
      return `Ran script · ${plural(changes, "change")}`;
    case "progress":
      return (
        d.message ??
        (d.progress ? `${d.progress[0]} of ${d.progress[1]} done` : "Reported progress")
      );
    default:
      return tool === "export" ? "Exported" : tool;
  }
}

/** A recorded step. */
export function describeStep(body: StepBody, seq: number | null = null): StepView {
  const d = body.detail ?? {};
  const tool = toolName(body.tool);
  const verb = d.verb ?? verbOf(tool);
  const a = args(body.arguments);
  const state: StepState =
    body.outcome === "done" ? "done" : body.outcome === "rejected" ? "warning" : "failed";
  const line =
    state === "failed"
      ? (FAILED[tool] ?? `${lineOf(tool, verb, d, a)} · failed`)
      : lineOf(tool, verb, d, a);
  const kind = (verb && KINDS[verb]) ?? (verb ? "change" : "other");
  const picture = body.thumbnail ?? null;
  const counts = d.counts ?? { created: 0, changed: 0, removed: 0, variables: 0 };
  return {
    key: seq === null ? `${tool}:${body.startedAt}` : String(seq),
    tool,
    verb,
    kind,
    icon: (verb && ICONS[verb]) ?? "circle",
    line,
    state,
    duration: formatDuration(body.durationMs),
    stuck: false,
    thumbnail:
      d.thumbnail || picture
        ? {
            // A checked screenshot shows the frame it looked at.
            node: d.thumbnail?.node ?? (kind === "screenshot" ? (d.target?.id ?? null) : null),
            name: d.target?.name ?? null,
            image: picture,
          }
        : null,
    verdict: kind === "screenshot" ? (d.message ?? null) : null,
    problems: kind === "layout" ? (d.message ?? null) : null,
    script: d.script ?? null,
    error: state === "done" ? null : (d.error ?? null),
    targets: [d.target ? `${d.target.name} · ${d.target.id}` : null, d.document ?? null].filter(
      (t): t is string => !!t,
    ),
    counts,
    arguments: body.arguments,
    output: body.result,
    seq,
  };
}

/** A step still running, from the engine's live stream. */
export function workingStep(
  call: string,
  tool: string,
  startedAt: number,
  now: number,
  target: string | null = null,
): StepView {
  const name = toolName(tool);
  const verb = verbOf(name);
  const doing = (verb && WORKING[verb]) ?? name;
  return {
    key: `live:${call}`,
    tool: name,
    verb,
    kind: (verb && KINDS[verb]) ?? (verb ? "change" : "other"),
    icon: (verb && ICONS[verb]) ?? "circle",
    line: target && TARGETED.has(verb as Verb) ? `${doing.split(" ")[0]} ${target}` : doing,
    state: "working",
    duration: formatDuration(Math.max(0, now - startedAt)),
    stuck: now - startedAt > STUCK_AFTER_MS,
    thumbnail: null,
    verdict: null,
    problems: null,
    script: null,
    error: null,
    targets: [],
    counts: { created: 0, changed: 0, removed: 0, variables: 0 },
    arguments: "",
    output: "",
    seq: null,
  };
}

/** The line of a group of consecutive steps of one tool ("Read 3 files"). */
export function groupLine(steps: readonly StepView[]): string {
  const first = steps[0] as StepView;
  const n = steps.length;
  if (first.tool === "read_file") return `Read ${n} files`;
  if (first.tool === "read_skill") return `Read ${n} skills`;
  const same = steps.every((s) => s.targets[0] === first.targets[0]);
  const target = same && first.targets[0] ? first.targets[0].split(" · ")[0] : null;
  if (first.verb === "updated" && target) return `Updated ${target} · ${n} steps`;
  return `${first.line.split(" · ")[0]} · ${n} steps`;
}
