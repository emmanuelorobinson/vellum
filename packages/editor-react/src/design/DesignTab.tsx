import {
  type DesignController,
  type Edit,
  type EditorSession,
  listVariables,
  type PaintEditor,
  type SectionId,
} from "@vellum/editor-core";
import type { Document, Value } from "@vellum/types-ts";
import type { ReactNode } from "react";
import { useStore } from "../hooks";
import { ThemeSection } from "../resources/ToolPanels";
import type { DesignContext, DesignHost } from "./context";
import {
  AppearanceSection,
  ExportSection,
  LayoutSection,
  NotesSection,
  PositionSection,
} from "./sections-basic";
import { ComponentSection } from "./sections-component";
import { EffectsSection, FillSection, StrokeSection } from "./sections-paint";
import { ContentSection, TextSection } from "./sections-text";

export interface DesignTabProps {
  design: DesignController;
  session: EditorSession;
  paintEditor: PaintEditor;
  /** The engine's current document, for variables, components and fonts. */
  document: Document | null;
  readOnly: boolean;
  host?: DesignHost;
}

const SECTIONS: Record<SectionId, (props: { ctx: DesignContext }) => ReactNode> = {
  notes: NotesSection,
  position: PositionSection,
  layout: LayoutSection,
  appearance: AppearanceSection,
  fill: FillSection,
  stroke: StrokeSection,
  effects: EffectsSection,
  text: TextSection,
  component: ComponentSection,
  content: ContentSection,
  export: ExportSection,
};

/**
 * The right panel's Design tab: the sections the selection has, in the design's order (C13, C14, C16, C33). It reads
 * the controller's model on every change and writes through the controller, one Person transaction per edit.
 */
export function DesignTab(props: DesignTabProps) {
  const { design, session, paintEditor, document: doc } = props;
  useStore(design.subscribe);
  useStore(paintEditor.subscribe);
  const model = design.model(doc);
  if (model.count === 0)
    return (
      <div className="vc-design">
        <p className="vc-design__empty">Select a layer to see its properties.</p>
      </div>
    );
  const mode = design.mode(doc);
  const ctx: DesignContext = {
    design,
    session,
    paintEditor,
    model,
    document: doc,
    density: model.sections[0] === "text" || model.sections[0] === "component" ? "roomy" : "dense",
    disabled: props.readOnly,
    host: props.host ?? {},
    numbers: listVariables(doc, "number", mode),
    colours: listVariables(doc, "color", mode),
    edit: (edit: Edit) => void design.edit(edit),
    colour: (value: Value<string>) => design.colour(value, doc),
  };
  return (
    <div className="vc-design" data-selection={model.count}>
      {props.host?.resources && doc ? (
        <ThemeSection design={design} session={session} document={doc} disabled={props.readOnly} />
      ) : null}
      {model.sections.map((id) => {
        const Section = SECTIONS[id];
        return <Section key={id} ctx={ctx} />;
      })}
    </div>
  );
}
