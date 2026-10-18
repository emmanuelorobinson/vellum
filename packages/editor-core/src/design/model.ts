import type { Document, Effect, NodeKind, Paint, Stroke, Value } from "@vellum/types-ts";
import type { SceneItem } from "../tool-types";
import {
  bindingOf,
  defaultMode,
  type FieldValue,
  isVarRef,
  type Mode,
  readValue,
  resolveVariable,
  type VariableBinding,
} from "./bindings";
import { type ComponentModel, readComponent } from "./component";
import { effectLabel, effectSummary, effectVisible } from "./effects";
import {
  type PaintKind,
  paintKind,
  paintLabel,
  paintOpacity,
  paintSwatch,
  paintVisible,
} from "./paint";
import { DEFAULT_ALIGN, DEFAULT_CAP, DEFAULT_JOIN, formatDashes, strokeSummary } from "./stroke";
import { readText, type TextContext, type TextModel } from "./text";

export type SectionId =
  | "notes"
  | "position"
  | "layout"
  | "appearance"
  | "fill"
  | "stroke"
  | "effects"
  | "text"
  | "component"
  | "content"
  | "export";

/** A selected layer, and whether its parent's layout (not the layer) decides where it sits. */
export interface DesignTarget {
  item: SceneItem;
  inLayout: boolean;
}
export interface DesignInput {
  targets: readonly DesignTarget[];
  document: Document | null;
  /** The theme the layers are shown under; the first value of each axis when absent. */
  mode?: Mode;
  readOnly: boolean;
  text: TextContext;
  /** A text layer is open for typing: the panel shows Text, Fill and Content only (C14). */
  editing?: boolean;
}

/* ------------------------------------------------------------------------------------------ sections */

const DESIGN_KINDS = new Set<NodeKind>([
  "frame",
  "group",
  "rect",
  "ellipse",
  "polygon",
  "star",
  "line",
  "vector",
  "boolean",
  "text",
  "image",
  "icon",
  "instance",
]);
const NO_FILL = new Set<NodeKind>(["group", "line"]);
const NO_STROKE = new Set<NodeKind>(["group"]);

export function sectionsFor(
  kinds: readonly NodeKind[],
  options: { main?: boolean; editing?: boolean } = {},
): SectionId[] {
  if (!kinds.length) return [];
  const all = (k: NodeKind) => kinds.every((x) => x === k);
  if (options.editing && all("text")) return ["text", "fill", "content"];
  if (!kinds.every((k) => DESIGN_KINDS.has(k))) return ["position", "layout"];
  const out: SectionId[] = [];
  if (options.main) out.push("component");
  else if (all("text")) out.push("text");
  else if (all("instance")) out.push("component");
  else if (all("frame")) out.push("notes");
  out.push("position", "layout", "appearance");
  if (!kinds.some((k) => NO_FILL.has(k))) out.push("fill");
  if (!kinds.some((k) => NO_STROKE.has(k))) out.push("stroke");
  out.push("effects", "export");
  return out;
}

/* ----------------------------------------------------------------------------------------- the model */

export interface PositionField {
  value: FieldValue<number>;
  editable: boolean;
  reason?: string;
}
export interface SizeAxis {
  mode: "hug" | "fill" | "fixed";
  /** The size in pixels: the fixed one, or what hug or fill gives now. */
  value: number;
  mixed: boolean;
  bound?: VariableBinding;
}
export interface PaintRow {
  index: number;
  kind: PaintKind;
  /** "Color", the variable's own name for a bound solid, or the paint's kind. */
  label: string;
  /** What to draw in the swatch (a colour or a CSS gradient); null for an image. */
  swatch: string | null;
  /** The hex of a solid, resolved through its variable; empty for the other kinds. */
  text: string;
  opacity: number;
  visible: boolean;
  bound: VariableBinding | null;
}
export type PaintsModel = { kind: "list"; rows: PaintRow[] } | { kind: "mixed" };
export interface EffectRow {
  index: number;
  kind: Effect["type"];
  label: string;
  summary: string;
  visible: boolean;
}
export type EffectsModel = { kind: "list"; rows: EffectRow[] } | { kind: "mixed" };
export type StrokeModel =
  | { kind: "none" }
  | { kind: "mixed" }
  | {
      kind: "stroke";
      rows: PaintRow[];
      summary: string;
      /** One width for all sides. */
      uniform: boolean;
      /** Top, right, bottom, left. */
      widths: [Value<number>, Value<number>, Value<number>, Value<number>];
      align: string;
      cap: string;
      join: string;
      dash: string;
      dashes: number[];
    };
