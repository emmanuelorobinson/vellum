import type {
  AuthoringController,
  DesignController,
  DesignModel,
  Edit,
  EditorSession,
  PaintEditor,
  ResourceController,
  VariableInfo,
} from "@vellum/editor-core";
import type { Document, Value } from "@vellum/types-ts";

/** What the host (the app around the editor) can do that the panel cannot do itself. Absent means disabled. */
export interface DesignHost {
  authoring?: AuthoringController;
  resources?: ResourceController;
  /** Font families that can be drawn here, for the font picker and the missing-font banner. */
  fonts?: readonly string[];
  /** A main component lives elsewhere on a page or in a library: the host shows it. */
  onGoToMain?(target: { componentId: string; library: string | null }): void;
  /** Commands the panel cannot carry out: "export.add", "image.choose", "image.replace", "shader.examples". */
  onAction?(command: string, detail?: Record<string, unknown>): void;
}

export interface DesignContext {
  design: DesignController;
  session: EditorSession;
  paintEditor: PaintEditor;
  model: DesignModel;
  document: Document | null;
  /** Nothing can be changed: the file is read-only. */
  disabled: boolean;
  host: DesignHost;
  /** Spacing: the design uses 8/6 for frames, 12/8 for text and components, and wide rows for paint, stroke and effects detail. */
  density: "dense" | "roomy";
  numbers: readonly VariableInfo[];
  colours: readonly VariableInfo[];
  /** Applies a change to every selected layer it fits. */
  edit(edit: Edit): void;
  /** Reads a colour, resolving a variable under the current theme. */
  colour(value: Value<string>): string;
}
