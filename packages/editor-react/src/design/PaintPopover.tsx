import { Popover } from "@base-ui/react/popover";
import {
  addStop,
  alphaOf,
  convertPaint,
  gradientAngle,
  imageFits,
  MESH_MAX,
  MESH_MIN,
  meshPointLabel,
  moveStop,
  normalizeHex,
  type PaintKind,
  paintKind,
  paintLabel,
  paintSwatch,
  removeStop,
  renderMesh,
  resizeMesh,
  setAlpha,
  setGradientAngle,
  setGradientKind,
  setImageFit,
  setMeshColour,
  setPaintOpacity,
  setStopColour,
  updateFill,
  updateStrokePaint,
} from "@vellum/editor-core";
import type { GradientKind, Paint, Value } from "@vellum/types-ts";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import type { DesignContext } from "./context";
import { Segmented, SelectField, TextButton, TextField, ValueField } from "./controls";
import { ShaderControls } from "./ShaderControls";
import { SPEC } from "./specs";

export type PaintScope = "fill" | "stroke";
type Loose = Record<string, unknown>;

/** The paint at `index` of the first selected layer: what the editor is open on. */
export function paintAt(ctx: DesignContext, scope: PaintScope, index: number): Paint | null {
  const node = ctx.design.targets()[0]?.item.node as unknown as Loose | undefined;
  if (!node) return null;
  const list =
    scope === "fill"
      ? (node.fills as Paint[] | undefined)
      : (node.stroke as { paints?: Paint[] } | null | undefined)?.paints;
  return list?.[index] ?? null;
}
export const changePaint = (
  ctx: DesignContext,
  scope: PaintScope,
  index: number,
  fn: (p: Paint) => Paint,
) => ctx.edit(scope === "fill" ? updateFill(index, fn) : updateStrokePaint(index, fn));

/* ---------------------------------------------------------------------------------------- colour */

