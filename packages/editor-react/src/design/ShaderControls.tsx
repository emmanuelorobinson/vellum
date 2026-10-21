import { Dialog } from "@base-ui/react/dialog";
import {
  isVarRef,
  listVariables,
  parseUniform,
  resolveVariable,
  type ShaderUniform,
  type ShaderView,
} from "@vellum/editor-core";
import type { ShaderEffect, ShaderPaint, Uniform, Value } from "@vellum/types-ts";
import { useEffect, useState } from "react";
import catalog from "../../../../examples/catalog.json";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import type { DesignContext } from "./context";
import { PropertyRow, TextButton, TextField, ValueField } from "./controls";
import { SPEC } from "./specs";

export function ShaderError({ view }: { view: ShaderView }) {
  if (view.status !== "error") return null;
  return (
    <div className="vc-shader-error" role="alert" data-kind={view.kind}>
      <Icon name="triangle-alert" size={16} />
      <span>Shader couldn't run · {view.message}</span>
    </div>
  );
}
function UniformControl({
  ctx,
  info,
  value,
  commit,
}: {
  ctx: DesignContext;
  info: ShaderUniform;
  value: Value<Uniform> | undefined;
  commit(v: Value<Uniform>): void;
}) {
  const [error, setError] = useState<string | null>(null);
  const mode = ctx.design.mode(ctx.document);
  const bound = isVarRef(value);
  const resolved = bound
    ? resolveVariable(ctx.document, value.$var, mode)
    : (value ?? info.default);
  const label = info.label ?? info.name.charAt(0).toUpperCase() + info.name.slice(1);
  const vars =
    info.kind === "vector"
      ? []
      : listVariables(
          ctx.document,
          info.kind === "color" ? "color" : info.kind === "boolean" ? "boolean" : "number",
          mode,
        );
  const apply = (raw: string | boolean) => {
    try {
      commit(parseUniform(info, raw));
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <div className="vc-shader-uniform">
      <PropertyRow label={label}>
        {info.kind === "number" ? (
          <ValueField
            label={label}
            value={
              bound
                ? {
                    kind: "bound",
                    variable: {
                      id: value.$var,
                      name: vars.find((v) => v.id === value.$var)?.name ?? null,
                      value: resolved,
                    },
                  }
                : typeof resolved === "number"
                  ? { kind: "value", value: resolved }
                  : { kind: "none" }
            }
            spec={SPEC.bare}
            disabled={ctx.disabled}
            onCommit={(v) => apply(String(v))}
          />
        ) : info.kind === "boolean" ? (
          <input
            type="checkbox"
            aria-label={label}
            checked={resolved === true}
            disabled={ctx.disabled}
            onChange={(e) => apply(e.target.checked)}
          />
        ) : (
          <TextField
            label={label}
            value={Array.isArray(resolved) ? resolved.join(", ") : String(resolved ?? "")}
            disabled={ctx.disabled}
            onCommit={(raw) => {
              try {
                commit(parseUniform(info, raw));
                return null;
              } catch (e) {
                return (e as Error).message;
              }
            }}
          />
        )}
      </PropertyRow>
      {info.kind !== "vector" ? (
        <select
          className="vx-binding"
          aria-label={`Bind ${label}`}
          value={bound ? value.$var : ""}
          disabled={ctx.disabled}
          onChange={(e) => {
            if (e.target.value) commit({ $var: e.target.value });
            else if (resolved !== undefined) commit(resolved as Uniform);
          }}
        >
          <option value="">Literal value</option>
          {bound && !vars.some((v) => v.id === value.$var) ? (
            <option value={value.$var}>Missing variable</option>
          ) : null}
          {vars.map((v) => (
            <option key={v.id} value={v.id}>{`{${v.name}}`}</option>
          ))}
        </select>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
    </div>
  );
}
export function ShaderControls({
  ctx,
  shader,
  apply,
  effect = false,
}: {
  ctx: DesignContext;
  shader: ShaderPaint | ShaderEffect | null;
  apply(v: ShaderPaint | ShaderEffect): void;
  effect?: boolean;
}) {
  const controller = ctx.host.shaders;
  useStore(controller?.subscribe ?? emptySubscribe);
  const container = usePortalContainer();
  const [busy, setBusy] = useState(false),
    [message, setMessage] = useState<string | null>(null);
  const src = shader?.src ?? "";
  const [category, setCategory] = useState("Shaders");
  useEffect(() => {
    if (src) void controller?.refresh(src);
  }, [controller, src]);
  const view = controller?.view(src) ?? { status: "unavailable" as const };
  const selection = () =>
    ctx.design
      .targets()
      .map((t) => t.item.node.id)
      .join("/");
  const choose = async (id?: string) => {
    const before = selection();
    const beforeNode = ctx.design.single()?.item.node;
    setBusy(true);
    setMessage(null);
    try {
      const choice = id
        ? await controller?.host.importExample?.(id)
        : await controller?.host.choose?.(effect);
      if (
        choice &&
        selection() === before &&
        ctx.design.single()?.item.node === beforeNode &&
        !ctx.disabled
      )
        apply({ ...choice, visible: true });
    } catch {
      setMessage("The shader could not be opened");
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="vc-shader-controls" aria-label={effect ? "Shader effect" : "Shader fill"}>
      <h4 className="vc-shader-heading">{effect ? "Shader effect" : "Shader fill"}</h4>
      <PropertyRow label="File">
        <span className="vc-field vc-mono-value" data-height="28">
          {src || "No shader file"}
        </span>
      </PropertyRow>
      <TextButton
        disabled={ctx.disabled || busy || !controller?.host.choose}
        onClick={() => void choose()}
      >
        Choose shader file…
      </TextButton>
      {view.status === "ready" ? (
        view.uniforms.map((info) => (
          <UniformControl
            key={info.name}
            ctx={ctx}
            info={info}
            value={shader?.uniforms?.[info.name]}
            commit={(value) => {
              if (shader)
                apply({ ...shader, uniforms: { ...shader.uniforms, [info.name]: value } });
            }}
          />
        ))
      ) : src || view.status === "unavailable" ? (
        <p className="vc-paint__hint">
          {view.status === "loading"
            ? "Compiling shader…"
            : view.status === "unavailable"
              ? "Shader previews are unavailable in this editor"
              : ""}
        </p>
      ) : null}
      <ShaderError view={view} />
      {message ? <p role="alert">{message}</p> : null}
      <Dialog.Root>
        <Dialog.Trigger className="vc-button">Examples…</Dialog.Trigger>
        <Dialog.Portal container={container}>
          <Dialog.Backdrop className="vc-examples-backdrop" />
          <Dialog.Popup className="vc-popover vc-examples">
            <Dialog.Title>Examples</Dialog.Title>
            <Dialog.Description>
              Shaders, scripts and design systems on your computer.
            </Dialog.Description>
            <label>
              Show{" "}
              <select
                aria-label="Example category"
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              >
                {["Shaders", "Scripts", "Design systems"].map((c) => (
                  <option key={c}>{c}</option>
                ))}
              </select>
            </label>
            <ul>
              {catalog
                .filter((e) => e.kind === category)
                .map((e) => (
                  <li key={e.id}>
                    <strong>{e.name}</strong>
                    <p>{e.description}</p>
                    <button
                      type="button"
                      className="vc-button"
                      disabled={
                        ctx.disabled ||
                        busy ||
                        (e.kind === "Shaders" && e.effect !== effect) ||
                        (e.kind === "Shaders"
                          ? !controller?.host.importExample
                          : !controller?.host.openExample)
                      }
                      title={
                        e.kind === "Shaders" && e.effect !== effect
                          ? e.effect
                            ? "Use this example from a shader effect"
                            : "Use this example from a shader fill"
                          : undefined
                      }
                      onClick={() =>
                        e.kind === "Shaders"
                          ? void choose(e.id)
                          : void controller?.host
                              .openExample?.(e.id)
                              .catch(() => setMessage("The example could not be opened"))
                      }
                    >
                      {e.kind === "Shaders" ? "Use shader" : "Open example"}
                    </button>
                  </li>
                ))}
            </ul>
            <Dialog.Close className="vc-button">Close</Dialog.Close>
          </Dialog.Popup>
        </Dialog.Portal>
      </Dialog.Root>
      <p className="vc-paint__hint">
        Controls come from the shader’s annotations. Bind any uniform to a variable.
      </p>
      {src && controller?.host.inspect ? (
        <TextButton disabled={busy} onClick={() => void controller.refresh(src)}>
          Reload shader
        </TextButton>
      ) : null}
    </section>
  );
}
const emptySubscribe = () => ({ dispose() {} });
