import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import {
  addEffectTo,
  addFill,
  addStroke,
  BLEND_MODES,
  blendLabel,
  convertPaint,
  EFFECT_KINDS,
  type EffectKind,
  type EffectRow,
  effectLabelOf,
  type FieldValue,
  formatDashes,
  kindLabel,
  newEffect,
  PAINT_KINDS,
  type PaintKind,
  type PaintRow,
  parseDashes,
  removeEffectAt,
  removeFill,
  removeStroke,
  SIDE_NAMES,
  setBlend,
  setBlurRadius,
  setEffectColour,
  setEffectVisible,
  setPaintOpacity,
  setPaintVisible,
  setShadowField,
  setStrokeAlign,
  setStrokeCap,
  setStrokeDash,
  setStrokeJoin,
  setStrokeSide,
  setStrokeWidth,
  updateEffectAt,
  updateStroke,
} from "@vellum/editor-core";
import type { Effect, Paint, Value } from "@vellum/types-ts";
import { type ReactNode, useState } from "react";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import type { DesignContext } from "./context";
import {
  Pair,
  PropertyRow,
  Section,
  SelectField,
  TextButton,
  TextField,
  ValueField,
} from "./controls";
import { changePaint, HexInline, PaintPopover, type PaintScope } from "./PaintPopover";
import { SPEC } from "./specs";

const swatchStyle = (css: string | null) => (css ? { background: css } : undefined);
const solidPaint = (colour: string) => ({ type: "solid", color: colour }) as unknown as Paint;

/* ------------------------------------------------------------------------------------------ paints */

const KIND_OPTIONS = (current: PaintKind) =>
  PAINT_KINDS.filter((k) => (k !== "image" && k !== "shader") || k === current).map((k) => ({
    value: k,
    label: kindLabel(k),
  }));

function PaintRowView(props: {
  ctx: DesignContext;
  scope: PaintScope;
  row: PaintRow;
  /** How many paints the list has: a stroke keeps its last one. */
  count: number;
  remove(): void;
}) {
  const { ctx, scope, row } = props;
  const resolve = (c: Value<string>) => ctx.colour(c);
  const change = (fn: (p: Paint) => Paint) => changePaint(ctx, scope, row.index, fn);
  const actions = (
    <span className="vc-paintrow__actions">
      <button
        type="button"
        className="vc-iconbutton"
        aria-label={row.visible ? "Hide paint" : "Show paint"}
        aria-pressed={!row.visible}
        disabled={ctx.disabled}
        onClick={() => change((p) => setPaintVisible(p, !row.visible))}
      >
        <Icon name={row.visible ? "eye" : "eye-off"} size={16} />
      </button>
      <button
        type="button"
        className="vc-iconbutton"
        aria-label="Remove paint"
        disabled={ctx.disabled || props.count <= (scope === "stroke" ? 1 : 0)}
        onClick={props.remove}
      >
        <Icon name="minus" size={16} />
      </button>
    </span>
  );
  if (row.kind === "solid")
    return (
      <PropertyRow label={row.label}>
        <span
          className="vc-field vc-paintfield"
          data-state={ctx.disabled ? "disabled" : "default"}
          data-hidden={row.visible ? undefined : ""}
        >
          <HexInline
            ctx={ctx}
            value={row.text}
            label={`${row.label} hex`}
            disabled={ctx.disabled}
            onColour={(hex) =>
              change((p) => ({ ...(p as object), color: hex }) as unknown as Paint)
            }
          />
          {actions}
          <PaintPopover
            ctx={ctx}
            scope={scope}
            index={row.index}
            triggerClassName="vc-swatch"
            triggerLabel={`Edit ${row.label.toLowerCase()}`}
            trigger={
              <span
                className="vc-swatch__fill"
                style={swatchStyle(row.swatch)}
                data-bound={row.bound ? "" : undefined}
              />
            }
          />
        </span>
      </PropertyRow>
    );
  return (
    <div className="vc-paintgroup" data-hidden={row.visible ? undefined : ""}>
      <PropertyRow label="Paint">
        <SelectField
          label="Paint type"
          value={row.kind}
          options={KIND_OPTIONS(row.kind)}
          disabled={ctx.disabled}
          onChange={(k) => change((p) => convertPaint(p, k, resolve) ?? p)}
        />
      </PropertyRow>
      <PropertyRow label="Opacity">
        <ValueField
          label="Paint opacity"
          value={{ kind: "value", value: row.opacity }}
          spec={SPEC.opacity}
          disabled={ctx.disabled}
          onCommit={(v) => change((p) => setPaintOpacity(p, v, resolve))}
        />
      </PropertyRow>
      <PropertyRow label="Blend">
        <BlendField ctx={ctx} />
      </PropertyRow>
      <div className="vc-paint__row">
        <PaintPopover
          ctx={ctx}
          scope={scope}
          index={row.index}
          triggerClassName="vc-button"
          triggerLabel="Edit fill"
          trigger="Edit fill"
        />
        {actions}
      </div>
    </div>
  );
}

