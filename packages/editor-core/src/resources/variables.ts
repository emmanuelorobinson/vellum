import type {
  Document,
  Node,
  NodeIdPath,
  Variable,
  VariableId,
  VariableType,
} from "@vellum/types-ts";
import { defaultMode, type Mode } from "../design/bindings";
import { normalizeHex } from "../design/colour";
import type { ToolOp } from "../tool-types";

type Entry = { value?: unknown; $alias?: string; when?: Mode };
export interface ThemeColumn {
  label: string;
  mode: Mode;
}
/** Presentation order is stable across the codec's map sorting: Mode first, then other axes by name. */
export const themeAxes = (doc: Document): [string, string[]][] =>
  Object.entries(doc.themes ?? {}).sort(([a], [b]) =>
    a === b ? 0 : a === "Mode" ? -1 : b === "Mode" ? 1 : a < b ? -1 : 1,
  );
/** Columns are grouped by outer axes, with the first axis varying fastest. Refuse an unbounded table. */
export function themeColumns(doc: Document): ThemeColumn[] {
  let modes: Mode[] = [{}];
  for (const [axis, values] of themeAxes(doc)) {
    if (!values.length || modes.length * values.length > 128)
      throw Error("Theme table requires 1–128 combinations");
    modes = values.flatMap((value) => modes.map((mode) => ({ ...mode, [axis]: value })));
  }
  const axes = themeAxes(doc).map(([axis]) => axis);
  return modes.map((mode) => ({ mode, label: axes.map((a) => mode[a]).join(" / ") || "Default" }));
}
export const variableCollections = (doc: Document): string[] => [
  ...new Set(Object.values(doc.variables ?? {}).map((v) => v.name.split("/")[0] ?? v.name)),
];
export function variableEntry(doc: Document, id: VariableId, mode: Mode): Entry | undefined {
  return doc.variables?.[id]?.values
    .filter((e) => Object.entries(e.when ?? {}).every(([k, v]) => mode[k] === v))
    .at(-1);
}
const sameMode = (a: Mode, b: Mode) =>
  Object.keys(a).length === Object.keys(b).length &&
  Object.entries(a).every(([k, v]) => b[k] === v);
export function parseVariableValue(type: VariableType, text: string): unknown {
  if (type === "string") return text;
  if (type === "number") {
    if (!text.trim() || !Number.isFinite(Number(text))) throw Error("Enter a finite number");
    return Number(text);
  }
  if (type === "boolean") {
    if (text !== "true" && text !== "false") throw Error("Enter true or false");
    return text === "true";
  }
  const colour = normalizeHex(text);
  if (!colour) throw Error("Enter a colour");
  return colour;
}
/** UI preflight only. The Rust op layer remains the authority for type, cycle and binding validation. */
export function editVariable(doc: Document, id: VariableId, mode: Mode, text: string): ToolOp {
  const variable = doc.variables?.[id];
  if (!variable) throw Error("Missing variable");
  for (const [axis, value] of Object.entries(mode))
    if (!doc.themes?.[axis]?.includes(value)) throw Error("Unknown theme value");
  const requestedAlias = text.startsWith("$") ? text.slice(1) : null;
  const alias = requestedAlias
    ? Object.hasOwn(doc.variables ?? {}, requestedAlias)
      ? requestedAlias
      : (Object.entries(doc.variables ?? {}).find(([, v]) => v.name === requestedAlias)?.[0] ??
        requestedAlias)
    : null;
  if (alias && doc.variables?.[alias]?.type !== variable.type)
    throw Error("Choose a variable of the same type");
  const entry: Entry = {
    ...(alias ? { $alias: alias } : { value: parseVariableValue(variable.type, text) }),
    ...(Object.keys(mode).length ? { when: { ...mode } } : {}),
  };
  const values = variable.values.filter((e) => !sameMode(e.when ?? {}, mode));
  const next = { ...variable, values: [...values, entry] } as Variable;
  const draft = { ...doc, variables: { ...doc.variables, [id]: next } };
  for (const column of themeColumns(draft)) {
    const seen = new Set<string>();
    let key: string | undefined = id;
    while (key !== undefined) {
      if (seen.has(key)) throw Error("Alias cycle");
      seen.add(key);
      key = variableEntry(draft, key, column.mode)?.$alias;
    }
  }
  return { type: "setVariables", set: { [id]: next }, remove: [] };
}
export function addThemeAxis(doc: Document, axis: string, first: string): Record<string, string[]> {
  axis = axis.trim();
  first = first.trim();
  if (!axis || !first || Object.hasOwn(doc.themes ?? {}, axis))
    throw Error("Use a new axis name and a value");
  return { ...doc.themes, [axis]: [first] };
}
export function addThemeValue(
  doc: Document,
  axis: string,
  value: string,
): Record<string, string[]> {
  value = value.trim();
  const values = doc.themes?.[axis];
  if (!values || !value || values.includes(value)) throw Error("Use a new theme value");
  const themes = { ...doc.themes, [axis]: [...values, value] };
  themeColumns({ ...doc, themes });
  return themes;
}
/** Read-only inspection for panel values; scene layout and rendering still belong to the engine. */
export function frameTheme(
  doc: Document | null,
  address: NodeIdPath | null,
  get: (id: NodeIdPath) => { node: Node; parent: NodeIdPath | null } | undefined | null,
): Mode {
  const mode = defaultMode(doc),
    chain: Node[] = [];
  const seen = new Set<string>();
  while (address && !seen.has(address)) {
    seen.add(address);
    const item = get(address);
    if (!item) break;
    chain.unshift(item.node);
    address = item.parent;
  }
  for (const node of chain)
    for (const [axis, value] of Object.entries((node as { theme?: Mode }).theme ?? {}))
      if (doc?.themes?.[axis]?.includes(value)) mode[axis] = value;
  return mode;
}
