import { clone } from "./clone";
import type { ToolScene } from "./model";
/** A retained facade follows the binding's current Scene; returned records are never storage handles. */
export function sceneView(current: () => ToolScene): ToolScene {
  return Object.freeze({
    get: (...args: Parameters<ToolScene["get"]>) => clone(current().get(...args)),
    roots: () => clone(current().roots()),
    hitPoint: (...args: Parameters<ToolScene["hitPoint"]>) => current().hitPoint(...clone(args)),
    hitRect: (...args: Parameters<ToolScene["hitRect"]>) =>
      clone(current().hitRect(...clone(args))),
    snap: (...args: Parameters<ToolScene["snap"]>) => clone(current().snap(...clone(args))),
    toParent: (...args: Parameters<ToolScene["toParent"]>) =>
      clone(current().toParent(...clone(args))),
    toLocal: (...args: Parameters<ToolScene["toLocal"]>) =>
      clone(current().toLocal(...clone(args))),
    hitPath: (address: string, point: import("@vellum/types-ts").Point, tolerance: number) =>
      clone(current().hitPath?.(address, clone(point), tolerance) ?? null),
    path: (...args: Parameters<ToolScene["path"]>) => clone(current().path(...args)),
  });
}