export interface PaddingModel {
  /** One value drives all four sides. */
  linked: boolean;
  all: FieldValue<number>;
  /** Top, right, bottom, left. */
  sides: [FieldValue<number>, FieldValue<number>, FieldValue<number>, FieldValue<number>];
}
export interface LayoutModel {
  mode: FieldValue<string>;
  gap: FieldValue<number>;
  padding: PaddingModel;
  align: FieldValue<string>;
  justify: FieldValue<string>;
  grid: { columns: FieldValue<number | null>; rows: FieldValue<number | null> } | null;
}
export interface DesignModel {
  count: number;
  kinds: NodeKind[];
  readOnly: boolean;
  sections: SectionId[];
  notes: FieldValue<string>;
  position: { x: PositionField; y: PositionField; rotation: PositionField };
  size: { width: SizeAxis; height: SizeAxis };
  layout: LayoutModel | null;
  appearance: {
    opacity: FieldValue<number>;
    radius: FieldValue<number> | null;
    clip: FieldValue<boolean> | null;
    blend: FieldValue<string>;
  };
  fills: PaintsModel;
  stroke: StrokeModel;
  effects: EffectsModel;
  text: TextModel | null;
  component: ComponentModel | null;
}

type Loose = Record<string, unknown>;
const props = (t: DesignTarget) => t.item.node as unknown as Loose;
const json = (v: unknown) => JSON.stringify(v ?? null);
const allSame = (values: readonly unknown[]) => values.every((v) => json(v) === json(values[0]));

