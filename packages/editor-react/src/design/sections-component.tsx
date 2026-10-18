import { Menu } from "@base-ui/react/menu";
import {
  detachOps,
  type Edit,
  type InstanceModel,
  type MainComponentModel,
  replaceInstanceOps,
  resetOverridePatch,
  resetPropPatch,
  resetVariantPatch,
  setComponentNameOps,
  setComponentPropOps,
  setInstancePropPatch,
  setVariantPatch,
} from "@vellum/editor-core";
import type { NodePatch, Scalar } from "@vellum/types-ts";
import { type ReactNode, useEffect, useState } from "react";
import { usePortalContainer } from "../chrome/portal";
import type { DesignContext } from "./context";
import {
  Banner,
  PropertyRow,
  Section,
  SelectField,
  TextButton,
  TextField,
  ToggleRow,
  ValueField,
} from "./controls";
import { SPEC } from "./specs";

type Prop = MainComponentModel["props"][number];

function PropControl(props: {
  ctx: DesignContext;
  prop: Pick<Prop, "name" | "type"> & { value: Scalar };
  disabled: boolean;
  set(value: Scalar): void;
  reset?: ReactNode;
}) {
  const { prop } = props;
  if (prop.type === "boolean")
    return (
      <ToggleRow
        label={prop.name}
        checked={prop.value === true}
        disabled={props.disabled}
        reset={props.reset}
        onChange={(on) => props.set(on)}
      />
    );
  return (
    <PropertyRow label={prop.name}>
      {prop.type === "number" ? (
        <ValueField
          label={prop.name}
          value={{ kind: "value", value: Number(prop.value) }}
          spec={SPEC.bare}
          disabled={props.disabled}
          onCommit={(v) => props.set(v)}
        />
      ) : (
        <TextField
          label={prop.name}
          value={String(prop.value)}
          disabled={props.disabled}
          onCommit={(v) => {
            props.set(v);
            return null;
          }}
        />
      )}
    </PropertyRow>
  );
}

/** A main component: its name and the default of each prop. Changes update every instance. */
function MainSection({ ctx, model }: { ctx: DesignContext; model: MainComponentModel }) {
  const send = (ops: ReturnType<typeof setComponentNameOps>) => ctx.design.ops(ops);
  return (
    <Section id="component" title="Component" density={ctx.density}>
      <PropertyRow label="Name">
        <TextField
          label="Component name"
          value={model.name}
          disabled={ctx.disabled}
          onCommit={(name) => {
            if (!name.trim()) return "A component needs a name";
            send(setComponentNameOps(ctx.document, model.id, name));
            return null;
          }}
        />
      </PropertyRow>
      {model.axes.map((axis) => (
        <PropertyRow key={axis.name} label={axis.name}>
          <SelectField
            label={axis.name}
            value={axis.values[0] ?? null}
            options={axis.values.map((v) => ({ value: v, label: v }))}
            disabled
            onChange={() => {}}
          />
        </PropertyRow>
      ))}
      {model.props.map((p) => (
        <PropControl
          key={p.name}
          ctx={ctx}
          prop={p}
          disabled={ctx.disabled}
          set={(v) => send(setComponentPropOps(ctx.document, model.id, p.name, v))}
        />
      ))}
      <span className="vc-section__empty">Changes update every instance.</span>
    </Section>
  );
}

function ResetLink(props: { label: string; ctx: DesignContext; onClick(): void }) {
  return (
    <button
      type="button"
      className="vc-textlink"
      disabled={props.ctx.disabled}
      onClick={props.onClick}
    >
      {props.label}
    </button>
  );
}