export function HexField(props: {
  ctx: DesignContext;
  value: string;
  label?: string;
  disabled?: boolean;
  /** The colour written: a 6-digit entry keeps the current opacity. */
  onColour(hex: string): void;
  height?: 26 | 28;
}) {
  return (
    <TextField
      label={props.label ?? "Hex"}
      value={props.value}
      disabled={props.disabled}
      {...(props.height ? { height: props.height } : {})}
      onCommit={(raw) => {
        const hex = normalizeHex(raw);
        if (!hex) return "Enter a hex colour with 6 or 8 digits";
        const typedAlpha = raw.replace(/^#/, "").length === 8 || raw.replace(/^#/, "").length === 4;
        props.onColour(typedAlpha ? hex : setAlpha(hex, alphaOf(props.value)));
        return null;
      }}
    />
  );
}

function SolidBody(props: {
  ctx: DesignContext;
  paint: Paint;
  apply(fn: (p: Paint) => Paint): void;
}) {
  const { ctx, paint, apply } = props;
  const [tab, setTab] = useState<"color" | "variables">("color");
  const colour = (paint as unknown as { color: Value<string> }).color;
  const resolved = ctx.colour(colour);
  const bound = typeof colour === "object";
  const set = (value: Value<string>) =>
    apply((p) => ({ ...(p as object), color: value }) as unknown as Paint);
  return (
    <>
      <Segmented<"color" | "variables">
        label="Colour source"
        value={tab}
        disabled={ctx.disabled}
        options={[
          { value: "color", label: "Color" },
          { value: "variables", label: "Variables" },
        ]}
        onChange={setTab}
      />
      {tab === "variables" ? (
        <div className="vc-paint__chips">
          {ctx.colours.length ? (
            ctx.colours.map((v) => (
              <button
                key={v.id}
                type="button"
                className="vc-chip vc-chip--button"
                disabled={ctx.disabled}
                aria-pressed={bound && (colour as { $var: string }).$var === v.id}
                title={String(v.value ?? "")}
                onClick={() => set({ $var: v.id })}
              >
                <span className="vc-mono-value">{v.name}</span>
              </button>
            ))
          ) : (
            <span className="vc-section__empty">This file has no colour variables yet.</span>
          )}
        </div>
      ) : null}
      <div className="vc-prop">
        <span className="vc-prop__label">Hex</span>
        <span className="vc-prop__control">
          <HexField ctx={ctx} value={resolved} disabled={ctx.disabled} onColour={set} />
        </span>
      </div>
      <div className="vc-prop">
        <span className="vc-prop__label">Opacity</span>
        <span className="vc-prop__control">
          <ValueField
            label="Opacity"
            value={{ kind: "value", value: alphaOf(resolved) }}
            spec={SPEC.opacity}
            disabled={ctx.disabled || bound}
            {...(bound ? { reason: "Opacity comes from the variable" } : {})}
            onCommit={(v) => apply((p) => setPaintOpacity(p, v, (c) => ctx.colour(c)))}
          />
        </span>
      </div>
    </>
  );
}

/* -------------------------------------------------------------------------------------- gradients */

const GRADIENT_TYPES = [
  { value: "linear", label: "Linear" },
  { value: "radial", label: "Radial" },
  { value: "angular", label: "Angular" },
  { value: "mesh", label: "Mesh" },
] as const;

const MESH_W = 328;
const MESH_H = 168;

function MeshPreview(props: {
  ctx: DesignContext;
  paint: Paint;
  selected: number | null;
  onSelect(index: number): void;
}) {
  const { ctx, paint } = props;
  const canvas = useRef<HTMLCanvasElement>(null);
  const dpr = typeof devicePixelRatio === "number" ? devicePixelRatio : 1;
  const points = ((paint as unknown as Loose).points ?? []) as {
    x: number;
    y: number;
    color: Value<string>;
  }[];
  useEffect(() => {
    const el = canvas.current;
    const g = el?.getContext("2d");
    if (!el || !g) return;
    const w = Math.round(MESH_W * dpr);
    const h = Math.round(MESH_H * dpr);
    const pixels = renderMesh(paint, w, h, (c) => ctx.colour(c));
    g.putImageData(new ImageData(pixels as Uint8ClampedArray<ArrayBuffer>, w, h), 0, 0);
  }, [paint, dpr]);
  return (
    <div className="vc-mesh" data-testid="mesh-preview">
      <canvas
        ref={canvas}
        className="vc-mesh__canvas"
        width={Math.round(MESH_W * dpr)}
        height={Math.round(MESH_H * dpr)}
        aria-hidden
      />
      {points.map((p, i) => (
        <button
          key={i}
          type="button"
          className="vc-mesh__point"
          aria-label={`${meshPointLabel(paint, i)} point`}
          aria-pressed={props.selected === i}
          style={{
            left: 14 + p.x * (MESH_W - 28) - 6,
            top: 14 + p.y * (MESH_H - 28) - 6,
            background: ctx.colour(p.color),
          }}
          onClick={() => props.onSelect(i)}
        />
      ))}
    </div>
  );
}

function GradientBody(props: {
  ctx: DesignContext;
  paint: Paint;
  scope: PaintScope;
  index: number;
  apply(fn: (p: Paint) => Paint): void;
}) {
  const { ctx, paint, apply } = props;
  const kind = paintKind(paint);
  const resolve = (c: Value<string>) => ctx.colour(c);
  const editor = ctx.paintEditor;
  const point = editor.state?.point ?? null;
  const mesh = kind === "mesh";
  const raw = paint as unknown as Loose;
  const stops = ((raw.stops ?? []) as { color: Value<string>; position: number }[]) ?? [];
  const points = ((raw.points ?? []) as { color: Value<string> }[]) ?? [];
  const [grid, setGrid] = useState<string | null>(null);
  const current = point !== null ? points[point] : undefined;
  const swatch = paintSwatch(paint, resolve);
  return (
    <>
      {mesh ? (
        <MeshPreview
          ctx={ctx}
          paint={paint}
          selected={point}
          onSelect={(i) => editor.selectPoint(i)}
        />
      ) : (
        <div
          className="vc-gradient"
          style={{ background: swatch ?? undefined }}
          role="img"
          aria-label={`${paintLabel(paint)} preview`}
        />
      )}
      <div className="vc-prop vc-prop--wide">
        <span className="vc-prop__label">Type</span>
        <span className="vc-prop__control">
          <SelectField
            label="Gradient type"
            value={kind as "linear" | "radial" | "angular" | "mesh"}
            options={GRADIENT_TYPES}
            className="vc-field--28"
            disabled={ctx.disabled}
            onChange={(next) => {
              if (next === kind) return;
              apply((p) =>
                next === "mesh"
                  ? (convertPaint(p, "mesh", resolve) ?? p)
                  : mesh
                    ? (convertPaint(p, next, resolve) ?? p)
                    : setGradientKind(p, next as GradientKind),
              );
            }}
          />
        </span>
      </div>
      {mesh ? (
        <>
          <div className="vc-prop vc-prop--wide">
            <span className="vc-prop__label">Grid</span>
            <span className="vc-prop__control">
              <TextField
                label="Grid size"
                value={grid ?? `${raw.columns} × ${raw.rows}`}
                height={28}
                disabled={ctx.disabled}
                onCommit={(text) => {
                  const m = /^\s*(\d+)\s*[x×*]\s*(\d+)\s*$/i.exec(text);
                  if (!m) return `Enter a size such as 3 × 3`;
                  const cols = Number(m[1]);
                  const rows = Number(m[2]);
                  if (cols < MESH_MIN || rows < MESH_MIN || cols > MESH_MAX || rows > MESH_MAX)
                    return `Use ${MESH_MIN} to ${MESH_MAX} points each way`;
                  setGrid(null);
                  apply((p) => resizeMesh(p, cols, rows, resolve));
                  return null;
                }}
              />
            </span>
          </div>
          <div className="vc-prop vc-prop--wide">
            <span className="vc-prop__label">Point</span>
            <span className="vc-prop__control">
              {point !== null && current ? (
                <span
                  className="vc-field"
                  data-height="28"
                  data-state={ctx.disabled ? "disabled" : "default"}
                >
                  <span className="vc-mono-value vc-field__prefix">
                    {meshPointLabel(paint, point)} ·
                  </span>
                  <HexInline
                    ctx={ctx}
                    value={resolve(current.color)}
                    disabled={ctx.disabled}
                    label={`${meshPointLabel(paint, point)} colour`}
                    onColour={(hex) => apply((p) => setMeshColour(p, point, hex))}
                    onStep={(by) =>
                      editor.selectPoint((point + by + points.length) % points.length)
                    }
                  />
                </span>
              ) : (
                <span className="vc-field" data-height="28" data-state="disabled">
                  <span className="vc-mono-value">No point selected</span>
                </span>
              )}
            </span>
          </div>
          <p className="vc-paint__hint">
            Move a point on the canvas. Select it to change its colour.
          </p>
        </>
      ) : (
        <>
          <fieldset className="vc-stops" aria-label="Colour stops">
            {stops.map((s, i) => (
              <div className="vc-stop" key={i}>
                <ValueField
                  label={`Stop ${i + 1} position`}
                  value={{ kind: "value", value: s.position }}
                  spec={{ suffix: "%", scale: 100, min: 0, max: 1, decimals: 0 }}
                  disabled={ctx.disabled}
                  onCommit={(v) => apply((p) => moveStop(p, i, v).paint)}
                />
                <HexField
                  ctx={ctx}
                  value={resolve(s.color)}
                  label={`Stop ${i + 1} colour`}
                  disabled={ctx.disabled}
                  height={26}
                  onColour={(hex) => apply((p) => setStopColour(p, i, hex))}
                />
                <button
                  type="button"
                  className="vc-iconbutton"
                  aria-label={`Remove stop ${i + 1}`}
                  disabled={ctx.disabled || stops.length <= 2}
                  onClick={() => apply((p) => removeStop(p, i))}
                >
                  <Icon name="minus" size={16} />
                </button>
              </div>
            ))}
          </fieldset>
          <div className="vc-paint__row">
            <TextButton
              disabled={ctx.disabled}
              onClick={() => apply((p) => addStop(p, 0.5, resolve))}
            >
              Add stop
            </TextButton>
            {kind === "linear" || kind === "angular" ? (
              <ValueField
                label="Angle"
                axis="Angle"
                value={{ kind: "value", value: gradientAngle(paint) }}
                spec={SPEC.degrees}
                disabled={ctx.disabled}
                onCommit={(v) => apply((p) => setGradientAngle(p, v))}
              />
            ) : null}
          </div>
        </>
      )}
    </>
  );
}

/** A hex input that sits inside another field, with Page Up and Page Down to move between points. */
export function HexInline(props: {
  ctx: DesignContext;
  value: string;
  label: string;
  disabled?: boolean;
  onColour(hex: string): void;
  onStep?(by: 1 | -1): void;
}) {
  const [text, setText] = useState<string | null>(null);
  const [bad, setBad] = useState(false);
  const reverting = useRef(false);
  const commit = (raw: string) => {
    setText(null);
    if (raw === props.value) return;
    const hex = normalizeHex(raw);
    if (!hex) {
      setBad(true);
      return;
    }
    setBad(false);
    const typedAlpha = raw.replace(/^#/, "").length === 8 || raw.replace(/^#/, "").length === 4;
    props.onColour(typedAlpha ? hex : setAlpha(hex, alphaOf(props.value)));
  };
  return (
    <input
      className="vc-field__input vc-mono-value"
      aria-label={props.label}
      aria-invalid={bad ? true : undefined}
      disabled={props.disabled}
      autoComplete="off"
      spellCheck={false}
      value={text ?? props.value}
      onFocus={(e) => e.currentTarget.select()}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={(e) => {
        if (reverting.current) {
          reverting.current = false;
          return;
        }
        if (text !== null) commit(e.currentTarget.value);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit(e.currentTarget.value);
        } else if (e.key === "PageUp" || e.key === "PageDown") {
          e.preventDefault();
          props.onStep?.(e.key === "PageUp" ? -1 : 1);
        } else if (e.key === "Escape") {
          e.stopPropagation();
          reverting.current = true;
          setText(null);
          setBad(false);
          e.currentTarget.blur();
        }
      }}
    />
  );
}

/* ------------------------------------------------------------------------------------------ image */

function ImageBody(props: {
  onChoose?(): void;
  ctx: DesignContext;
  paint: Paint;
  scope: PaintScope;
  index: number;
  apply(fn: (p: Paint) => Paint): void;
}) {
  const { ctx, paint, apply } = props;
  const isImage = paintKind(paint) === "image";
  const raw = paint as unknown as Loose;
  const cropping = ctx.paintEditor.state?.cropping ?? false;
  const choose = (command: string) => {
    if (ctx.host.authoring && ctx.session.state.selection.length === 1) {
      ctx.host.authoring.openImage(
        ctx.session.state.selection[0] as string,
        props.scope,
        props.index,
      );
      props.onChoose?.();
    } else ctx.host.onAction?.(command, { scope: props.scope, index: props.index });
  };
  if (!isImage)
    return (
      <>
        <p className="vc-paint__hint">Fill with a picture from a file, a link or stock images.</p>
        <TextButton
          disabled={ctx.disabled || (!ctx.host.onAction && !ctx.host.authoring)}
          title={
            ctx.host.onAction || ctx.host.authoring
              ? undefined
              : "Choosing an image is not available here"
          }
          onClick={() => choose("image.choose")}
        >
          Choose image…
        </TextButton>
      </>
    );
  return (
    <>
      <div className="vc-prop vc-prop--wide">
        <span className="vc-prop__label">Image</span>
        <span className="vc-prop__control">
          <span className="vc-field" data-height="28" data-state="default">
            <span className="vc-mono-value vc-select__value">{String(raw.asset)}</span>
          </span>
        </span>
      </div>
      <Segmented
        label="Image fit"
        value={(raw.fit as "fill" | "fit" | "stretch" | undefined) ?? "fill"}
        disabled={ctx.disabled}
        options={imageFits.map((f) => ({ value: f, label: f[0]?.toUpperCase() + f.slice(1) }))}
        onChange={(fit) => apply((p) => setImageFit(p, fit))}
      />
      <div className="vc-paint__row">
        <TextButton disabled={ctx.disabled} onClick={() => ctx.paintEditor.setCropping(!cropping)}>
          {cropping ? "Done cropping" : "Crop on canvas"}
        </TextButton>
        <TextButton
          disabled={ctx.disabled || (!ctx.host.onAction && !ctx.host.authoring)}
          title={
            ctx.host.onAction || ctx.host.authoring
              ? undefined
              : "Replacing an image is not available here"
          }
          onClick={() => choose("image.replace")}
        >
          Replace…
        </TextButton>
      </div>
      {cropping ? (
        <p className="vc-paint__hint">Drag a corner of the image on the canvas to crop it.</p>
      ) : null}
    </>
  );
}

/* ----------------------------------------------------------------------------------------- popover */

type Tab = "solid" | "gradient" | "image" | "shader";
const TABS: readonly { id: Tab; label: string }[] = [
  { id: "solid", label: "Solid" },
  { id: "gradient", label: "Gradient" },
  { id: "image", label: "Image" },
  { id: "shader", label: "Shader" },
];
const tabOf = (kind: PaintKind): Tab =>
  kind === "solid"
    ? "solid"
    : kind === "image"
      ? "image"
      : kind === "shader"
        ? "shader"
        : "gradient";

/**
 * The fill editor (C33), opened from a swatch or Edit fill. Changing a field writes at once, one transaction each;
 * mesh points and image crop corners are dragged on the canvas while it is open.
 */
export function PaintPopover(props: {
  ctx: DesignContext;
  scope: PaintScope;
  index: number;
  triggerClassName: string;
  triggerLabel: string;
  trigger: ReactNode;
  title?: string;
}) {
  const container = usePortalContainer();
  const { ctx, scope, index } = props;
  const [open, setOpen] = useState(false);
  const paint = open ? paintAt(ctx, scope, index) : null;
  const [tab, setTab] = useState<Tab>("solid");
  const kind = paint ? paintKind(paint) : null;
  const resolve = (c: Value<string>) => ctx.colour(c);
  // Opening the editor starts on the tab of the paint, and the canvas shows its points.
  useEffect(() => {
    if (!open) return;
    const p = paintAt(ctx, scope, index);
    if (p) setTab(tabOf(paintKind(p)));
    ctx.paintEditor.open({ scope, index });
    return () => ctx.paintEditor.close();
  }, [open]);
  // Another layer selected: this paint is not the one on screen any more.
  const selection = ctx.session.state.selection.join(",");
  const openedOn = useRef(selection);
  if (!open) openedOn.current = selection;
  useEffect(() => {
    if (open && openedOn.current !== selection) setOpen(false);
  }, [open, selection]);
  const apply = (fn: (p: Paint) => Paint) => changePaint(ctx, scope, index, fn);
  const choose = (next: Tab) => {
    setTab(next);
    if (!paint || ctx.disabled) return;
    if (next === "solid" && kind !== "solid") apply((p) => convertPaint(p, "solid", resolve) ?? p);
    if (next === "gradient" && tabOf(paintKind(paint)) !== "gradient")
      apply((p) => convertPaint(p, "linear", resolve) ?? p);
  };
  return (
    <Popover.Root
      open={open}
      onOpenChange={(next, details) => {
        // The fill editor works with the canvas: a press on a mesh point or crop corner must not close it.
        const target =
          details.reason === "focus-out"
            ? (details.event as FocusEvent).relatedTarget
            : details.event.target;
        const onCanvas = (target as Element | null)?.closest?.(".vc-canvas");
        if (
          !next &&
          onCanvas &&
          (details.reason === "outside-press" || details.reason === "focus-out")
        ) {
          details.cancel();
          return;
        }
        setOpen(next);
      }}
    >
      <Popover.Trigger className={props.triggerClassName} aria-label={props.triggerLabel}>
        {props.trigger}
      </Popover.Trigger>
      <Popover.Portal container={container}>
        <Popover.Positioner side="left" align="start" sideOffset={12} collisionPadding={8}>
          <Popover.Popup
            className="vc-popover vc-paint"
            aria-label="Fill editor"
            data-testid="paint-editor"
          >
            {paint ? (
              <>
                <h4 className="vc-paint__title">
                  {scope === "fill" ? "Fill" : "Stroke"} · {paintLabel(paint)}
                </h4>
                <div className="vc-paint__tabs" role="tablist" aria-label="Paint type">
                  {TABS.map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      role="tab"
                      className="vc-paint__tab"
                      aria-selected={tab === t.id}
                      onClick={() => choose(t.id)}
                    >
                      {t.label}
                    </button>
                  ))}
                </div>
                {tab === "solid" && paintKind(paint) === "solid" ? (
                  <SolidBody ctx={ctx} paint={paint} apply={apply} />
                ) : null}
                {tab === "gradient" && tabOf(paintKind(paint)) === "gradient" ? (
                  <GradientBody ctx={ctx} paint={paint} scope={scope} index={index} apply={apply} />
                ) : null}
                {tab === "image" ? (
                  <ImageBody
                    ctx={ctx}
                    paint={paint}
                    scope={scope}
                    index={index}
                    apply={apply}
                    onChoose={() => setOpen(false)}
                  />
                ) : null}
                {tab === "shader" ? (
                  <ShaderControls
                    ctx={ctx}
                    shader={
                      paintKind(paint) === "shader"
                        ? (paint as Extract<Paint, { type: "shader" }>)
                        : null
                    }
                    apply={(v) => apply(() => ({ type: "shader", ...v }) as Paint)}
                  />
                ) : null}
                <Popover.Close className="vc-button vc-paint__done" data-primary="">
                  Done
                </Popover.Close>
              </>
            ) : (
              <p className="vc-paint__hint">This paint is no longer there.</p>
            )}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}
