import { fontUsage, type ResourceController } from "@vellum/editor-core";
import type { Document } from "@vellum/types-ts";
import { useState } from "react";
import { ResourceButton as Button } from "./Dialog";
export function FontPickerBody({
  resources: r,
  document: doc,
  disabled,
  onChoose,
}: {
  resources: ResourceController;
  document: Document;
  disabled: boolean;
  onChoose(family: string): void;
}) {
  const [query, setQuery] = useState("");
  const usage = fontUsage(doc);
  const groups: [string, readonly string[]][] = [
    ["Recently used", r.host.recentFonts ?? []],
    [
      "This document",
      usage
        .filter((u) => u.assets.length || r.host.availableFonts?.includes(u.family))
        .map((u) => u.family),
    ],
    ["Google Fonts", r.host.googleFonts ?? []],
    ["System fonts", r.host.systemFonts ?? []],
  ];
  return (
    <>
      <label className="vc-resource-search">
        Search
        <input
          type="search"
          aria-label="Search fonts"
          placeholder="Search fonts…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      {groups.map(([label, names]) => (
        <section className="vc-resource-section" key={label}>
          <h3>{label}</h3>
          <div className="vc-font-choices">
            {[...new Set(names)]
              .filter((n) => n.toLowerCase().includes(query.toLowerCase()))
              .map((n) => (
                <button
                  type="button"
                  key={n}
                  className="vc-font-choice"
                  disabled={disabled}
                  onClick={() => onChoose(n)}
                >
                  {n}
                  {usage.some((u) => u.family === n && u.assets.length) ? " · custom" : ""}
                </button>
              ))}
          </div>
        </section>
      ))}
      <div className="vc-resource-actions">
        <Button
          primary
          disabled={disabled || !r.host.chooseFont}
          onClick={() => void r.importFont()}
        >
          Add font…
        </Button>
        <Button onClick={() => r.open("manageFonts")}>Manage fonts</Button>
      </div>
      <p className="vc-resource-hint">TTF, OTF, WOFF, WOFF2</p>
    </>
  );
}
export function ManageFontsBody({
  resources: r,
  document: doc,
  disabled,
}: {
  resources: ResourceController;
  document: Document;
  disabled: boolean;
}) {
  const [replace, setReplace] = useState<string | null>(null);
  return (
    <>
      <p className="vc-resource-description">
        Review text wrapping after changing or replacing a font.
      </p>
      {fontUsage(doc)
        .filter((u) => u.assets.length || !r.host.availableFonts?.includes(u.family))
        .map((u) => {
          const missing =
            !!r.host.availableFonts &&
            !r.host.availableFonts.includes(u.family) &&
            u.assets.length === 0;
          return (
            <section className="vc-resource-section" key={u.family}>
              <h3>{u.family}</h3>
              <p className="vc-resource-hint">
                {u.nodes.length} text layers ·{" "}
                {missing
                  ? `substitute: ${r.host.fallbackFont ?? "unavailable"}`
                  : u.assets.length
                    ? "stored in file"
                    : "available on this computer"}
              </p>
              <p
                className={`vc-font-status ${missing ? "vc-resource-warning" : "vc-resource-hint"}`}
              >
                {missing ? "⚠ Missing" : u.assets.length ? "Custom" : "Available"}
              </p>
              <div className="vc-resource-actions">
                {u.assets.map((asset) => (
                  <Button
                    key={asset}
                    disabled={disabled}
                    onClick={() => r.attempt(() => r.removeFont(asset))}
                  >
                    Remove
                  </Button>
                ))}
                {missing ? (
                  <>
                    <Button disabled={disabled} onClick={() => setReplace(u.family)}>
                      Replace…
                    </Button>
                    <Button
                      disabled={disabled || !r.host.action}
                      onClick={() => r.host.action?.("font.install", { family: u.family })}
                    >
                      Install from Google Fonts
                    </Button>
                  </>
                ) : null}
              </div>
              {replace === u.family ? (
                <label>
                  Replacement font
                  <select
                    aria-label={`Replace ${u.family}`}
                    defaultValue=""
                    disabled={disabled}
                    onChange={(e) =>
                      r.attempt(() => {
                        r.replaceFont(u.family, e.target.value);
                        setReplace(null);
                      })
                    }
                  >
                    <option value="" disabled>
                      Choose replacement
                    </option>
                    {r.host.availableFonts?.map((f) => (
                      <option key={f}>{f}</option>
                    ))}
                  </select>
                </label>
              ) : null}
            </section>
          );
        })}
      <Button primary onClick={() => r.close()}>
        Done
      </Button>
    </>
  );
}
