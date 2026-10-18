import {
  alignPatch,
  contentPatch,
  type Edit,
  type FieldValue,
  fontPatch,
  letterSpacingPatch,
  lineHeightPatch,
  plainText,
  resizePatch,
  type SceneItem,
  sizePatch,
  stylePatch,
  weightPatch,
} from "@vellum/editor-core";
import type { NodePatch, TextRun } from "@vellum/types-ts";
import { useEffect, useState } from "react";
import type { DesignContext } from "./context";
import { Banner, Pair, PropertyRow, Section, Segmented, SelectField, ValueField } from "./controls";
import { SPEC } from "./specs";

/** An edit that is about the text of each selected layer, not the layer's place. */
const onText =
  (f: (item: SceneItem) => NodePatch | null): Edit =>
  (target) =>
    f(target.item);

/**
 * Changes the text. While the layer is open for typing, a change to its characters or their style goes to the
 * draft, so typing and restyling are one change when the text is finished; the layer's own properties (alignment,
 * resizing, the style it remembers) are written at once.
 */
function change(ctx: DesignContext, f: (item: SceneItem) => NodePatch | null) {
  const mode = ctx.session.state.mode;
  if (mode.type !== "editing" || mode.node.type !== "text") return ctx.edit(onText(f));
  const draft = {
    node: mode.node,
    bounds: { x: 0, y: 0, width: 0, height: 0 },
    children: [],
    parent: null,
  };
  const patch = f(draft as SceneItem);
  if (!patch) return;
  const { content, ...layer } = patch;
  if (content) ctx.session.dispatch({ type: "content", content: content as TextRun[] });
  if (Object.keys(layer).length) ctx.edit(() => layer);
}

const NO_STYLE = "__none";
const ALIGNS = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "right", label: "Right" },
] as const;
const RESIZES = [
  { value: "auto-width", label: "Auto width" },
  { value: "auto-height", label: "Auto height" },
  { value: "fixed", label: "Fixed" },
] as const;

const number = (f: FieldValue<number | "auto">): FieldValue<number> =>
  f.kind === "value" && typeof f.value === "number"
    ? { kind: "value", value: f.value }
    : f.kind === "mixed"
      ? { kind: "mixed" }
      : f.kind === "bound"
        ? ({ kind: "bound", variable: f.variable } as FieldValue<number>)
        : { kind: "none" };

export function TextSection({ ctx }: { ctx: DesignContext }) {
  const t = ctx.model.text;
  if (!t) return null;
  const families = new Set<string>(ctx.host.fonts ?? []);
  for (const face of Object.values(
    (ctx.document?.fonts ?? {}) as Record<string, { family: string }>,
  ))
    families.add(face.family);
  const font = t.font.kind === "value" ? t.font.value : null;
  if (font) families.add(font);
  const style = t.style.kind === "value" ? (t.style.value ?? NO_STYLE) : null;
  return (
    <Section id="text" title="Text" density={ctx.density}>
      <PropertyRow label="Style">
        <SelectField
          label="Text style"
          value={style}
          mixed={t.style.kind === "mixed"}
          options={[
            { value: NO_STYLE, label: "None" },
            ...t.styles.map((s) => ({ value: s.id, label: s.name })),
          ]}
          disabled={ctx.disabled}
          onChange={(id) =>
            change(ctx, (item) => stylePatch(item, id === NO_STYLE ? null : id, ctx.document))
          }
        />
      </PropertyRow>
      {t.missing.length ? (
        <Banner icon="info">Font missing · using {font ?? "the fallback"}</Banner>
      ) : null}
      <PropertyRow label="Font">
        {ctx.host.resources ? (
          <button
            type="button"
            className="vc-field"
            aria-label="Font"
            disabled={ctx.disabled}
            onClick={() => ctx.host.resources?.open("fonts")}
          >
            {font ?? "Mixed"}
          </button>
        ) : (
          <SelectField
            label="Font"
            value={font}
            mixed={t.font.kind === "mixed"}
            options={[...families].map((f) => ({ value: f, label: f }))}
            disabled={ctx.disabled}
            onChange={(f) => change(ctx, (item) => fontPatch(item, f))}
          />
        )}
      </PropertyRow>
      <Pair>
        <ValueField
          label="Weight"
          axis="Weight"
          value={t.weight as FieldValue<number>}
          spec={SPEC.weight}
          disabled={ctx.disabled}
          onCommit={(v) => change(ctx, (item) => weightPatch(item, v))}
        />
        <ValueField
          label="Size"
          axis="Size"
          value={t.size}
          spec={SPEC.size}
          disabled={ctx.disabled}
          onCommit={(v) => change(ctx, (item) => sizePatch(item, v))}
        />
      </Pair>
      <Pair>
        <ValueField
          label="Line height"
          axis="Line"
          value={number(t.lineHeight)}
          spec={SPEC.lineHeight}
          placeholder="Auto"
          disabled={ctx.disabled}
          keyword={{
            text: "auto",
            commit: () => change(ctx, (item) => lineHeightPatch(item, null)),
          }}
          onCommit={(v) => change(ctx, (item) => lineHeightPatch(item, v))}
        />
        <ValueField
          label="Letter spacing"
          axis="Letter"
          value={t.letterSpacing}
          spec={SPEC.letter}
          disabled={ctx.disabled}
          onCommit={(v) => change(ctx, (item) => letterSpacingPatch(item, v))}
        />
      </Pair>
      <Segmented
        label="Text alignment"
        value={t.align.kind === "value" ? (t.align.value as "left" | "center" | "right") : null}
        mixed={t.align.kind === "mixed"}
        disabled={ctx.disabled}
        options={ALIGNS}
        onChange={(a) => change(ctx, (item) => alignPatch(item, a))}
      />
      <Segmented
        label="Text resizing"
        value={
          t.resize.kind === "value"
            ? (t.resize.value as "auto-width" | "auto-height" | "fixed")
            : null
        }
        mixed={t.resize.kind === "mixed"}
        disabled={ctx.disabled}
        options={RESIZES}
        onChange={(r) => change(ctx, (item) => resizePatch(item, r))}
      />
    </Section>
  );
}

/** The characters of the text layer. While it is open for typing, typing here is typing on the canvas. */
export function ContentSection({ ctx }: { ctx: DesignContext }) {
  const t = ctx.model.text;
  const mode = ctx.session.state.mode;
  const editing = mode.type === "editing" && mode.node.type === "text" ? mode : null;
  const text = editing ? plainText(editing.node) : (t?.content ?? "");
  const [draft, setDraft] = useState<string | null>(null);
  const id = ctx.session.state.selection.join(",");
  useEffect(() => setDraft(null), [id]);
  if (!t) return null;
  const push = (value: string) => {
    if (!editing) return;
    const runs = (editing.node as unknown as { content: TextRun[] }).content;
    const first = runs[0];
    if (first) ctx.session.dispatch({ type: "content", content: [{ ...first, text: value }] });
  };
  return (
    <Section id="content" title="Content" density={ctx.density}>
      <textarea
        className="vc-notes__text vc-content__text"
        rows={1}
        aria-label="Text content"
        disabled={ctx.disabled || (t.content === null && !editing)}
        placeholder={t.content === null && !editing ? "Mixed" : undefined}
        value={draft ?? text}
        onChange={(e) => {
          setDraft(e.currentTarget.value);
          push(e.currentTarget.value);
        }}
        onBlur={(e) => {
          if (draft === null) return;
          if (!editing) change(ctx, (item) => contentPatch(item, e.currentTarget.value));
          setDraft(null);
        }}
      />
    </Section>
  );
}