/** An instance: the main component, the choices made on this instance with a Reset for each, and the actions. */
function InstanceSection({ ctx, model }: { ctx: DesignContext; model: InstanceModel }) {
  const container = usePortalContainer();
  const target = ctx.design.single();
  const item = target?.item;
  const components = Object.entries(
    (ctx.document?.components ?? {}) as Record<string, { name: string }>,
  );
  const [pick, setPick] = useState(false);
  // "Replace inside instance" from the keyboard asks for the picker; the panel opens it.
  useEffect(() => {
    if (ctx.design.takeRequest() === "replace") setPick(true);
  });
  const patch = (build: Edit) => ctx.edit(build);
  const reset_ =
    (f: (i: NonNullable<typeof item>) => NodePatch | null): Edit =>
    (t) =>
      f(t.item);
  const sendOps = (ops: Parameters<typeof ctx.design.ops>[0]) => ctx.design.ops(ops);
  return (
    <Section id="component" title="Instance" density={ctx.density}>
      {model.missing ? <Banner icon="info">This component is missing from the file</Banner> : null}
      <PropertyRow label="Main">
        <span className="vc-field" title={model.library ? `From ${model.library}` : undefined}>
          <span className="vc-mono-value vc-select__value">{model.component}</span>
        </span>
      </PropertyRow>
      {model.axes.map((axis) => (
        <div key={axis.name} className="vc-component__row">
          <PropertyRow label={axis.name}>
            <SelectField
              label={axis.name}
              value={axis.value}
              options={axis.values.map((v) => ({ value: v, label: v }))}
              disabled={ctx.disabled}
              onChange={(v) => patch(reset_((i) => setVariantPatch(i, axis.name, v)))}
            />
          </PropertyRow>
          {axis.overridden ? (
            <ResetLink
              ctx={ctx}
              label={`Reset ${axis.name.toLowerCase()}`}
              onClick={() => patch(reset_((i) => resetVariantPatch(i, axis.name)))}
            />
          ) : null}
        </div>
      ))}
      {[
        ...model.props.filter((p) => p.type !== "boolean"),
        ...model.props.filter((p) => p.type === "boolean"),
      ].map((p) => {
        const reset = p.overridden ? (
          <ResetLink
            ctx={ctx}
            label={p.type === "boolean" ? "Reset" : `Reset ${p.name.toLowerCase()}`}
            onClick={() => patch(reset_((i) => resetPropPatch(i, p.name)))}
          />
        ) : null;
        return (
          <div key={p.name} className="vc-component__row">
            <PropControl
              ctx={ctx}
              prop={p}
              disabled={ctx.disabled}
              set={(v) => patch(reset_((i) => setInstancePropPatch(i, p.name, v)))}
              {...(p.type === "boolean" ? { reset } : {})}
            />
            {p.type !== "boolean" ? reset : null}
          </div>
        );
      })}
      {model.overrides.length ? (
        <fieldset className="vc-component__overrides" aria-label="Changes inside this instance">
          {model.overrides.map((o) => (
            <div key={o.path} className="vc-component__override">
              <span className="vc-mono-value vc-component__path">{o.path.split("/").at(-1)}</span>
              <span className="vc-section__empty">{o.summary}</span>
              <ResetLink
                ctx={ctx}
                label="Reset"
                onClick={() => patch(reset_((i) => resetOverridePatch(i, o.path)))}
              />
            </div>
          ))}
        </fieldset>
      ) : null}
      <div className="vc-component__actions">
        <TextButton
          disabled={!(ctx.host.onGoToMain || !model.library)}
          title={
            ctx.host.onGoToMain ? undefined : "Showing the main component is not available here"
          }
          onClick={() =>
            ctx.host.onGoToMain?.({ componentId: model.componentId, library: model.library })
          }
        >
          {model.library ? "Open library" : "Go to main component"}
        </TextButton>
        <Menu.Root open={pick} onOpenChange={setPick}>
          <Menu.Trigger className="vc-button" disabled={ctx.disabled || !item}>
            Replace inside instance
          </Menu.Trigger>
          <Menu.Portal container={container}>
            <Menu.Positioner side="bottom" align="start" sideOffset={4}>
              <Menu.Popup className="vc-menu">
                <Menu.Group>
                  <Menu.GroupLabel className="vc-menu__heading">Replace with</Menu.GroupLabel>
                  {components.length ? (
                    components.map(([id, c]) => (
                      <Menu.Item
                        key={id}
                        className="vc-menu__item"
                        disabled={!model.library && id === model.componentId}
                        onClick={() =>
                          item &&
                          sendOps(replaceInstanceOps(item, { componentId: id, library: null }))
                        }
                      >
                        {c.name}
                      </Menu.Item>
                    ))
                  ) : (
                    <Menu.Item className="vc-menu__item" disabled>
                      No components in this file
                    </Menu.Item>
                  )}
                </Menu.Group>
              </Menu.Popup>
            </Menu.Positioner>
          </Menu.Portal>
        </Menu.Root>
        <TextButton
          disabled={ctx.disabled || !item || model.nested}
          title={
            model.nested
              ? "This instance is inside another one: detach the outer instance first"
              : "Turns the instance into layers. Nested instances and variable links stay linked."
          }
          onClick={() => item && sendOps(detachOps(item))}
        >
          Detach
        </TextButton>
      </div>
    </Section>
  );
}

export function ComponentSection({ ctx }: { ctx: DesignContext }) {
  const model = ctx.model.component;
  if (!model) return null;
  return model.kind === "main" ? (
    <MainSection ctx={ctx} model={model} />
  ) : (
    <InstanceSection ctx={ctx} model={model} />
  );
}