/** The last part of a variable's path, as a label: `color/surface` is "Surface". */
export function variableLabel(name: string): string {
  const last = name.split("/").at(-1) ?? name;
  const spaced = last.replace(/[-_]+/g, " ");
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

export function readDesign(input: DesignInput): DesignModel {
  const { targets, document: doc } = input;
  const mode = input.mode ?? defaultMode(doc);
  const items = targets.map((t) => t.item);
  const kinds = [...new Set(items.map((i) => i.node.type))] as NodeKind[];
  const read = <T>(pick: (p: Loose) => unknown, fallback: T) =>
    readValue(
      targets.map((t) => pick(props(t)) as T | { $var: string } | undefined),
      doc,
      mode,
      fallback,
    );
  const number = (v: Value<number>): number =>
    isVarRef(v) ? ((resolveVariable(doc, v.$var, mode) as number | undefined) ?? 0) : v;
  const colour = (c: Value<string>): string =>
    isVarRef(c) ? ((resolveVariable(doc, c.$var, mode) as string | undefined) ?? "#00000000") : c;
  const allFrames = kinds.length > 0 && kinds.every((k) => k === "frame");
  const component = targets.length === 1 ? readComponent(items, doc) : null;

  /* position */
  const stuck = targets.some(
    (t) => t.inLayout && (props(t).position as { type?: string } | undefined)?.type !== "absolute",
  );
  const editable = !input.readOnly && !stuck;
  const reason = input.readOnly
    ? "This file is read-only"
    : stuck
      ? "Placed by its parent's layout"
      : undefined;
  const axis = (key: "x" | "y"): PositionField => {
    const raws = targets.map((t) => {
      const p = props(t).position as { type?: string; x?: number; y?: number } | undefined;
      if (p?.type === "absolute") return p[key];
      return t.inLayout ? t.item.bounds[key] : undefined;
    });
    return {
      value: readValue(raws, doc, mode, 0),
      editable,
      ...(reason ? { reason } : {}),
    };
  };

  /* size */
  const sizeAxis = (key: "width" | "height"): SizeAxis => {
    const parts = targets.map((t) => {
      const raw = props(t)[key] as unknown;
      const bounds = t.item.bounds[key];
      if (typeof raw === "number") return { mode: "fixed" as const, value: raw };
      if (raw === "hug" || (typeof raw === "object" && raw !== null && "hug" in raw))
        return { mode: "hug" as const, value: bounds };
      if (raw === "fill" || (typeof raw === "object" && raw !== null && "fill" in raw))
        return { mode: "fill" as const, value: bounds };
      return {
        mode: "fixed" as const,
        value: bounds,
        bound: isVarRef(raw) ? bindingOf(doc, raw.$var, mode) : undefined,
      };
    });
    const first = parts[0] ?? { mode: "fixed" as const, value: 0 };
    const mixed = !allSame(parts.map((p) => [p.mode, p.mode === "fixed" ? p.value : null]));
    const bound = "bound" in first ? first.bound : undefined;
    return {
      mode: first.mode,
      value: first.value,
      mixed,
      ...(bound && !mixed ? { bound } : {}),
    };
  };

  /* layout */
  let layout: LayoutModel | null = null;
  if (allFrames) {
    const sidesOf = (p: Loose): Value<number>[] => {
      const raw = p.padding as Value<number> | Value<number>[] | undefined;
      if (raw === undefined) return [0, 0, 0, 0];
      if (!Array.isArray(raw)) return [raw, raw, raw, raw];
      return raw.length === 2
        ? ([raw[0], raw[1], raw[0], raw[1]] as Value<number>[])
        : (raw as Value<number>[]);
    };
    const per = targets.map((t) => sidesOf(props(t)));
    const side = (i: number) =>
      readValue(
        per.map((s) => s[i] as Value<number>),
        doc,
        mode,
        0,
      );
    const linked = per.every((s) => allSame(s));
    const mode_ = read((p) => p.layout, "none");
    layout = {
      mode: mode_,
      gap: read((p) => p.gap, 0),
      padding: {
        linked,
        all: readValue(
          per.map((s) => s[0] as Value<number>),
          doc,
          mode,
          0,
        ),
        sides: [side(0), side(1), side(2), side(3)],
      },
      align: read((p) => p.align, "start"),
      justify: read((p) => p.justify, "start"),
      grid:
        mode_.kind === "value" && mode_.value === "grid"
          ? {
              columns: read<number | null>((p) => p.columns, null),
              rows: read<number | null>((p) => p.rows, null),
            }
          : null,
    };
  }

  /* appearance */
  const hasRadius =
    kinds.length > 0 && kinds.every((k) => k === "frame" || k === "rect" || k === "polygon");
  const radius = (): FieldValue<number> => {
    const raws = targets.map((t, i) => {
      const r = props(t).cornerRadius as Value<number> | Value<number>[] | undefined;
      if (!Array.isArray(r)) return r;
      return allSame(r) ? r[0] : { $mixed: i };
    });
    const out = readValue(raws as never, doc, mode, 0);
    if (out.kind === "value" && typeof out.value === "object" && out.value !== null)
      return { kind: "mixed" };
    return out as FieldValue<number>;
  };

  /* paints */
  const paintRow = (p: Paint, index: number): PaintRow => {
    const kind = paintKind(p);
    const solid = (p as unknown as Loose).type === "solid";
    const ref = solid ? (p as unknown as { color: Value<string> }).color : undefined;
    const bound = ref && isVarRef(ref) ? bindingOf(doc, ref.$var, mode) : null;
    return {
      index,
      kind,
      label: solid ? (bound?.name ? variableLabel(bound.name) : "Color") : paintLabel(p),
      swatch: paintSwatch(p, colour as never),
      text: solid && ref !== undefined ? colour(ref) : "",
      opacity: paintOpacity(p, colour as never),
      visible: paintVisible(p),
      bound,
    };
  };
  const paints = (list: (p: Loose) => Paint[] | undefined): PaintsModel => {
    const lists = targets.map((t) => list(props(t)) ?? []);
    if (!allSame(lists)) return { kind: "mixed" };
    return { kind: "list", rows: (lists[0] ?? []).map(paintRow) };
  };

  /* stroke */
  const strokes = targets.map((t) => (props(t).stroke as Stroke | null | undefined) ?? null);
  let stroke: StrokeModel;
  if (!allSame(strokes)) stroke = { kind: "mixed" };
  else if (!strokes[0]) stroke = { kind: "none" };
  else {
    const s = strokes[0];
    const w = s.width as
      | Value<number>
      | [Value<number>, Value<number>, Value<number>, Value<number>];
    const widths = (Array.isArray(w) ? w : [w, w, w, w]) as [
      Value<number>,
      Value<number>,
      Value<number>,
      Value<number>,
    ];
    stroke = {
      kind: "stroke",
      rows: s.paints.map(paintRow),
      summary: strokeSummary(s, number),
      uniform: allSame(widths),
      widths,
      align: s.align ?? DEFAULT_ALIGN,
      cap: s.cap ?? DEFAULT_CAP,
      join: s.join ?? DEFAULT_JOIN,
      dash: formatDashes(s.dash),
      dashes: s.dash ?? [],
    };
  }

  /* effects */
  const effectLists = targets.map((t) => (props(t).effects as Effect[] | undefined) ?? []);
  const effects: EffectsModel = allSame(effectLists)
    ? {
        kind: "list",
        rows: (effectLists[0] ?? []).map((e, index) => ({
          index,
          kind: e.type,
          label: effectLabel(e),
          summary: effectSummary(e, number),
          visible: effectVisible(e),
        })),
      }
    : { kind: "mixed" };

  const main = component?.kind === "main";
  const texts = targets.length > 0 && kinds.every((k) => k === "text");
  return {
    count: targets.length,
    kinds,
    readOnly: input.readOnly,
    sections: sectionsFor(kinds, { main, editing: input.editing ?? false }),
    notes: readValue(
      targets.map((t) => (props(t).agentNotes as string | null | undefined) ?? undefined),
      doc,
      mode,
      "",
    ),
    position: {
      x: axis("x"),
      y: axis("y"),
      rotation: {
        value: read((p) => p.rotation, 0),
        editable: !input.readOnly,
        ...(input.readOnly ? { reason: "This file is read-only" } : {}),
      },
    },
    size: { width: sizeAxis("width"), height: sizeAxis("height") },
    layout,
    appearance: {
      opacity: read((p) => p.opacity, 1),
      radius: hasRadius ? radius() : null,
      clip: allFrames ? read((p) => p.clip, false) : null,
      blend: read((p) => p.blendMode, "normal"),
    },
    fills: paints((p) => p.fills as Paint[] | undefined),
    stroke,
    effects,
    text: texts ? readText(items, doc, mode, input.text) : null,
    component,
  };
}
