/** Framework-agnostic editor controller. No renderer, DOM, React or document mutation. */
export const PACKAGE_NAME = "@vellum/editor-core";
export * from "./tool-types";
export { initialToolState, transitionTool } from "./tools";
