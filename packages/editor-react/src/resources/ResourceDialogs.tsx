import type { DesignController } from "@vellum/editor-core";
import { type EditorSession, fontPatch, type ResourceController } from "@vellum/editor-core";
import { useLocale } from "../accessibility/locale";
import { useStore } from "../hooks";
import { ResourceButton as Button, ResourceDialog as Dialog } from "./Dialog";
import { FontPickerBody, ManageFontsBody } from "./FontsDialog";
import { LibrariesBody, PasteBody } from "./LibrariesDialog";
import { VariablesBody } from "./VariablesDialog";
export function ResourceDialogs({
  resources: r,
  design,
  session,
  readOnly,
}: {
  resources: ResourceController;
  design?: DesignController | undefined;
  session: EditorSession;
  readOnly: boolean;
}) {
  const { t: translateUi } = useLocale();
  useStore(r.subscribe);
  const doc = r.document();
  if (!doc) return null;
  const dialog = r.state.dialog;
  const disabled = readOnly || r.state.busy;
  const titles = {
    variables: "Variables",
    themes: "Variables · Theme axes",
    libraries: "Libraries",
    fonts: "Choose a font",
    manageFonts: "Manage fonts",
    convert: "Turn this file into a library?",
    paste: r.state.paste?.libraryPath ? "Link pasted components?" : "Resolve variable names",
  };
  const descriptions: Partial<Record<NonNullable<typeof dialog>, string>> = {
    variables: `${doc.meta.name} · modes are columns; aliases retain their bindings.`,
    themes: "A value resolves across every axis. Children inherit unless they set the same axis.",
    libraries: "Shared components stay linked to the library file in your repo.",
    fonts: "Custom fonts are stored with this file, so they travel with it.",
  };
  const widths = {
    variables: 1032,
    themes: 960,
    libraries: 660,
    fonts: 520,
    manageFonts: 620,
    convert: 600,
    paste: r.state.paste?.libraryPath ? 620 : 760,
  };
  return (
    <>
      {dialog ? (
        <Dialog
          key={dialog}
          title={titles[dialog]}
          {...(descriptions[dialog] ? { description: descriptions[dialog] } : {})}
          width={widths[dialog]}
          onClose={() => r.close()}
          className={dialog === "variables" ? "vc-resource-dialog--variables" : ""}
        >
          {dialog === "variables" || dialog === "themes" ? (
            <VariablesBody
              resources={r}
              document={doc}
              disabled={disabled}
              axes={dialog === "themes"}
            />
          ) : null}
          {dialog === "libraries" ? (
            <LibrariesBody resources={r} document={doc} disabled={disabled} />
          ) : null}
          {dialog === "fonts" ? (
            <FontPickerBody
              resources={r}
              document={doc}
              disabled={disabled || !design}
              onChoose={(font) => {
                const mode = session.state.mode;
                if (mode.type === "editing" && mode.node.type === "text") {
                  const patch = fontPatch(
                    {
                      node: mode.node,
                      bounds: { x: 0, y: 0, width: 0, height: 0 },
                      children: [],
                      parent: null,
                    },
                    font,
                  );
                  if (patch?.content) {
                    session.dispatch({
                      type: "content",
                      content: patch.content as import("@vellum/types-ts").TextRun[],
                    });
                    r.close();
                  }
                  return;
                }
                const result = design?.edit((target) => fontPatch(target.item, font));
                if (result?.status === "applied") r.close();
                else if (result?.status === "rejected")
                  r.attempt(() => {
                    throw Error(`Change refused: ${result.reason}`);
                  });
              }}
            />
          ) : null}
          {dialog === "manageFonts" ? (
            <ManageFontsBody resources={r} document={doc} disabled={disabled} />
          ) : null}
          {dialog === "paste" ? <PasteBody resources={r} disabled={disabled} /> : null}
          {dialog === "convert" ? (
            <>
              <p className="vc-resource-description">
                {doc.meta.name} will become a reusable library. This action cannot be undone.
              </p>
              <div className="vc-resource-actions">
                <Button onClick={() => r.open("libraries")}>{translateUi("Cancel")}</Button>
                <Button
                  primary
                  disabled={disabled}
                  onClick={() => r.attempt(() => r.confirmConversion())}
                >
                  Turn into library
                </Button>
              </div>
            </>
          ) : null}
          {r.state.busy ? <p role="status">Working…</p> : null}
          {r.state.error ? (
            <p role="alert" className="vc-resource-error">
              × {r.state.error}
            </p>
          ) : null}
        </Dialog>
      ) : null}
      {r.state.saveSuggested ? (
        <div className="vc-resource-save" role="status">
          <span>Library located · Save the document to keep its new path.</span>
          <Button disabled={!r.host.save || r.state.busy || readOnly} onClick={() => void r.save()}>
            Save document
          </Button>
        </div>
      ) : null}
    </>
  );
}
