import { CODE_STACKS, type CodeStack, type InspectController } from "@vellum/editor-core";
import { useState } from "react";
import { useStore } from "../hooks";

/** C19: the implementation spec for the selection, with the hand-offs to a coding agent and to HTML. */
export function InspectPanel({ inspect }: { inspect: InspectController }) {
  useStore(inspect.subscribe);
  const s = inspect.state;
  const [menu, setMenu] = useState<"engines" | "settings" | null>(null);
  if (!s.node)
    return <p className="vc-design__empty">Select a layer to see its implementation spec.</p>;
  if (s.loading) return <p className="vc-design__empty">Reading the spec…</p>;
  if (!s.spec)
    return <p className="vc-design__empty">{s.error ?? "No spec is available for this layer."}</p>;
  const stackLabel = CODE_STACKS.find((x) => x.id === s.stack)?.label ?? s.stack;
  return (
    <div className="vl-inspect">
      <section>
        <h3>Implementation spec</h3>
        <pre className="vc-mono">{s.spec.summary.join("\n")}</pre>
      </section>
      <section>
        <h3>Tokens used</h3>
        <pre className="vc-mono">
          {s.spec.tokens.map((t) => (t.value ? `${t.name}\n${t.value}` : t.name)).join("\n")}
        </pre>
      </section>
      <section>
        <h3>Components</h3>
        <pre className="vc-mono">{s.spec.components.join("\n")}</pre>
      </section>
      <section>
        <label className="vl-stack">
          <span>Stack · {stackLabel}</span>
          <select
            aria-label="Code stack"
            value={s.stack}
            onChange={(e) => inspect.set({ stack: e.target.value as CodeStack })}
          >
            {CODE_STACKS.map((x) => (
              <option key={x.id} value={x.id}>
                {x.label}
              </option>
            ))}
          </select>
        </label>
        <p className="vl-muted">{CODE_STACKS.map((x) => x.label).join(" · ")}</p>
        <pre className="vl-code vc-mono">{inspect.code()}</pre>
      </section>
      <section className="vl-handoff">
        <button
          type="button"
          className="vx-button"
          data-primary=""
          onClick={() => void inspect.copyPrompt()}
        >
          Copy prompt for your coding agent
        </button>
        <div className="vl-menu">
          <button
            type="button"
            className="vx-button"
            aria-haspopup="menu"
            aria-expanded={menu === "engines"}
            onClick={() => setMenu(menu === "engines" ? null : "engines")}
          >
            Implement with…
          </button>
          {menu === "engines" ? (
            <div role="menu" className="vl-popup">
              {s.engines.map((e) => (
                <button
                  key={e.id}
                  type="button"
                  role="menuitem"
                  disabled={!e.ready}
                  onClick={() => {
                    setMenu(null);
                    void inspect.implementWith(e.id);
                  }}
                >
                  {e.name} · {e.status}
                </button>
              ))}
              {s.engines.length === 0 ? <p className="vl-muted">No engine installed</p> : null}
            </div>
          ) : null}
        </div>
        {inspect.engineLine() ? <p className="vl-muted">{inspect.engineLine()}</p> : null}
      </section>
      <section>
        <h3>Code</h3>
        <fieldset className="vl-flavour">
          <legend className="vl-sr">HTML flavour</legend>
          {(
            [
              ["tailwind", "HTML + Tailwind"],
              ["css", "HTML + CSS"],
            ] as const
          ).map(([id, label]) => (
            <label key={id}>
              <input
                type="radio"
                name="html-flavour"
                checked={s.settings.flavour === id}
                onChange={() => inspect.set({ flavour: id })}
              />
              <span>{label}</span>
            </label>
          ))}
        </fieldset>
        <div className="vl-row">
          <button type="button" className="vx-button" onClick={() => void inspect.copyHtml()}>
            Copy HTML
          </button>
          <button type="button" className="vx-button" onClick={() => void inspect.exportHtml()}>
            Export HTML
          </button>
        </div>
        <div className="vl-menu">
          <button
            type="button"
            className="vl-link vl-muted"
            aria-haspopup="dialog"
            aria-expanded={menu === "settings"}
            onClick={() => setMenu(menu === "settings" ? null : "settings")}
          >
            {inspect.settingsLine()}
          </button>
          {menu === "settings" ? (
            <div className="vl-popup" role="dialog" aria-label="HTML export settings">
              <label>
                <input
                  type="checkbox"
                  checked={s.settings.scaffold}
                  onChange={(e) => inspect.set({ scaffold: e.target.checked })}
                />
                Scaffold
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={s.settings.assets === "files"}
                  onChange={(e) => inspect.set({ assets: e.target.checked ? "files" : "inline" })}
                />
                Write assets as files
              </label>
            </div>
          ) : null}
        </div>
      </section>
      {s.error ? (
        <p className="vl-error" role="alert">
          {s.error}
        </p>
      ) : null}
    </div>
  );
}
