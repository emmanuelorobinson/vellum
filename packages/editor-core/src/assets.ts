import type { ComponentId, DeviceId, Node, NodeId, Point } from "@vellum/types-ts";
import type { Place, ToolTransaction } from "./tool-types";

export type AssetSectionId = "document" | "libraries" | "icons" | "styles" | "systems";
export type LibraryState = "synced" | "missing";

export interface AssetsInput {
  components: Readonly<Record<ComponentId, { name: string }>>;
  libraries: readonly { alias: string; state: LibraryState; components: number }[];
  iconSets: readonly { id: string; label: string }[];
  projectIconFolder: { path: string } | null;
  visualStyles: readonly { id: string; name: string }[];
  designSystems: readonly { id: string; name: string }[];
  query: string;
}

export type AssetPayload =
  | { kind: "component"; componentId: ComponentId; library: string | null }
  | { kind: "library"; alias: string }
  | { kind: "iconSets"; sets: readonly string[] }
  | { kind: "iconFolder"; path: string }
  | { kind: "styles"; styles: readonly string[] }
  | { kind: "system"; id: string };

export interface AssetItem {
  id: string;
  label: string;
  /** A Lucide icon name. */
  icon: string;
  /** Shown after the label, joined with a middle dot. */
  detail: string | null;
  status: "ok" | "missing" | null;
  action: "add" | "browse" | null;
  payload: AssetPayload;
  /** What the search reads besides the label and the detail. */
  keywords: string;
}
export interface AssetSection {
  id: AssetSectionId;
  heading: string;
  items: AssetItem[];
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
const title = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export function assetSections(input: AssetsInput): AssetSection[] {
  const sections: AssetSection[] = [];
  const components = Object.entries(input.components).map(
    ([componentId, c]): AssetItem => ({
      id: `component:${componentId}`,
      label: c.name,
      icon: "component",
      detail: null,
      status: null,
      action: null,
      payload: { kind: "component", componentId, library: null },
      keywords: "",
    }),
  );
  sections.push({ id: "document", heading: "This document", items: components });
  sections.push({
    id: "libraries",
    heading: "Libraries",
    items: input.libraries.map(
      (l): AssetItem => ({
        id: `library:${l.alias}`,
        label: l.alias,
        icon: "palette",
        detail: `${title(l.state)} · ${plural(l.components, "component")}`,
        status: l.state === "missing" ? "missing" : "ok",
        action: null,
        payload: { kind: "library", alias: l.alias },
        keywords: "",
      }),
    ),
  });
  const icons: AssetItem[] = [];
  if (input.iconSets.length)
    icons.push({
      id: "icons:sets",
      label: input.iconSets.map((s) => s.label).join(" · "),
      icon: "image",
      detail: null,
      status: null,
      action: null,
      payload: { kind: "iconSets", sets: input.iconSets.map((s) => s.id) },
      keywords: "",
    });
  if (input.projectIconFolder)
    icons.push({
      id: "icons:folder",
      label: "Your SVG folder",
      icon: "image",
      detail: null,
      status: null,
      action: "browse",
      payload: { kind: "iconFolder", path: input.projectIconFolder.path },
      keywords: `Browse ${input.projectIconFolder.path}`,
    });
  sections.push({ id: "icons", heading: "Icons", items: icons });
  sections.push({
    id: "styles",
    heading: "Styles",
    items: input.visualStyles.length
      ? [
          {
            id: "styles:visual",
            label: input.visualStyles.map((s) => s.name).join(" · "),
            icon: "palette",
            detail: null,
            status: null,
            action: null,
            payload: { kind: "styles", styles: input.visualStyles.map((s) => s.id) },
            keywords: "",
          },
        ]
      : [],
  });
  sections.push({
    id: "systems",
    heading: "Starter design systems",
    items: input.designSystems.map(
      (d): AssetItem => ({
        id: `system:${d.id}`,
        label: d.name,
        icon: "palette",
        detail: null,
        status: null,
        action: "add",
        payload: { kind: "system", id: d.id },
        keywords: "Add Guidelines",
      }),
    ),
  });
  const words = input.query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  return sections
    .map((s) => ({
      ...s,
      items: words.length
        ? s.items.filter((i) => {
            const hay = `${i.label} ${i.detail ?? ""} ${i.keywords}`.toLocaleLowerCase();
            return words.every((w) => hay.includes(w));
          })
        : s.items,
    }))
    .filter((s) => s.items.length > 0);
}

const sizeOf = (s: unknown): number | "hug" | "fill" =>
  typeof s === "number" ? s : s === "fill" ? "fill" : "hug";

/** Dropping a component from Assets onto the canvas: one instance, at the drop point, at the component's size. */
export function instanceTransaction(args: {
  device: DeviceId;
  componentId: ComponentId;
  library: string | null;
  name: string;
  root: Node;
  id: NodeId;
  at: Point;
  to: Place;
}): ToolTransaction {
  const r = args.root as unknown as { width?: unknown; height?: unknown };
  const width = sizeOf(r.width);
  const height = sizeOf(r.height);
  const node = {
    type: "instance",
    id: args.id,
    name: args.name,
    componentId: args.componentId,
    ...(args.library ? { library: args.library } : {}),
    width: width === "fill" ? "hug" : width,
    height: height === "fill" ? "hug" : height,
    position: { type: "absolute", x: args.at.x, y: args.at.y },
  } as unknown as Node;
  return {
    origin: { actor: { kind: "person", device: args.device } },
    ops: [{ type: "insert", to: args.to, node }],
  };
}
