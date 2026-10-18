import type { Document, VariableId } from "@vellum/types-ts";

/** The value of each theme axis a layer is shown under: `{"Mode": "Dark"}`. */
export type Mode = Record<string, string>;
export type VariableType = "color" | "number" | "string" | "boolean";

export interface VariableInfo {
  id: VariableId;
  /** By convention a path: `space/4`. */
  name: string;
  type: VariableType;
  /** What it is under the mode asked for; undefined when it cannot be worked out (a cycle). */
  value: unknown;
}
/** A bound field: the variable, or `name: null` when the file refers to one that is not there. */
export interface VariableBinding {
  id: VariableId;
  name: string | null;
  value: unknown;
}
export type FieldValue<T> =
  | { kind: "value"; value: T; isDefault?: true }
  | { kind: "bound"; variable: VariableBinding }
  | { kind: "mixed" }
  | { kind: "none" };

type Entry = { value?: unknown; $alias?: VariableId; when?: Record<string, string> };
type Var = { type: VariableType; name: string; values: Entry[] };
const variablesOf = (doc: Document | null): Record<string, Var> =>
  (doc?.variables ?? {}) as unknown as Record<string, Var>;

export const isVarRef = (v: unknown): v is { $var: VariableId } =>
  typeof v === "object" &&
  v !== null &&
  Object.keys(v).length === 1 &&
  typeof (v as { $var?: unknown }).$var === "string";

/** The first value of each axis, which is what a layer with no theme of its own is shown under. */
export function defaultMode(doc: Document | null): Mode {
  const themes = (doc?.themes ?? {}) as Record<string, string[]>;
  const mode: Mode = {};
  for (const [axis, values] of Object.entries(themes))
    if (values[0] !== undefined) mode[axis] = values[0];
  return mode;
}

/** The last entry whose conditions the mode satisfies wins. A seen-id set terminates cycles. */
export function resolveVariable(doc: Document | null, id: VariableId, mode: Mode): unknown {
  const variables = variablesOf(doc),
    seen = new Set<string>();
  while (!seen.has(id)) {
    seen.add(id);
    const variable = variables[id];
    if (!variable) return undefined;
    const entry = variable.values
      .filter((e) => Object.entries(e.when ?? {}).every(([axis, value]) => mode[axis] === value))
      .at(-1);
    if (!entry) return undefined;
    if (entry.$alias === undefined) return entry.value;
    id = entry.$alias;
  }
  return undefined;
}

export function listVariables(
  doc: Document | null,
  type: VariableType,
  mode: Mode,
): VariableInfo[] {
  return Object.entries(variablesOf(doc))
    .filter(([, v]) => v.type === type)
    .map(([id, v]) => ({ id, name: v.name, type, value: resolveVariable(doc, id, mode) }))
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
}

export function bindingOf(doc: Document | null, id: VariableId, mode: Mode): VariableBinding {
  const variable = variablesOf(doc)[id];
  return {
    id,
    name: variable ? variable.name : null,
    value: variable ? resolveVariable(doc, id, mode) : undefined,
  };
}

/**
 * What a field shows for the layers selected: a shared literal, a shared binding, the default when nothing is
 * set, or Mixed. A bound layer next to a literal one is mixed even when the numbers happen to agree.
 */
export function readValue<T>(
  raws: readonly (T | { $var: VariableId } | undefined)[],
  doc: Document | null,
  mode: Mode,
  fallback: T,
): FieldValue<T> {
  const [first, ...rest] = raws;
  if (!raws.length) return { kind: "none" };
  const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
  if (!rest.every((r) => same(r, first))) return { kind: "mixed" };
  if (isVarRef(first)) return { kind: "bound", variable: bindingOf(doc, first.$var, mode) };
  if (first === undefined) return { kind: "value", value: fallback, isDefault: true };
  return { kind: "value", value: first as T };
}

/** The Variable chip's text: `{space/4} 16`. */
export function describeValue(binding: VariableBinding, suffix: string): string {
  if (binding.name === null) return "Missing variable";
  const v = binding.value;
  const shown =
    typeof v === "number" ? String(Math.round(v * 100) / 100) : v === undefined ? "" : String(v);
  return `{${binding.name}}${shown ? ` ${shown}${suffix.trim() === "px" ? "" : suffix}` : ""}`;
}
