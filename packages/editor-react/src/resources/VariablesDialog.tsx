import {
  addThemeAxis,
  addThemeValue,
  type ResourceController,
  themeAxes,
  themeColumns,
  variableCollections,
  variableEntry,
} from "@vellum/editor-core";
import type { Document, VariableType } from "@vellum/types-ts";
import { useState } from "react";
import { useLocale } from "../accessibility/locale";
import { ResourceButton as Button } from "./Dialog";
export function VariablesBody({
  resources: r,
  document: doc,
  disabled,
  axes = false,
}: {
  resources: ResourceController;
  document: Document;
  disabled: boolean;
  axes?: boolean;
}) {
  const { t: translateUi } = useLocale();
  const [collection, setCollection] = useState<string | null>(null);
  const [add, setAdd] = useState<"variable" | "axis" | string | null>(null);
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const [type, setType] = useState<VariableType>("number");
  const collections = variableCollections(doc);
  let columns: ReturnType<typeof themeColumns>;
  try {
    columns = themeColumns(doc);
  } catch (e) {
    return <p role="alert">{(e as Error).message}</p>;
  }
  const variables = Object.entries(doc.variables ?? {}).filter(
    ([, v]) => collection === null || v.name.split("/")[0] === collection,
  );
  const begin = (kind: string) => {
    setAdd(kind);
    setName("");
    setValue("");
  };
  const submit = () =>
    r.attempt(() => {
      if (add === "variable") r.createVariable(name, type, value);
      else if (add === "axis") r.setThemes(addThemeAxis(doc, name, value));
      else if (add) r.setThemes(addThemeValue(doc, add, name));
      setAdd(null);
    });
  const toolbar = (
    <div className="vc-resource-actions">
      {axes ? (
        themeAxes(doc).map(([axis, values]) => (
          <Button key={axis} disabled={disabled} onClick={() => begin(axis)}>
            {axis}: {values.join(", ")}
          </Button>
        ))
      ) : (
        <>
          <Button primary disabled={disabled} onClick={() => begin("variable")}>
            New variable
          </Button>
          <Button disabled={disabled} onClick={() => begin(themeAxes(doc)[0]?.[0] ?? "axis")}>
            New mode
          </Button>
          <Button
            disabled={disabled || !r.host.action}
            onClick={() => r.host.action?.("tokens.import")}
          >
            Import from CSS
          </Button>
          <Button disabled={!r.host.action} onClick={() => r.host.action?.("tokens.export")}>
            Export
          </Button>
        </>
      )}
      {axes ? (
        <Button disabled={disabled} onClick={() => begin("axis")}>
          + Add axis
        </Button>
      ) : null}
    </div>
  );
  return (
    <>
      <div
        className={
          axes ? "vc-variable-workspace vc-variable-workspace--axes" : "vc-variable-workspace"
        }
      >
        {!axes ? (
          <nav aria-label="Variable collections" className="vc-variable-collections">
            <Button primary={collection === null} onClick={() => setCollection(null)}>
              All
            </Button>
            {[...new Set(["color", "space", "radius", "type", ...collections])].map((c) => (
              <Button key={c} primary={collection === c} onClick={() => setCollection(c)}>
                {c.charAt(0).toUpperCase() + c.slice(1)}
              </Button>
            ))}
          </nav>
        ) : null}
        <div className="vc-variable-main">
          {toolbar}
          {add ? (
            <form
              className="vc-resource-form"
              onSubmit={(e) => {
                e.preventDefault();
                submit();
              }}
            >
              <label>
                {add === "variable"
                  ? "Variable name"
                  : add === "axis"
                    ? "Axis name"
                    : `${add} value`}
                <input
                  aria-label="New name"
                  value={name}
                  disabled={disabled}
                  onChange={(e) => setName(e.target.value)}
                />
              </label>
              {add === "variable" ? (
                <label>
                  Type
                  <select
                    aria-label="Variable type"
                    value={type}
                    onChange={(e) => setType(e.target.value as VariableType)}
                  >
                    {["color", "number", "string", "boolean"].map((t) => (
                      <option key={t}>{t}</option>
                    ))}
                  </select>
                </label>
              ) : null}
              {add === "variable" || add === "axis" ? (
                <label>
                  {add === "axis" ? "First value" : "Value"}
                  <input
                    aria-label="New value"
                    value={value}
                    disabled={disabled}
                    onChange={(e) => setValue(e.target.value)}
                  />
                </label>
              ) : null}
              <button type="submit" className="vc-resource-button" disabled={disabled}>
                Add
              </button>
              <Button onClick={() => setAdd(null)}>{translateUi("Cancel")}</Button>
            </form>
          ) : null}
          <div className="vc-variable-scroll">
            <table className="vc-variable-table" data-axes={axes ? "" : undefined}>
              <thead>
                <tr>
                  <th>Variable</th>
                  {columns.map((c) => (
                    <th key={c.label} scope="col">
                      {c.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {variables.map(([id, v]) => (
                  <tr key={id}>
                    <th scope="row">
                      <input
                        className="vc-variable-name"
                        aria-label={`Variable name ${v.name}`}
                        defaultValue={v.name}
                        disabled={disabled}
                        onBlur={(e) => {
                          if (e.currentTarget.value !== v.name)
                            r.attempt(() => r.renameVariable(id, e.currentTarget.value));
                        }}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") e.currentTarget.blur();
                          if (e.key === "Escape") {
                            e.stopPropagation();
                            e.currentTarget.value = v.name;
                            e.currentTarget.blur();
                          }
                        }}
                      />
                    </th>
                    {columns.map((c) => {
                      const entry = variableEntry(doc, id, c.mode);
                      const text = entry?.$alias
                        ? `$${entry.$alias}`
                        : entry?.value === undefined
                          ? ""
                          : String(entry.value);
                      return (
                        <td key={c.label}>
                          {!axes && v.type === "color" && entry?.value ? (
                            <span
                              className="vc-variable-swatch"
                              style={{ background: String(entry.value) }}
                            />
                          ) : null}
                          <VariableCell
                            key={`${id}:${c.label}:${text}`}
                            label={`${v.name} · ${c.label}`}
                            text={text}
                            alias={
                              entry?.$alias
                                ? (doc.variables?.[entry.$alias]?.name ?? "Missing variable")
                                : null
                            }
                            disabled={disabled}
                            onCommit={(text) => r.attempt(() => r.editVariable(id, c.mode, text))}
                          />
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!variables.length ? (
            <p className="vc-resource-hint">
              No variables in this collection. Add a variable to begin.
            </p>
          ) : null}
        </div>
      </div>
      <div className="vc-resource-actions">
        {axes
          ? Object.keys(doc.themes ?? {})
              .reverse()
              .map((axis) => (
                <Button key={axis} disabled={disabled} onClick={() => begin(axis)}>
                  + {axis} value
                </Button>
              ))
          : null}
        <Button
          disabled={!r.host.action}
          onClick={() =>
            r.host.action?.("agent.variables", {
              variables: variables.map(([id]) => id),
            })
          }
        >
          Ask agent
        </Button>
      </div>
    </>
  );
}
function VariableCell({
  label,
  text,
  alias,
  disabled,
  onCommit,
}: {
  label: string;
  text: string;
  alias: string | null;
  disabled: boolean;
  onCommit(v: string): void;
}) {
  const [draft, setDraft] = useState(text);
  const [editing, setEditing] = useState(false);
  return (
    <span className="vc-variable-cell">
      {alias && !editing ? (
        <span className="vc-variable-alias" aria-hidden>
          → {alias}
        </span>
      ) : null}
      <input
        className="vc-variable-input"
        style={{ opacity: alias && !editing ? 0 : 1 }}
        aria-label={label}
        disabled={disabled}
        value={draft}
        title="Enter a value or $variable-id for an alias"
        onFocus={() => setEditing(true)}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => {
          setEditing(false);
          if (draft !== text) onCommit(draft);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.currentTarget.blur();
          }
          if (e.key === "Escape") {
            e.stopPropagation();
            setDraft(text);
            setEditing(false);
          }
        }}
      />
    </span>
  );
}
