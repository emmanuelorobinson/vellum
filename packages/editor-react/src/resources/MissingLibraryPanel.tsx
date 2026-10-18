import type { ResourceController } from "@vellum/editor-core";
import { Section } from "../design/controls";
import { ResourceButton as Button } from "./Dialog";
export function MissingLibraryPanel({
  resources: r,
  alias,
  disabled,
}: {
  resources: ResourceController;
  alias: string;
  disabled: boolean;
}) {
  const info = r.libraryInfo(alias);
  const path = r.document()?.imports?.[alias];
  if (!path || info?.state !== "missing") return null;
  return (
    <Section id="missing-library" title="Libraries" className="vc-missing-library">
      <strong className="vc-resource-warning">{info.name} · Missing</strong>
      <p className="vc-library-path">{path}</p>
      <p>
        {info.cached
          ? "Instances keep rendering from their cached copy."
          : "Cached copy unavailable."}
      </p>
      <Button
        primary
        disabled={disabled || !r.host.chooseLibrary || r.state.busy}
        onClick={() => void r.locate(alias)}
      >
        Locate…
      </Button>
      {info.cached ? (
        <span className="vc-resource-warning vc-missing-library__badge">Cached instance</span>
      ) : null}
      <p>After locating, save this document to retain the updated path.</p>
      {r.state.error ? (
        <p role="alert" className="vc-resource-error">
          × {r.state.error}
        </p>
      ) : null}
    </Section>
  );
}
