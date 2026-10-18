import {
  type AuthoringController,
  documentNodes,
  listVariables,
  parseScriptInput,
  type ScriptInput,
  type ScriptView,
} from "@vellum/editor-core";
import type { Node, Scalar, Value } from "@vellum/types-ts";
import { useId, useState } from "react";
import type { DesignContext } from "../design/context";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";

export function ScriptDiagnostic({
  view,
  src,
  canvas = false,
}: {
  view: ScriptView;
  src: string;
  canvas?: boolean;
}) {
  if (view.status !== "error") return null;
  return (
    <div className="vx-script-error" role={canvas ? "status" : "alert"}>
      {canvas ? <Icon name="triangle-alert" size={24} /> : null}
      {canvas ? <h2>Script couldn't run</h2> : null}
      <p>
        {!canvas ? <Icon name="triangle-alert" size={12} /> : null}
        {view.message}
      </p>
      <p className={`vc-mono ${canvas ? "" : "vx-script-location"}`}>
        {src.split("/").at(-1)}
        {view.line ? ` · line ${view.line}` : ""}
      </p>
      {!canvas && view.excerpt ? <pre>{view.excerpt}</pre> : null}
    </div>
  );
}
function InputControl({
  ctx,
  input,
  node,
  address,
  authoring,
}: {
  ctx: DesignContext;
  input: ScriptInput;
  node: Extract<Node, { type: "script" }>;
  address: string;
  authoring: AuthoringController;
}) {
  const controlId = useId();
  const raw = node.inputs?.[input.name] ?? input.default ?? (input.kind === "boolean" ? false : "");
  const bound = typeof raw === "object" && raw !== null && "$var" in raw;
  const [error, setError] = useState<string | null>(null);
  const vars = listVariables(
    ctx.document,
    input.kind === "number"
      ? "number"
      : input.kind === "boolean"
        ? "boolean"
        : input.kind === "color"
          ? "color"
          : "string",
    ctx.design.mode(ctx.document),
  );
  const refs =
    input.kind === "ref" && ctx.document
      ? input.refKind === "page"
        ? ctx.document.pages.map((p) => ({ id: p.id, name: p.name }))
        : input.refKind === "component"
          ? Object.entries(ctx.document.components ?? {}).map(([id, c]) => ({ id, name: c.name }))
          : input.refKind === "variable"
            ? Object.entries(ctx.document.variables ?? {}).map(([id, v]) => ({ id, name: v.name }))
            : documentNodes(ctx.document).map((n) => ({ id: n.id, name: n.name }))
      : [];
  const value = bound ? (vars.find((v) => v.id === raw.$var)?.value ?? "") : raw;
  const apply = (v: Value<Scalar>) => authoring.setInput(address, input, v);
  const commit = (value: string | boolean) => {
    try {
      apply(parseScriptInput(input, value));
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const label = input.label ?? input.name.charAt(0).toUpperCase() + input.name.slice(1);
  return (
    <div className="vx-input" data-kind={input.kind} data-bound={bound ? "" : undefined}>
      <label htmlFor={controlId}>
        {label}
        {input.kind === "boolean" ? (
          <input
            id={controlId}
            type="checkbox"
            aria-label={label}
            checked={value === true}
            disabled={ctx.disabled}
            onChange={(e) => commit(e.target.checked)}
          />
        ) : input.kind === "enum" ? (
          <select
            id={controlId}
            aria-label={label}
            value={String(value)}
            disabled={ctx.disabled}
            onChange={(e) => commit(e.target.value)}
          >
            {input.options?.map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        ) : input.kind === "string" && input.multiline ? (
          <textarea
            id={controlId}
            key={JSON.stringify(raw)}
            aria-label={label}
            defaultValue={String(value)}
            disabled={ctx.disabled}
            onBlur={(e) => {
              if (e.target.value !== String(value)) commit(e.target.value);
            }}
          />
        ) : (
          <input
            key={JSON.stringify(raw)}
            id={controlId}
            aria-label={label}
            defaultValue={String(value)}
            disabled={ctx.disabled}
            type={input.kind === "number" ? "number" : "text"}
            list={input.kind === "ref" ? `${controlId}-refs` : undefined}
            min={input.min}
            max={input.max}
            step={input.step ?? "any"}
            onBlur={(e) => {
              if (e.target.value !== String(value)) commit(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
          />
        )}
      </label>
      {input.kind === "ref" ? (
        <datalist id={`${controlId}-refs`}>
          {refs.map((r) => (
            <option key={r.id} value={r.id}>
              {r.name}
            </option>
          ))}
        </datalist>
      ) : null}
      {vars.length || bound ? (
        <select
          className="vx-binding"
          aria-label={`Bind ${label}`}
          disabled={ctx.disabled}
          value={bound ? raw.$var : ""}
          onChange={(e) => {
            if (e.target.value) apply({ $var: e.target.value });
            else commit(typeof value === "boolean" ? value : String(value));
          }}
        >
          <option value="">Literal value</option>
          {bound && !vars.some((v) => v.id === raw.$var) ? (
            <option value={raw.$var}>Missing variable · {raw.$var}</option>
          ) : null}
          {vars.map((v) => (
            <option key={v.id} value={v.id}>{`{${v.name}} ${String(v.value ?? "")}`}</option>
          ))}
        </select>
      ) : null}
      {error ? (
        <p role="alert" className="vx-error">
          {error}
        </p>
      ) : null}
    </div>
  );
}
const NO_STORE = () => ({ dispose() {} });
export function ScriptSection({ ctx }: { ctx: DesignContext }) {
  const a = ctx.host.authoring;
  useStore(a?.subscribe ?? NO_STORE);
  const item = ctx.design.single()?.item;
  if (item?.node.type !== "script") return null;
  const node = item.node,
    address = ctx.session.state.selection[0] as string;
  const view = a?.view(address, node) ?? { status: "unavailable" as const, inputs: [] };
  return (
    <section className="vx-section" data-section="script" aria-label="Script">
      <h3>Script</h3>
      <p className="vc-mono vx-script-path">{node.src || "No script selected"}</p>
      <button
        type="button"
        className="vx-button"
        disabled={!a?.host.openScript || !node.src}
        onClick={() => a?.host.openScript?.(node.src)}
      >
        Open in editor
      </button>
      <ScriptDiagnostic view={view} src={node.src} />
      {view.status === "empty" ? (
        <p className="vx-muted">Inputs appear here after you choose a script file.</p>
      ) : view.status === "unavailable" ? (
        <p className="vx-muted" role="status">
          Script runtime unavailable
        </p>
      ) : view.status === "loading" ? (
        <p className="vx-muted" role="status">
          ◌ Running script…
        </p>
      ) : null}
      {a
        ? view.inputs.map((input) => (
            <InputControl
              key={`${address}/${input.name}`}
              ctx={ctx}
              input={input}
              node={node}
              address={address}
              authoring={a}
            />
          ))
        : null}
      {view.status === "ready" ? (
        <button
          type="button"
          className="vx-button"
          data-primary=""
          disabled={ctx.disabled || !a || !!item.instance}
          title={item.instance ? "Detach the instance before converting this script" : undefined}
          onClick={() => a?.convertScript(address)}
        >
          Convert to layers
        </button>
      ) : null}
      {view.status === "error" ? (
        <button
          type="button"
          className="vx-button"
          data-primary=""
          disabled={ctx.disabled || !a?.host.askScript}
          onClick={() => a?.host.askScript?.(address, "fix")}
        >
          Ask agent to fix
        </button>
      ) : null}
    </section>
  );
}