/** The layer's blend mode: the format has one, so every paint's Blend row edits it. */
function BlendField({ ctx, opacity }: { ctx: DesignContext; opacity?: number }) {
  const blend = ctx.model.appearance.blend;
  return (
    <SelectField
      {...(opacity !== undefined && blend.kind === "value"
        ? { display: `${blendLabel(blend.value)} · ${Math.round(opacity * 100)}%` }
        : {})}
      label="Blend mode"
      value={blend.kind === "value" ? blend.value : null}
      mixed={blend.kind !== "value"}
      options={BLEND_MODES.map((m) => ({ value: m, label: blendLabel(m) }))}
      disabled={ctx.disabled}
      onChange={(m) => ctx.edit(setBlend(m))}
    />
  );
}

function AddButton(props: { label: string; disabled?: boolean; onClick?(): void }) {
  return (
    <button
      type="button"
      className="vc-iconbutton"
      aria-label={props.label}
      title={props.label}
      disabled={props.disabled}
      onClick={props.onClick}
    >
      <Icon name="plus" size={16} />
    </button>
  );
}

export function FillSection({ ctx }: { ctx: DesignContext }) {
  const fills = ctx.model.fills;
  return (
    <Section
      id="fill"
      title="Fill"
      density={
        fills.kind === "list" && fills.rows.some((r) => r.kind !== "solid") ? "wide" : ctx.density
      }
      actions={
        <AddButton
          label="Add fill"
          disabled={ctx.disabled}
          onClick={() => ctx.edit(addFill(solidPaint(ctx.design.defaults.fill)))}
        />
      }
    >
      {fills.kind === "mixed" ? (
        <span className="vc-section__empty">Mixed</span>
      ) : fills.rows.length === 0 ? (
        <span className="vc-section__empty">No fill</span>
      ) : (
        fills.rows.map((row) => (
          <PaintRowView
            key={row.index}
            ctx={ctx}
            scope="fill"
            row={row}
            count={fills.rows.length}
            remove={() => ctx.edit(removeFill(row.index))}
          />
        ))
      )}
    </Section>
  );
}

/* ------------------------------------------------------------------------------------------ stroke */

const ALIGNS = [
  { value: "inside", label: "Inside" },
  { value: "center", label: "Center" },
  { value: "outside", label: "Outside" },
] as const;
const CAPS = [
  { value: "butt", label: "Butt" },
  { value: "round", label: "Round" },
  { value: "square", label: "Square" },
] as const;
const JOINS = [
  { value: "miter", label: "Miter" },
  { value: "round", label: "Round" },
  { value: "bevel", label: "Bevel" },
] as const;

