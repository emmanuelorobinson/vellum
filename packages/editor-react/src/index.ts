/** Editor UI built on shadcn and Base UI over editor-core. The visual harness is at ./harness. */
export const PACKAGE_NAME = "@vellum/editor-react";

export type { ComposerHost } from "./agent/Composer";
export {
  ConnectAgentsBody,
  ConnectAgentsDialog,
  type ConnectAgentsProps,
  type ConnectClient,
} from "./agent/ConnectAgentsDialog";
export { EditorWorkspace, type EditorWorkspaceProps } from "./EditorWorkspace";
export { ResourceDialogs } from "./resources/ResourceDialogs";
