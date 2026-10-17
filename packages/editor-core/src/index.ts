/** Framework-agnostic editor controller. No renderer, DOM, React or document mutation. */
export const PACKAGE_NAME = "@vellum/editor-core";
export { createCommandController } from "./command-controller";
export * from "./keybindings";
export * from "./registries";
export * from "./tool-types";
export { initialToolState, transitionTool } from "./tools";