function SidesPopover(props: {
  ctx: DesignContext;
  summary: string;
  widths: readonly Value<number>[];
  uniform: boolean;
}) {
  const container = usePortalContainer();
  const { ctx } = props;
  const number = (v: Value<number>): FieldValue<number> =>
    typeof v === "number" ? { kind: "value", value: v } : { kind: "none" };
  return (
    <Popover.Root>
      <Popover.Trigger
        className="vc-field vc-sidesfield"
        data-height="28"
        aria-label="Stroke sides"
      >
        <span className="vc-mono-value">{props.summary}</span>
      </Popover.Trigger>
      <Popover.Portal container={container}>
        <Popover.Positioner side="left" align="start" sideOffset={12}>
          <Popover.Popup className="vc-popover vc-sides" aria-label="Stroke widths">
            <PropertyRow label="All sides">
              <ValueField
                label="Stroke width"
                value={props.uniform ? number(props.widths[0] as Value<number>) : { kind: "mixed" }}
                spec={SPEC.length}
                disabled={ctx.disabled}
                onCommit={(v) => ctx.edit(updateStroke((s) => setStrokeWidth(s, v)))}
              />
            </PropertyRow>
            {[0, 2].map((row) => (
              <Pair key={row}>
                {SIDE_NAMES.slice(row, row + 2).map((name, i) => (
                  <ValueField
                    key={name}
                    label={`${name} width`}
                    axis={name}
                    value={number(props.widths[row + i] as Value<number>)}
                    spec={SPEC.bareLength}
                    disabled={ctx.disabled}
                    onCommit={(v) =>
                      ctx.edit(updateStroke((s) => setStrokeSide(s, (row + i) as 0 | 1 | 2 | 3, v)))
                    }
                  />
                ))}
              </Pair>
            ))}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

export function StrokeSection({ ctx }: { ctx: DesignContext }) {
  const stroke = ctx.model.stroke;
  return (
    <Section
      id="stroke"
      title="Stroke"
      density={stroke.kind === "stroke" ? "wide" : ctx.density}
      aside={
        stroke.kind === "stroke"
          ? stroke.rows.map((row) => (
              <PaintPopover
                key={row.index}
                ctx={ctx}
                scope="stroke"
                index={row.index}
                triggerClassName="vc-swatch"
                triggerLabel={`Edit stroke ${row.label.toLowerCase()}`}
                trigger={<span className="vc-swatch__fill" style={swatchStyle(row.swatch)} />}
              />
            ))
          : null
      }
      actions={
        stroke.kind === "stroke" ? (
          <button
            type="button"
            className="vc-iconbutton"
            aria-label="Remove stroke"
            title="Remove stroke"
            disabled={ctx.disabled}
            onClick={() => ctx.edit(removeStroke)}
          >
            <Icon name="minus" size={16} />
          </button>
        ) : (
          <AddButton
            label="Add stroke"
            disabled={ctx.disabled || stroke.kind === "mixed"}
            onClick={() => ctx.edit(addStroke(ctx.design.defaults.stroke))}
          />
        )
      }
    >
      {stroke.kind === "none" ? (
        <span className="vc-section__empty">No stroke</span>
      ) : stroke.kind === "mixed" ? (
        <span className="vc-section__empty">Mixed</span>
      ) : (
        <>
          <PropertyRow label="Sides">
            <SidesPopover
              ctx={ctx}
              summary={stroke.summary}
              widths={stroke.widths}
              uniform={stroke.uniform}
            />
          </PropertyRow>
          <PropertyRow label="Align">
            <SelectField
              label="Stroke alignment"
              value={stroke.align as "inside" | "center" | "outside"}
              options={ALIGNS}
              disabled={ctx.disabled}
              onChange={(v) => ctx.edit(updateStroke((s) => setStrokeAlign(s, v)))}
            />
          </PropertyRow>
          <PropertyRow label="Caps">
            <SelectField
              label="Stroke caps"
              value={stroke.cap as "butt" | "round" | "square"}
              options={CAPS}
              disabled={ctx.disabled}
              onChange={(v) => ctx.edit(updateStroke((s) => setStrokeCap(s, v)))}
            />
          </PropertyRow>
          <PropertyRow label="Joins">
            <SelectField
              label="Stroke joins"
              value={stroke.join as "miter" | "round" | "bevel"}
              options={JOINS}
              disabled={ctx.disabled}
              onChange={(v) => ctx.edit(updateStroke((s) => setStrokeJoin(s, v)))}
            />
          </PropertyRow>
          <PropertyRow label="Dashes">
            <TextField
              label="Dashes"
              value={stroke.dashes.length ? formatDashes(stroke.dashes) : ""}
              placeholder="None"
              disabled={ctx.disabled}
              height={26}
              onCommit={(text) => {
                const parsed = parseDashes(text);
                if (!parsed.ok) return parsed.message;
                ctx.edit(updateStroke((s) => setStrokeDash(s, parsed.dash)));
                return null;
              }}
            />
          </PropertyRow>
          <PropertyRow label="Blend">
            <BlendField ctx={ctx} opacity={stroke.rows[0]?.opacity ?? 1} />
          </PropertyRow>
        </>
      )}
    </Section>
  );
}

/* ----------------------------------------------------------------------------------------- effects */

function effectAt(ctx: DesignContext, index: number): Effect | null {
  const node = ctx.design.targets()[0]?.item.node as unknown as { effects?: Effect[] } | undefined;
  return node?.effects?.[index] ?? null;
}
const change = (ctx: DesignContext, index: number, fn: (e: Effect) => Effect) =>
  ctx.edit(updateEffectAt(index, fn));

function EffectPopover(props: { ctx: DesignContext; row: EffectRow; children: ReactNode }) {
  const container = usePortalContainer();
  const { ctx, row } = props;
  const [open, setOpen] = useState(false);
  const effect = open ? effectAt(ctx, row.index) : null;
  const raw = effect as unknown as Record<string, Value<number> | string | undefined> | null;
  const num = (v: unknown): FieldValue<number> =>
    typeof v === "number" ? { kind: "value", value: v } : { kind: "none" };
  const resolveColour = (c: Value<string>) => ctx.colour(c);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger
        className="vc-field vc-effectfield"
        data-height="28"
        aria-label={`Edit ${row.label.toLowerCase()}`}
      >
        {props.children}
      </Popover.Trigger>
      <Popover.Portal container={container}>
        <Popover.Positioner side="left" align="start" sideOffset={12}>
          <Popover.Popup className="vc-popover vc-effect" aria-label={`${row.label} settings`}>
            {effect && raw ? (
              <>
                <h4 className="vc-paint__title">{row.label}</h4>
                {row.kind === "drop-shadow" || row.kind === "inner-shadow" ? (
                  <>
                    <Pair>
                      <ValueField
                        label="Shadow X"
                        axis="X"
                        value={num(raw.x)}
                        spec={SPEC.bare}
                        disabled={ctx.disabled}
                        onCommit={(v) => change(ctx, row.index, (e) => setShadowField(e, "x", v))}
                      />
                      <ValueField
                        label="Shadow Y"
                        axis="Y"
                        value={num(raw.y)}
                        spec={SPEC.bare}
                        disabled={ctx.disabled}
                        onCommit={(v) => change(ctx, row.index, (e) => setShadowField(e, "y", v))}
                      />
                    </Pair>
                    <Pair>
                      <ValueField
                        label="Shadow blur"
                        axis="Blur"
                        value={num(raw.blur)}
                        spec={SPEC.bareLength}
                        disabled={ctx.disabled}
                        onCommit={(v) =>
                          change(ctx, row.index, (e) => setShadowField(e, "blur", v))
                        }
                      />
                      <ValueField
                        label="Shadow spread"
                        axis="Spread"
                        value={num(raw.spread ?? 0)}
                        spec={SPEC.bare}
                        disabled={ctx.disabled}
                        onCommit={(v) =>
                          change(ctx, row.index, (e) => setShadowField(e, "spread", v))
                        }
                      />
                    </Pair>
                    <PropertyRow label="Color">
                      <HexInlineField
                        ctx={ctx}
                        value={resolveColour(raw.color as Value<string>)}
                        disabled={ctx.disabled}
                        onColour={(hex) => change(ctx, row.index, (e) => setEffectColour(e, hex))}
                      />
                    </PropertyRow>
                  </>
                ) : row.kind === "shader" ? (
                  <p className="vc-paint__hint">Shader effects arrive in v1.1.</p>
                ) : (
                  <PropertyRow label="Radius">
                    <ValueField
                      label="Blur radius"
                      value={num(raw.radius)}
                      spec={SPEC.length}
                      disabled={ctx.disabled}
                      onCommit={(v) => change(ctx, row.index, (e) => setBlurRadius(e, v))}
                    />
                  </PropertyRow>
                )}
                <div className="vc-paint__row">
                  <TextButton
                    disabled={ctx.disabled}
                    onClick={() => change(ctx, row.index, (e) => setEffectVisible(e, !row.visible))}
                  >
                    {row.visible ? "Hide" : "Show"}
                  </TextButton>
                  <TextButton
                    disabled={ctx.disabled}
                    onClick={() => ctx.edit(removeEffectAt(row.index))}
                  >
                    Remove
                  </TextButton>
                </div>
              </>
            ) : null}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** A hex field in a popover row. */
function HexInlineField(props: {
  ctx: DesignContext;
  value: string;
  disabled?: boolean;
  onColour(hex: string): void;
}) {
  return (
    <span className="vc-field" data-state={props.disabled ? "disabled" : "default"}>
      <HexInline
        ctx={props.ctx}
        value={props.value}
        label="Shadow colour"
        disabled={props.disabled}
        onColour={props.onColour}
      />
    </span>
  );
}

function AddEffectMenu(props: {
  ctx: DesignContext;
  children: ReactNode;
  className: string;
  label: string;
}) {
  const container = usePortalContainer();
  const { ctx } = props;
  return (
    <Menu.Root>
      <Menu.Trigger className={props.className} aria-label={props.label} disabled={ctx.disabled}>
        {props.children}
      </Menu.Trigger>
      <Menu.Portal container={container}>
        <Menu.Positioner side="bottom" align="start" sideOffset={4}>
          <Menu.Popup className="vc-menu">
            {EFFECT_KINDS.map((k: EffectKind) => {
              const made = newEffect(k, { shadow: ctx.design.defaults.shadow });
              return (
                <Menu.Item
                  key={k}
                  className="vc-menu__item"
                  disabled={!made}
                  onClick={() => made && ctx.edit(addEffectTo(made))}
                >
                  {effectLabelOf(k)}
                  {!made ? <span className="vc-menu__key">v1.1</span> : null}
                </Menu.Item>
              );
            })}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

export function EffectsSection({ ctx }: { ctx: DesignContext }) {
  const effects = ctx.model.effects;
  return (
    <Section
      id="effects"
      title="Effects"
      density={effects.kind === "list" && effects.rows.length ? "wide" : ctx.density}
      actions={
        <AddEffectMenu ctx={ctx} className="vc-iconbutton" label="Add effect">
          <Icon name="plus" size={16} />
        </AddEffectMenu>
      }
    >
      {effects.kind === "mixed" ? (
        <span className="vc-section__empty">Mixed</span>
      ) : effects.rows.length === 0 ? (
        <span className="vc-section__empty">No effects</span>
      ) : (
        <>
          {effects.rows.map((row) => (
            <PropertyRow key={row.index} label={row.label}>
              <EffectPopover ctx={ctx} row={row}>
                <span className="vc-mono-value">{row.summary}</span>
              </EffectPopover>
              <button
                type="button"
                className="vc-iconbutton vc-effectvis"
                aria-label={row.visible ? "Hide effect" : "Show effect"}
                aria-pressed={!row.visible}
                disabled={ctx.disabled || row.kind === "shader"}
                onClick={() => change(ctx, row.index, (e) => setEffectVisible(e, !row.visible))}
              >
                <Icon name={row.visible ? "eye" : "eye-off"} size={16} />
              </button>
            </PropertyRow>
          ))}
          <AddEffectMenu ctx={ctx} className="vc-button" label="Add effect">
            Add effect
          </AddEffectMenu>
        </>
      )}
    </Section>
  );
}
