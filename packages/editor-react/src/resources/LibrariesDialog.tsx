import type { PasteChoices, ResourceController } from "@vellum/editor-core";
import type { Document } from "@vellum/types-ts";
import { useState } from "react";
import { useLocale } from "../accessibility/locale";
import { ResourceButton as Button } from "./Dialog";
export function LibrariesBody({
  resources: r,
  document: doc,
  disabled,
}: {
  resources: ResourceController;
  document: Document;
  disabled: boolean;
}) {
  const { t: translateUi } = useLocale();
  return (
    <>
      <section className="vc-resource-section">
        <h3>Imported</h3>
        {Object.entries(doc.imports ?? {}).map(([alias, path]) => {
          const info = r.libraryInfo(alias);
          return (
            <div key={alias} className="vc-library-row">
              <div>
                <strong>{info?.name ?? alias}</strong>
                <span
                  className={info?.state === "missing" ? "vc-resource-warning" : "vc-resource-hint"}
                >
                  {info?.state === "missing"
                    ? `⚠ Missing${info.cached ? " · using cached copy" : ""}`
                    : info?.updated
                      ? `Updated · ${info.updated}`
                      : "Status unavailable"}
                </span>
                {info?.state === "missing" ? <span className="vc-library-path">{path}</span> : null}
              </div>
              {info?.state === "missing" ? (
                <Button
                  disabled={disabled || !r.host.chooseLibrary}
                  onClick={() => void r.locate(alias)}
                >
                  Locate…
                </Button>
              ) : (
                <Button disabled={!r.host.openLibrary} onClick={() => r.host.openLibrary?.(alias)}>
                  Open
                </Button>
              )}
              <Button disabled={disabled} onClick={() => r.attempt(() => r.removeLibrary(alias))}>
                {translateUi("Remove")}
              </Button>
            </div>
          );
        })}
        <Button disabled={disabled || !r.host.chooseLibrary} onClick={() => void r.importLibrary()}>
          Import library…
        </Button>
      </section>
      <section className="vc-resource-section">
        <h3>Bundled design systems</h3>
        {r.host.bundled?.map((s) => (
          <div key={s.id} className="vc-library-row">
            <span>{s.name}</span>
            <Button
              disabled={disabled || !r.host.action}
              onClick={() => r.host.action?.("library.add", { id: s.id })}
            >
              Add to document
            </Button>
            <Button
              disabled={!r.host.action}
              onClick={() => r.host.action?.("library.guidelines", { id: s.id })}
            >
              Guidelines
            </Button>
          </div>
        )) ?? <p className="vc-resource-hint">No bundled libraries available.</p>}
      </section>
      <section className="vc-resource-section">
        <h3>This file</h3>
        <p className="vc-resource-hint">
          {doc.meta.kind === "library"
            ? "This file is a library."
            : `Turn ${doc.meta.name} into a reusable library. This cannot be undone.`}
        </p>
        <Button
          disabled={disabled || doc.meta.kind === "library"}
          onClick={() => r.attempt(() => r.requestConversion())}
        >
          Turn this file into a library
        </Button>
      </section>
    </>
  );
}
export function PasteBody({
  resources: r,
  disabled,
}: {
  resources: ResourceController;
  disabled: boolean;
}) {
  const { t: translateUi } = useLocale();
  const request = r.state.paste;
  const [link, setLink] = useState<boolean | null>(request?.libraryPath ? null : false);
  const [variables, setVariables] = useState<PasteChoices["variables"]>({});
  if (!request) return null;
  const omissions = request.omissions.length ? (
    <p className="vc-resource-warning">Not carried over: {request.omissions.join(", ")}.</p>
  ) : null;
  const paste = (choices: PasteChoices) => void r.paste(choices);
  if (link === null)
    return (
      <>
        <p className="vc-resource-description">
          These components come from a library this document has not imported.
        </p>
        <p className="vc-library-path">{request.libraryPath}</p>
        <p className="vc-resource-description">
          Link keeps future library updates connected. Copies become local components.
        </p>
        {omissions}
        <div className="vc-resource-actions">
          <Button
            primary
            disabled={disabled || !r.host.planPaste}
            onClick={() => {
              if (request.conflicts.length) setLink(true);
              else paste({ link: true, variables: {} });
            }}
          >
            Link to library
          </Button>
          <Button
            disabled={disabled || !r.host.planPaste}
            onClick={() => {
              if (request.conflicts.length) setLink(false);
              else paste({ link: false, variables: {} });
            }}
          >
            Paste as copies
          </Button>
          <Button onClick={() => r.close()}>{translateUi("Cancel")}</Button>
        </div>
      </>
    );
  return (
    <>
      <p className="vc-resource-description">Choose how to handle each name that already exists.</p>
      {request.conflicts.map((c) => (
        <section key={c.id} className="vc-resource-section">
          <h3>{c.name}</h3>
          <p className="vc-library-path">
            Existing {c.existing} · Pasted {c.pasted}
          </p>
          <div className="vc-resource-actions">
            <Button
              primary={variables[c.id] !== "renamed"}
              disabled={disabled || !c.sameType}
              onClick={() => setVariables((v) => ({ ...v, [c.id]: "existing" }))}
            >
              Use existing
            </Button>
            <Button
              primary={variables[c.id] === "renamed"}
              disabled={disabled}
              onClick={() => setVariables((v) => ({ ...v, [c.id]: "renamed" }))}
            >
              Add renamed
            </Button>
          </div>
        </section>
      ))}
      {omissions}
      <p className="vc-resource-hint">Renamed variables use a new name, such as color/primary-2.</p>
      <Button
        primary
        disabled={disabled || !r.host.planPaste || request.conflicts.some((c) => !variables[c.id])}
        onClick={() => paste({ link, variables })}
      >
        Paste
      </Button>
    </>
  );
}
