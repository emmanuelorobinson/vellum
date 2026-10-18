import type {
  Component,
  ComponentId,
  Document,
  NodePatch,
  PropDef,
  Scalar,
} from "@vellum/types-ts";
import type { SceneItem, ToolOp } from "../tool-types";
import { patchOps } from "./patch";

export interface MainComponentModel {
  kind: "main";
  id: ComponentId;
  name: string;
  /** Variant axes and their values, in the order the component lists them. Edited in the file or by an agent. */
  axes: { name: string; values: string[] }[];
  props: { name: string; type: PropDef["type"]; value: Scalar }[];
}
export interface InstanceModel {
  kind: "instance";
  componentId: ComponentId;
  /** The component's name; its id when it is not in this file. */
  component: string;
  /** The import alias when the component lives in a library. */
  library: string | null;
  /** The definition is not in the file. */
  missing: boolean;
  /** This instance sits inside another instance, so it can change only through an override. */
  nested: boolean;
  axes: { name: string; values: string[]; value: string; overridden: boolean }[];
  props: {
    name: string;
    type: PropDef["type"];
    value: Scalar;
    default: Scalar;
    overridden: boolean;
  }[];
  overrides: { path: string; summary: string }[];
}
export type ComponentModel = MainComponentModel | InstanceModel;

type Loose = Record<string, unknown>;
const components = (doc: Document | null) =>
  (doc?.components ?? {}) as Record<ComponentId, Component>;

function axesOf(component: Component): { name: string; values: string[] }[] {
  const axes = new Map<string, string[]>();
  for (const variant of component.variants ?? [])
    for (const [axis, value] of Object.entries(variant.properties)) {
      const list = axes.get(axis) ?? [];
      if (!list.includes(value)) list.push(value);
      axes.set(axis, list);
    }
  return [...axes].map(([name, values]) => ({ name, values }));
}
const propsOf = (component: Component) =>
  Object.entries(component.props ?? {}).map(([name, def]) => ({
    name,
    type: def.type,
    default: def.default as Scalar,
  }));
const overrideSummary = (o: Loose): string =>
  "props" in o ? "Changed properties" : "replace" in o ? "Replaced" : "Changed children";

/** What the Component or Instance section shows for the selection: one main component or one instance. */
export function readComponent(
  items: readonly SceneItem[],
  doc: Document | null,
): ComponentModel | null {
  const [only, ...more] = items;
  if (!only || more.length) return null;
  const node = only.node as unknown as Loose;
  if (node.type === "instance") {
    const componentId = node.componentId as ComponentId;
    const library = (node.library as string | null | undefined) ?? null;
    const component = library ? undefined : components(doc)[componentId];
    const chosen = (node.variant as Record<string, string> | undefined) ?? {};
    const given = (node.props as Record<string, Scalar> | undefined) ?? {};
    return {
      kind: "instance",
      componentId,
      component: component?.name ?? componentId,
      library,
      missing: !library && !component,
      nested: !!only.instance,
      axes: component
        ? axesOf(component).map((a) => ({
            ...a,
            value: chosen[a.name] ?? (a.values[0] as string),
            overridden: a.name in chosen,
          }))
        : [],
      props: component
        ? propsOf(component).map((p) => ({
            ...p,
            value: p.name in given ? (given[p.name] as Scalar) : p.default,
            overridden: p.name in given,
          }))
        : [],
      overrides: Object.entries((node.overrides as Record<string, Loose> | undefined) ?? {}).map(
        ([path, o]) => ({ path, summary: overrideSummary(o) }),
      ),
    };
  }
  if (only.instance) return null;
  for (const [id, component] of Object.entries(components(doc)))
    if ((component.root as unknown as Loose).id === node.id)
      return {
        kind: "main",
        id,
        name: component.name,
        axes: axesOf(component),
        props: propsOf(component).map((p) => ({ name: p.name, type: p.type, value: p.default })),
      };
  return null;
}

/* ---------------------------------------------------------------------------------- instance patches */

const isInstance = (item: SceneItem) => item.node.type === "instance";
const without = <T extends Record<string, unknown>>(o: T, key: string) => {
  const { [key]: _old, ...rest } = o;
  return rest;
};
const orNull = (o: Record<string, unknown>) => (Object.keys(o).length ? o : null);
const of = (item: SceneItem, key: string) =>
  ((item.node as unknown as Loose)[key] ?? {}) as Record<string, unknown>;

export function setVariantPatch(item: SceneItem, axis: string, value: string): NodePatch | null {
  return isInstance(item) ? { variant: { ...of(item, "variant"), [axis]: value } } : null;
}
export function resetVariantPatch(item: SceneItem, axis: string): NodePatch | null {
  return isInstance(item) ? { variant: orNull(without(of(item, "variant"), axis)) } : null;
}
export function setInstancePropPatch(
  item: SceneItem,
  name: string,
  value: Scalar,
): NodePatch | null {
  return isInstance(item) ? { props: { ...of(item, "props"), [name]: value } } : null;
}
export function resetPropPatch(item: SceneItem, name: string): NodePatch | null {
  return isInstance(item) ? { props: orNull(without(of(item, "props"), name)) } : null;
}
export function resetOverridePatch(item: SceneItem, path: string): NodePatch | null {
  return isInstance(item) ? { overrides: orNull(without(of(item, "overrides"), path)) } : null;
}

/** Points an instance at another component; the choices that belonged to the old one go. */
export function replaceInstanceOps(
  item: SceneItem,
  to: { componentId: ComponentId; library: string | null },
): ToolOp[] {
  const node = item.node as unknown as Loose;
  if (!isInstance(item)) return [];
  if (
    node.componentId === to.componentId &&
    ((node.library as string | null) ?? null) === to.library
  )
    return [];
  return patchOps([item], () => ({
    componentId: to.componentId,
    library: to.library,
    variant: null,
    props: null,
    overrides: null,
    slotContent: null,
  }));
}
/** Expands an instance into plain layers. Nested instances and variable links stay linked. */
export function detachOps(item: SceneItem): ToolOp[] {
  return isInstance(item) && !item.instance ? [{ type: "detach", id: item.node.id }] : [];
}

/* ------------------------------------------------------------------------------ main component edits */

function updateOp(id: ComponentId, c: Component, props: Component["props"], name = c.name): ToolOp {
  return {
    type: "updateComponent",
    id,
    name,
    props: { ...(props ?? {}) } as Record<string, PropDef>,
    variants: [...(c.variants ?? [])],
  };
}
export function setComponentNameOps(doc: Document | null, id: ComponentId, name: string): ToolOp[] {
  const c = components(doc)[id];
  const trimmed = name.trim();
  if (!c || !trimmed) return [];
  return [updateOp(id, c, c.props, trimmed)];
}
/** A prop's default, which every instance without its own value takes. The value must suit the prop's kind. */
export function setComponentPropOps(
  doc: Document | null,
  id: ComponentId,
  prop: string,
  value: Scalar,
): ToolOp[] {
  const c = components(doc)[id];
  const def = c?.props?.[prop];
  if (!c || !def) return [];
  const suits =
    def.type === "boolean"
      ? typeof value === "boolean"
      : def.type === "text"
        ? typeof value === "string"
        : typeof value === "number";
  if (!suits) return [];
  return [updateOp(id, c, { ...c.props, [prop]: { ...def, default: value } as PropDef })];
}
