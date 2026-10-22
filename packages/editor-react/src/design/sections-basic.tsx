import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import { copy } from "@vellum/config";
import {
  combine,
  type FieldValue,
  type SizeAxis,
  setAlign,
  setClip,
  setGap,
  setGridCount,
  setJustify,
  setLayout,
  setNotes,
  setOpacity,
  setPaddingAll,
  setPaddingLinked,
  setPaddingSide,
  setPosition,
  setRadius,
  setRotation,
  setSize,
  setSizeMode,
} from "@vellum/editor-core";
import type { Value } from "@vellum/types-ts";
import { type ReactNode, useEffect, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import type { DesignContext } from "./context";
import {
  IconToggle,
  Pair,
  PropertyRow,
  Section,
  Segmented,
  SelectField,
  TextButton,
  ValueField,
  VariableChip,
} from "./controls";
import { SPEC } from "./specs";

/** A numeric property that may be bound: the chip above the row when it is, and the field that detaches it. */
export function Bound(props: {
  ctx: DesignContext;
  field: FieldValue<number>;
  suffix?: string;
  set(value: Value<number>): void;
}) {
  const container = usePortalContainer();
  const { field } = props;
  if (field.kind !== "bound") {
    if (!props.ctx.host.resources || field.kind === "none") return null;
    return (
      <Menu.Root>
        <Menu.Trigger
          className="vc-chip vc-chip--button"
          aria-label="Bind variable"
          disabled={props.ctx.disabled || !props.ctx.numbers.length}
        >
          <Icon name="link" size={12} />
          Bind variable
        </Menu.Trigger>
        <Menu.Portal container={container}>
          <Menu.Positioner side="bottom" align="start" sideOffset={4}>
            <Menu.Popup className="vc-menu">
              {props.ctx.numbers.map((v) => (
                <Menu.Item
                  key={v.id}
                  className="vc-menu__item"
                  onClick={() => props.set({ $var: v.id })}
                >
                  <span className="vc-mono-value">{v.name}</span>
                  <span className="vc-menu__key">{String(v.value ?? "")}</span>
                </Menu.Item>
              ))}
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
    );
  }
  const resolved = field.variable.value;
  return (
    <VariableChip
      binding={field.variable}
      {...(props.suffix !== undefined ? { suffix: props.suffix } : {})}
      options={props.ctx.numbers}
      disabled={props.ctx.disabled}
      onBind={(id) => props.set({ $var: id })}
      onDetach={typeof resolved === "number" ? () => props.set(resolved) : undefined}
    />
  );
}

/** The write a field makes: a literal number, or a variable it was bound to. */
const literal = (v: number) => v;

/* ------------------------------------------------------------------------------------------- notes */

export function NotesSection({ ctx }: { ctx: DesignContext }) {
  const { notes } = ctx.model;
  const text = notes.kind === "value" ? notes.value : "";
  const [open, setOpen] = useState(text !== "");
  const [draft, setDraft] = useState<string | null>(null);
  // A different layer starts collapsed unless it has notes; typing into the field never collapses it.
  const id = ctx.session.state.selection.join(",");
  useEffect(() => {
    setOpen(text !== "");
    setDraft(null);
  }, [id]);
  const commit = (value: string) => {
    setDraft(null);
    if (value !== text) ctx.edit(setNotes(value));
  };
  return (
    <section className="vc-section" data-section="notes" aria-label="Notes for agents">
      <button
        type="button"
        className="vc-notes__toggle"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        Notes for agents <Icon name={open ? "chevron-down" : "chevron-right"} size={12} />
      </button>
      {open ? (
        <div className="vc-notes__body">
          <textarea
            className="vc-notes__text"
            aria-label="Notes for agents"
            placeholder="Instructions agents read before changing this frame"
            disabled={ctx.disabled}
            value={draft ?? (notes.kind === "mixed" ? "" : text)}
            onChange={(e) => setDraft(e.currentTarget.value)}
            onBlur={(e) => {
              if (draft !== null) commit(e.currentTarget.value);
            }}
          />
          <span className="vc-notes__hint">{copy.notesForAgentsLine}</span>
        </div>
      ) : null}
    </section>
  );
}

/* ---------------------------------------------------------------------------------------- position */

export function PositionSection({ ctx }: { ctx: DesignContext }) {
  const { x, y, rotation } = ctx.model.position;
  return (
    <Section id="position" title="Position" density={ctx.density}>
      <Pair>
        <ValueField
          label="X"
          axis="X"
          value={x.value}
          spec={SPEC.bare}
          disabled={!x.editable}
          {...(x.reason ? { reason: x.reason } : {})}
          onCommit={(v) => ctx.edit(setPosition("x", v))}
        />
        <ValueField
          label="Y"
          axis="Y"
          value={y.value}
          spec={SPEC.bare}
          disabled={!y.editable}
          {...(y.reason ? { reason: y.reason } : {})}
          onCommit={(v) => ctx.edit(setPosition("y", v))}
        />
      </Pair>
      <Bound ctx={ctx} field={rotation.value} set={(v) => ctx.edit(setRotation(v))} />
      <PropertyRow label="Rotation">
        <ValueField
          label="Rotation"
          value={rotation.value}
          spec={SPEC.degrees}
          disabled={!rotation.editable}
          {...(rotation.reason ? { reason: rotation.reason } : {})}
          onCommit={(v) => ctx.edit(setRotation(literal(v)))}
        />
      </PropertyRow>
    </Section>
  );
}

/* ----------------------------------------------------------------------------------------- layout */

const LAYOUTS = [
  { value: "none", label: "Free" },
  { value: "row", label: "Row" },
  { value: "column", label: "Column" },
  { value: "grid", label: "Grid" },
] as const;
const SIZINGS = [
  { value: "hug", label: "Hug" },
  { value: "fill", label: "Fill" },
  { value: "fixed", label: "Fixed" },
] as const;
const SIZING_LABEL = {
  hug: "Hug contents",
  fill: "Fill container",
  fixed: "Fixed",
} as const;

function SizeField(props: {
  ctx: DesignContext;
  axis: "width" | "height";
  letter: string;
  field: SizeAxis;
}) {
  const container = usePortalContainer();
  const { ctx, axis, field } = props;
  const value: FieldValue<number> = field.mixed
    ? { kind: "mixed" }
    : { kind: "value", value: Math.round(field.value * 100) / 100 };
  return (
    <span className="vc-sizefield">
      <ValueField
        label={axis === "width" ? "Width" : "Height"}
        axis={props.letter}
        value={value}
        spec={SPEC.bareLength}
        disabled={ctx.disabled}
        onCommit={(v) => ctx.edit(setSize(axis, v))}
      />
      <Menu.Root>
        <Menu.Trigger
          className="vc-iconbutton vc-sizefield__menu"
          aria-label={`${axis === "width" ? "Width" : "Height"} sizing`}
          disabled={ctx.disabled}
        >
          <Icon name="chevron-down" size={12} />
        </Menu.Trigger>
        <Menu.Portal container={container}>
          <Menu.Positioner side="bottom" align="end" sideOffset={4}>
            <Menu.Popup className="vc-menu">
              {ctx.host.resources ? (
                <Menu.Group>
                  <Menu.GroupLabel className="vc-menu__heading">Variables</Menu.GroupLabel>
                  {ctx.numbers.map((v) => (
                    <Menu.Item
                      key={v.id}
                      className="vc-menu__item"
                      onClick={() => ctx.edit(setSize(axis, { $var: v.id }))}
                    >
                      {v.name}
                    </Menu.Item>
                  ))}
                </Menu.Group>
              ) : null}
              {SIZINGS.map((s) => (
                <Menu.Item
                  key={s.value}
                  className="vc-menu__item"
                  onClick={() => ctx.edit(setSizeMode(axis, s.value))}
                >
                  {s.value === "fixed" && !field.mixed
                    ? `Fixed · ${Math.round(field.value)} px`
                    : SIZING_LABEL[s.value]}
                  {!field.mixed && field.mode === s.value ? (
                    <span className="vc-menu__key">
                      <Icon name="check" size={12} />
                    </span>
                  ) : null}
                </Menu.Item>
              ))}
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
    </span>
  );
}

const ALIGN = {
  column: ["align-start-vertical", "align-center-vertical", "align-end-vertical"],
  row: ["align-start-horizontal", "align-center-horizontal", "align-end-horizontal"],
} as const;
const ALIGN_VALUES = ["start", "center", "end"] as const;
const ALIGN_LABEL = ["Align start", "Align centre", "Align end"] as const;
const JUSTIFY = [
  { value: "start", label: "Start" },
  { value: "center", label: "Centre" },
  { value: "end", label: "End" },
  { value: "space-between", label: "Space between" },
  { value: "space-around", label: "Space around" },
] as const;

function Matrix(props: { ctx: DesignContext; direction: "row" | "column"; trigger: ReactNode }) {
  const container = usePortalContainer();
  const { ctx } = props;
  const layout = ctx.model.layout;
  if (!layout) return null;
  const align = layout.align.kind === "value" ? layout.align.value : null;
  const justify = layout.justify.kind === "value" ? layout.justify.value : null;
  // The 3 × 3 grid: the main axis runs along rows for a column layout.
  const cell = (main: number, cross: number) => {
    const m = ALIGN_VALUES[main] as (typeof ALIGN_VALUES)[number];
    const c = ALIGN_VALUES[cross] as (typeof ALIGN_VALUES)[number];
    return (
      <button
        key={`${main}${cross}`}
        type="button"
        className="vc-matrix__cell"
        aria-label={`${ALIGN_LABEL[cross]}, ${m} distribution`}
        aria-pressed={align === c && justify === m}
        disabled={ctx.disabled}
        onClick={() => ctx.edit(combine(setAlign(c), setJustify(m)))}
      />
    );
  };
  return (
    <Popover.Root>
      <Popover.Trigger
        className="vc-iconbutton"
        aria-label="More alignment"
        disabled={ctx.disabled}
      >
        {props.trigger}
      </Popover.Trigger>
      <Popover.Portal container={container}>
        <Popover.Positioner side="bottom" align="start" sideOffset={6}>
          <Popover.Popup className="vc-popover vc-matrix">
            <div className="vc-matrix__grid" data-direction={props.direction}>
              {[0, 1, 2].flatMap((row) =>
                [0, 1, 2].map((col) =>
                  props.direction === "column" ? cell(row, col) : cell(col, row),
                ),
              )}
            </div>
            <PropertyRow label="Distribute">
              <SelectField
                label="Distribution"
                value={justify as (typeof JUSTIFY)[number]["value"] | null}
                mixed={justify === null}
                options={JUSTIFY}
                disabled={ctx.disabled}
                onChange={(v) => ctx.edit(setJustify(v))}
              />
            </PropertyRow>
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

export function LayoutSection({ ctx }: { ctx: DesignContext }) {
  const { t: translateUi } = useLocale();
  const { layout, size } = ctx.model;
  const [unlinked, setUnlinked] = useState<boolean | null>(null);
  const id = ctx.session.state.selection.join(",");
  useEffect(() => setUnlinked(null), [id]);
  const sizing =
    size.width.mixed || size.height.mixed || size.width.mode !== size.height.mode
      ? null
      : size.width.mode;
  const mode = layout?.mode.kind === "value" ? layout.mode.value : null;
  const flow = mode === "row" || mode === "column";
  const padding = layout?.padding;
  const isLinked = unlinked === null ? (padding?.linked ?? true) : !unlinked;
  const SIDES = [
    ["Top", 0],
    ["End", 1],
    ["Bottom", 2],
    ["Start", 3],
  ] as const;
  return (
    <Section id="layout" title="Layout" density={ctx.density}>
      {layout ? (
        <>
          <Segmented
            label="Layout"
            value={mode as "none" | "row" | "column" | "grid" | null}
            mixed={layout.mode.kind === "mixed"}
            disabled={ctx.disabled}
            options={LAYOUTS}
            onChange={(v) => ctx.edit(setLayout(v))}
          />
          <Bound ctx={ctx} field={layout.gap} suffix=" px" set={(v) => ctx.edit(setGap(v))} />
          {mode !== "none" ? (
            <PropertyRow label="Gap">
              <ValueField
                label="Gap"
                value={layout.gap}
                spec={SPEC.px}
                disabled={ctx.disabled}
                onCommit={(v) => ctx.edit(setGap(literal(v)))}
              />
            </PropertyRow>
          ) : null}
          {layout.grid ? (
            <Pair>
              <ValueField
                label="Columns"
                axis="Columns"
                value={
                  layout.grid.columns.kind === "value" && layout.grid.columns.value !== null
                    ? { kind: "value", value: layout.grid.columns.value }
                    : { kind: "none" }
                }
                spec={SPEC.count}
                disabled={ctx.disabled}
                onCommit={(v) => ctx.edit(setGridCount("columns", v))}
              />
              <ValueField
                label="Rows"
                axis="Rows"
                value={
                  layout.grid.rows.kind === "value" && layout.grid.rows.value !== null
                    ? { kind: "value", value: layout.grid.rows.value }
                    : { kind: "none" }
                }
                spec={SPEC.count}
                disabled={ctx.disabled}
                onCommit={(v) => ctx.edit(setGridCount("rows", v))}
              />
            </Pair>
          ) : null}
          <Bound
            ctx={ctx}
            field={padding?.all ?? { kind: "none" }}
            suffix=" px"
            set={(v) => ctx.edit(setPaddingAll(v))}
          />
          {isLinked ? (
            <Pair>
              <ValueField
                label="Padding"
                axis="Padding"
                value={padding?.all ?? { kind: "none" }}
                spec={SPEC.bare}
                disabled={ctx.disabled}
                onCommit={(v) => ctx.edit(setPaddingAll(literal(v)))}
              />
              <span className="vc-fieldwrap">
                <button
                  type="button"
                  className="vc-field vc-linkfield"
                  data-height="28"
                  aria-pressed={true}
                  aria-label="Padding linked"
                  disabled={ctx.disabled}
                  onClick={() => {
                    setUnlinked(true);
                    ctx.edit(setPaddingLinked(false));
                  }}
                >
                  <span className="vc-field__axis">Linked</span>
                  <span className="vc-mono-value">{translateUi("On")}</span>
                </button>
              </span>
            </Pair>
          ) : (
            <>
              <PropertyRow label="Padding">
                <button
                  type="button"
                  className="vc-field vc-linkfield"
                  aria-pressed={false}
                  aria-label="Padding unlinked"
                  disabled={ctx.disabled}
                  onClick={() => {
                    setUnlinked(false);
                    ctx.edit(setPaddingLinked(true));
                  }}
                >
                  <span className="vc-mono-value">Unlinked</span>
                </button>
              </PropertyRow>
              {[0, 2].map((row) => (
                <Pair key={row}>
                  {SIDES.slice(row, row + 2).map(([name, side]) => (
                    <ValueField
                      key={name}
                      label={`Padding ${name.toLowerCase()}`}
                      axis={name}
                      value={padding?.sides[side] ?? { kind: "none" }}
                      spec={SPEC.bare}
                      disabled={ctx.disabled}
                      onCommit={(v) => ctx.edit(setPaddingSide(side, literal(v)))}
                    />
                  ))}
                </Pair>
              ))}
            </>
          )}
          {flow ? (
            <fieldset className="vc-aligns" aria-label="Alignment">
              {(ALIGN[mode as "row" | "column"] ?? ALIGN.column).map((icon, i) => (
                <button
                  key={icon}
                  type="button"
                  className="vc-iconbutton"
                  aria-label={ALIGN_LABEL[i]}
                  aria-pressed={
                    layout.align.kind === "value" && layout.align.value === ALIGN_VALUES[i]
                  }
                  disabled={ctx.disabled}
                  onClick={() => ctx.edit(setAlign(ALIGN_VALUES[i] as "start"))}
                >
                  <Icon name={icon} size={16} />
                </button>
              ))}
              <Matrix
                ctx={ctx}
                direction={mode as "row" | "column"}
                trigger={<Icon name="sliders-horizontal" size={16} />}
              />
            </fieldset>
          ) : null}
        </>
      ) : null}
      <Segmented
        label="Sizing"
        value={sizing}
        mixed={sizing === null}
        disabled={ctx.disabled}
        options={SIZINGS}
        onChange={(m) => ctx.edit(combine(setSizeMode("width", m), setSizeMode("height", m)))}
      />
      <Pair>
        <SizeField ctx={ctx} axis="width" letter="W" field={size.width} />
        <SizeField ctx={ctx} axis="height" letter="H" field={size.height} />
      </Pair>
    </Section>
  );
}

/* ------------------------------------------------------------------------------------ appearance */

export function AppearanceSection({ ctx }: { ctx: DesignContext }) {
  const { t: translateUi } = useLocale();
  const { opacity, radius, clip } = ctx.model.appearance;
  return (
    <Section
      id="appearance"
      title={translateUi("Appearance")}
      density={ctx.density}
      actions={
        clip ? (
          <IconToggle
            icon="scissors"
            label="Clip content"
            pressed={clip.kind === "value" && clip.value === true}
            disabled={ctx.disabled}
            onChange={(on) => ctx.edit(setClip(on))}
          />
        ) : null
      }
    >
      <Bound ctx={ctx} field={opacity} set={(v) => ctx.edit(setOpacity(v))} />
      <Pair>
        <ValueField
          label="Opacity"
          axis="Opacity"
          value={opacity}
          spec={SPEC.opacity}
          disabled={ctx.disabled}
          onCommit={(v) => ctx.edit(setOpacity(literal(v)))}
        />
        {radius ? (
          <ValueField
            label="Corner radius"
            axis="Radius"
            value={radius}
            spec={SPEC.px}
            disabled={ctx.disabled}
            onCommit={(v) => ctx.edit(setRadius(literal(v)))}
          />
        ) : (
          <span />
        )}
      </Pair>
      {radius ? (
        <Bound ctx={ctx} field={radius} suffix=" px" set={(v) => ctx.edit(setRadius(v))} />
      ) : null}
    </Section>
  );
}

/* --------------------------------------------------------------------------------------- export */

export function ExportSection({ ctx }: { ctx: DesignContext }) {
  return (
    <Section id="export" title="Export" density={ctx.density}>
      <TextButton
        disabled={ctx.disabled || !ctx.host.onAction}
        title={ctx.host.onAction ? undefined : "Export is not available here"}
        onClick={() => ctx.host.onAction?.("export.add")}
      >
        Add export
      </TextButton>
    </Section>
  );
}
