import { copy } from "@vellum/config";
import {
  type DesignController,
  type EditorSession,
  frameTheme,
  normalizeHex,
  setStrokeWidth,
  themeAxes,
} from "@vellum/editor-core";
import type { Stroke, Document as VellumDocument } from "@vellum/types-ts";
import { useRef, useState } from "react";
import { Pair, PropertyRow, Section, SelectField, TextField, ValueField } from "../design/controls";
import { SPEC } from "../design/specs";
import { Icon } from "../icons/Icon";
import { ResourceButton as Button } from "./Dialog";
import "./resources.css";
export function ThemeSection({
  design,
  session,
  document: doc,
  disabled,
}: {
  design: DesignController;
  session: EditorSession;
  document: VellumDocument;
  disabled: boolean;
}) {
  const all = design.targets();
  if (
    !Object.keys(doc.themes ?? {}).length ||
    !all.length ||
    all.some((t) => t.item.node.type !== "frame")
  )
    return null;
  const inherited = frameTheme(doc, all[0]?.item.parent ?? null, (id) => session.sceneItem(id));
  const own = (all[0]?.item.node as { theme?: Record<string, string> } | undefined)?.theme ?? {};
  return (
    <Section id="theme" title="Theme" className="vc-theme-section">
      {themeAxes(doc).map(([axis, values]) => {
        const choices = all.map(
          (t) => (t.item.node as { theme?: Record<string, string> }).theme?.[axis] ?? "__inherit",
        );
        const mixed = choices.some((v) => v !== choices[0]);
        return (
          <PropertyRow key={axis} label={axis}>
            <SelectField
              label={`Theme ${axis}`}
              value={mixed ? null : (choices[0] ?? "__inherit")}
              mixed={mixed}
              disabled={disabled}
              options={[
                { value: "__inherit", label: `Inherits ${inherited[axis] ?? values[0]}` },
                ...values.map((v) => ({ value: v, label: v })),
              ]}
              onChange={(value) =>
                design.edit((t) => {
                  const theme = { ...(t.item.node as { theme?: Record<string, string> }).theme };
                  if (value === "__inherit") delete theme[axis];
                  else theme[axis] = value;
                  return { theme: Object.keys(theme).length ? theme : null };
                })
              }
            />
          </PropertyRow>
        );
      })}
      <p>
        {Object.keys(own).length
          ? `This frame uses ${Object.values(own).join(" · ")}. Its children inherit; neighbouring frames keep ${Object.keys(
              own,
            )
              .map((axis) => inherited[axis])
              .join(" · ")}.`
          : "Children inherit unless they set the same axis."}
      </p>
      <Button
        disabled={
          disabled ||
          all.every((t) => !Object.keys((t.item.node as { theme?: object }).theme ?? {}).length)
        }
        onClick={() => design.edit(() => ({ theme: null }))}
      >
        Reset to inherited
      </Button>
    </Section>
  );
}
export function ScalePanel({ session, disabled }: { session: EditorSession; disabled: boolean }) {
  const mode = session.state.mode;
  const input = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  if (mode.type !== "scaling") return null;
  const boxes = mode.ids.flatMap((id) => {
    const b = session.sceneBounds(id);
    return b ? [b] : [];
  });
  const width = boxes.length === 1 ? (boxes[0]?.width ?? 0) * mode.factor : null;
  const height = boxes.length === 1 ? (boxes[0]?.height ?? 0) * mode.factor : null;
  const factor = (value: string) => {
    const result = session.dispatch({ type: "scaleFactor", factor: value });
    setError(result.error ? "Enter a positive factor or percentage" : null);
  };
  return (
    <div className="vc-resource-panel">
      <Section id="scale" title="Scale · K" className="vc-scale-section">
        <p>Anchor</p>
        <div className="vc-scale-anchor" role="radiogroup" aria-label="Scale anchor">
          {[0, 0.5, 1].flatMap((y) =>
            [0, 0.5, 1].map((x) => (
              <label key={`${x}:${y}`}>
                <input
                  type="radio"
                  name="scale-anchor"
                  aria-label={`Anchor ${x * 2}, ${y * 2}`}
                  checked={mode.anchor.x === x && mode.anchor.y === y}
                  disabled={disabled}
                  onChange={() => session.dispatch({ type: "scaleAnchor", anchor: { x, y } })}
                />
                <Icon name="plus" />
              </label>
            )),
          )}
        </div>
        <PropertyRow label="Factor">
          <input
            ref={input}
            className="vc-field vc-mono-value"
            aria-label="Scale factor"
            disabled={disabled}
            key={mode.factor}
            defaultValue={`${mode.factor * 100}%`}
            onBlur={(e) => factor(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                e.stopPropagation();
                const result = session.dispatch({
                  type: "scaleFactor",
                  factor: e.currentTarget.value,
                });
                if (!result.error) session.dispatch({ type: "finish" });
                else setError("Enter a positive factor or percentage");
              }
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                session.dispatch({ type: "tool", tool: "move" });
              }
            }}
          />
        </PropertyRow>
        <PropertyRow label="Width">
          <output className="vc-field vc-mono-value">
            {width === null ? "Mixed" : `${Math.round(width * 100) / 100} px`}
          </output>
        </PropertyRow>
        <PropertyRow label="Height">
          <output className="vc-field vc-mono-value">
            {height === null ? "Mixed" : `${Math.round(height * 100) / 100} px`}
          </output>
        </PropertyRow>
        <p>{copy.scaleNote}</p>
        <p>Corner handles scale proportionally. Enter applies and returns to Move.</p>
        {error ? (
          <p role="alert" className="vc-resource-error">
            <span aria-hidden>×</span> <span>{error}</span>
          </p>
        ) : null}
        <Button
          primary
          disabled={disabled || !mode.ids.length}
          onClick={() => {
            const result = session.dispatch({
              type: "scaleFactor",
              factor: input.current?.value ?? mode.factor,
            });
            if (!result.error) session.dispatch({ type: "finish" });
            else setError("Enter a positive factor or percentage");
          }}
        >
          Apply scale
        </Button>
        <Button onClick={() => session.dispatch({ type: "tool", tool: "move" })}>Cancel</Button>
      </Section>
    </div>
  );
}
export function VectorPanel({
  session,
  design,
  disabled,
}: {
  session: EditorSession;
  design: DesignController;
  disabled: boolean;
}) {
  const mode = session.state.mode;
  if (mode.type !== "path") return null;
  const stroke = (design.single()?.item.node as { stroke?: Stroke | null } | undefined)?.stroke;
  const paint = stroke?.paints[0];
  const target = mode.selected.length === 1 ? mode.selected[0] : null;
  const point =
    target?.type === "point" ? mode.path.contours[target.contour]?.points[target.index] : null;
  const field = (axis: "x" | "y") => (
    <ValueField
      label={`Point ${axis.toUpperCase()}`}
      axis={axis.toUpperCase()}
      value={point ? { kind: "value", value: point.point[axis] } : { kind: "none" }}
      spec={SPEC.bare}
      disabled={disabled || !point}
      onCommit={(value) => {
        if (point && target)
          session.dispatch({ type: "pathPoint", target, point: { ...point.point, [axis]: value } });
      }}
    />
  );
  return (
    <div className="vc-resource-panel">
      <Section id="vector-point" title="Vector point" density="roomy">
        <Pair>
          {field("x")}
          {field("y")}
        </Pair>
        <PropertyRow label="Radius">
          <ValueField
            label="Point radius"
            value={{ kind: "value", value: 0 }}
            spec={SPEC.length}
            disabled
            reason="Point radius is not supported by the current path model"
            onCommit={() => {}}
          />
        </PropertyRow>
      </Section>
      <Section id="path" title="Path" density="roomy">
        <PropertyRow label="Stroke">
          <TextField
            label="Path stroke"
            mono
            value={paint?.type === "solid" ? design.colour(paint.color) : "Mixed"}
            disabled={disabled || paint?.type !== "solid"}
            onCommit={(text) => {
              const color = normalizeHex(text);
              if (!color) return "Enter a colour";
              design.edit((t) => {
                const current = (t.item.node as { stroke?: Stroke | null }).stroke;
                if (!current) return null;
                return {
                  stroke: {
                    ...current,
                    paints: current.paints.map((p, i) =>
                      i === 0 && p.type === "solid" ? { ...p, color } : p,
                    ),
                  },
                };
              });
              return null;
            }}
          />
        </PropertyRow>
        <PropertyRow label="Width">
          <ValueField
            label="Path stroke width"
            value={
              typeof stroke?.width === "number"
                ? { kind: "value", value: stroke.width }
                : { kind: "mixed" }
            }
            spec={SPEC.length}
            disabled={disabled || !stroke}
            onCommit={(value) =>
              design.edit((t) => {
                const current = (t.item.node as { stroke?: Stroke | null }).stroke;
                return current ? { stroke: setStrokeWidth(current, value) } : null;
              })
            }
          />
        </PropertyRow>
        <div className="vc-resource-actions">
          {(
            [
              ["toggleSmooth", "Smooth / corner"],
              ["cut", "Cut"],
              ["connect", "Connect"],
              ["straighten", "Straighten"],
              ["delete", "Delete"],
            ] as const
          ).map(([action, label]) => (
            <Button
              key={action}
              disabled={
                disabled ||
                !mode.selected.length ||
                ((action === "cut" || action === "straighten") &&
                  mode.selected.some((t) => t.type !== "segment")) ||
                (action === "connect" &&
                  (mode.selected.length !== 2 || mode.selected.some((t) => t.type !== "point"))) ||
                (action === "toggleSmooth" && mode.selected.some((t) => t.type !== "point"))
              }
              onClick={() => session.dispatch({ type: "pathAction", action })}
            >
              {label}
            </Button>
          ))}
        </div>
      </Section>
    </div>
  );
}
