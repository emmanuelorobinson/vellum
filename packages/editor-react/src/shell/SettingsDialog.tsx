import { Dialog } from "@base-ui/react/dialog";
import { copy } from "@vellum/config";
import {
  type ComposerController,
  EFFORTS,
  type Effort,
  LANGUAGES,
  type PreferenceKey,
  type Preferences,
  type SettingsController,
  TELEMETRY_DISCLOSURE,
} from "@vellum/editor-core";
import { type ReactNode, useEffect, useState } from "react";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import "./shell.css";

/** What only the host can do for Settings. */
export interface SettingsHost {
  /** Stock image providers the host has configured. */
  stockProviders?: readonly { id: string; name: string }[];
  /** A stock provider key is in the keychain. The key itself never comes here. */
  providerKeySet?: boolean;
  /** Opens the keychain entry for the stock provider key. */
  changeProviderKey?(): void;
  removeSkill?(path: string): void;
  /** The host keeps these chords (the IDE's own); the line under Keyboard shortcuts names them. */
  hostChords?: readonly string[];
}

const NO_STORE = () => ({ dispose() {} });
const PLAN_THRESHOLDS = [
  ["frames-2", "2 frames"],
  ["frames-3", "3 frames"],
  ["changes-20", "20 changes"],
] as const;
const ICON_SETS = [
  ["lucide", "Lucide"],
  ["material", "Material Symbols"],
  ["phosphor", "Phosphor"],
  ["feather", "Feather"],
] as const;

function Row({ label, children, note }: { label: string; children: ReactNode; note?: string }) {
  return (
    <div className="vs-row">
      <span className="vs-row__label">{label}</span>
      <span className="vs-row__control">{children}</span>
      {note ? <span className="vs-row__note">{note}</span> : null}
    </div>
  );
}
function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="vs-section" aria-label={title}>
      <h3>{title}</h3>
      {children}
    </section>
  );
}
function Switch({
  label,
  on,
  onChange,
  disabled,
}: {
  label: string;
  on: boolean;
  onChange(on: boolean): void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className="vs-switch"
      disabled={disabled}
      onClick={() => onChange(!on)}
    >
      <span className="vs-switch__word">{on ? "On" : "Off"}</span>
    </button>
  );
}
function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly (readonly [T, string])[];
  onChange(v: T): void;
}) {
  return (
    <span className="vs-segmented" role="radiogroup" aria-label={label}>
      {options.map(([id, text]) => (
        // biome-ignore lint/a11y/useSemanticElements: a segmented control; the radio role gives the group its arrow-key meaning
        <button
          key={id}
          type="button"
          role="radio"
          aria-checked={value === id}
          data-checked={value === id ? "" : undefined}
          onClick={() => onChange(id)}
        >
          {text}
        </button>
      ))}
    </span>
  );
}
function Select<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled,
}: {
  label: string;
  value: T;
  options: readonly (readonly [T, string])[];
  onChange(v: T): void;
  disabled?: boolean;
}) {
  return (
    <select
      className="vs-select"
      aria-label={label}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value as T)}
    >
      {options.map(([id, text]) => (
        <option key={id} value={id}>
          {text}
        </option>
      ))}
    </select>
  );
}

/** A number the person types; a value that does not validate puts the old one back and says why beside the field. */
function NumberField({
  label,
  prefKey,
  settings,
}: {
  label: string;
  prefKey: "canvas.nudgeSmall" | "canvas.nudgeBig";
  settings: SettingsController;
}) {
  const prefs = settings.preferences;
  const [text, setText] = useState(String(prefs.get(prefKey)));
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => setText(String(prefs.get(prefKey))), [prefs.get(prefKey), prefs, prefKey]);
  const commit = () => {
    const result = prefs.set(prefKey, Number(text));
    if (result.ok) setProblem(null);
    else {
      setProblem(result.reason);
      setText(String(prefs.get(prefKey)));
    }
  };
  return (
    <span className="vs-number">
      <input
        className="vs-input vc-mono"
        inputMode="numeric"
        aria-label={label}
        aria-invalid={problem ? true : undefined}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
      <span className="vs-unit">px</span>
      {problem ? (
        <span className="vs-problem" role="alert">
          {problem}
        </span>
      ) : null}
    </span>
  );
}

function Shortcuts({
  settings,
  host,
  platform,
}: {
  settings: SettingsController;
  host: SettingsHost;
  platform: "mac" | "windows" | "linux";
}) {
  useStore(settings.subscribe);
  const [query, setQuery] = useState("");
  const [capturing, setCapturing] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const rows = settings.shortcuts(query);
  const label = (chord: string) => shortcutChord(chord, platform);
  const capture = (command: string, e: React.KeyboardEvent) => {
    e.preventDefault();
    if (e.key === "Escape") {
      setCapturing(null);
      return;
    }
    if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return;
    const chord = [
      (platform === "mac" ? e.metaKey : e.ctrlKey) && "Mod",
      platform === "mac" && e.ctrlKey && "Control",
      e.altKey && "Alt",
      e.shiftKey && "Shift",
      e.code,
    ]
      .filter(Boolean)
      .join("+");
    const result = settings.rebind(command, chord);
    setCapturing(null);
    setMessage(
      result.ok
        ? null
        : result.conflicts.includes("host")
          ? "The host keeps that shortcut"
          : result.conflicts.includes("invalid")
            ? "That shortcut cannot be used"
            : `Already used by ${settings.titleOf(result.conflicts[0] as string)}`,
    );
  };
  return (
    <div className="vs-shortcuts">
      <input
        className="vs-input"
        type="search"
        aria-label="Search shortcuts"
        placeholder="Search shortcuts"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {message ? (
        <p className="vs-problem" role="alert">
          {message}
        </p>
      ) : null}
      <ul className="vs-shortcuts__list" aria-label="Keyboard shortcuts">
        {rows.map((r, i) => (
          <li key={`${r.command}:${i}`} data-conflict={r.hostConflict ? "" : undefined}>
            <span className="vs-shortcuts__title">{r.title}</span>
            <button
              type="button"
              className="vs-chord vc-mono"
              aria-label={`Change shortcut for ${r.title}`}
              disabled={r.hostConflict}
              onClick={() => {
                setMessage(null);
                setCapturing(r.command);
              }}
              onKeyDown={(e) => capturing === r.command && capture(r.command, e)}
              onBlur={() => capturing === r.command && setCapturing(null)}
            >
              {capturing === r.command ? "Press keys" : label(r.chord)}
            </button>
            {r.hostConflict ? <span className="vs-host">Kept by the host</span> : null}
          </li>
        ))}
      </ul>
      <p className="vs-row__note">
        {(host.hostChords ?? HOST_CHORDS).map((c) => shortcutChord(c, platform)).join(", ")} stay
        with the host
      </p>
    </div>
  );
}

/** The chords Vellum never takes from the host (DESIGN.md "Host conflicts"). */
const HOST_CHORDS = ["Mod+KeyP", "Mod+Shift+KeyP", "Mod+KeyB", "Mod+KeyJ", "Mod+KeyW", "Mod+KeyS"];
const MAC_MODS: Record<string, string> = {
  Mod: "⌘",
  Alt: "⌥",
  Shift: "⇧",
  Control: "⌃",
  Meta: "⌘",
};
const OTHER_MODS: Record<string, string> = {
  Mod: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Control: "Ctrl",
  Meta: "Win",
};
const KEYS: Record<string, string> = {
  Backslash: "\\",
  Slash: "/",
  Period: ".",
  Quote: "'",
  Equal: "=",
  Minus: "−",
  BracketLeft: "[",
  BracketRight: "]",
};
function shortcutChord(chord: string, platform: "mac" | "windows" | "linux"): string {
  const parts = chord.split("+");
  const key = parts.pop() as string;
  const text = KEYS[key] ?? key.replace(/^(Key|Digit)/, "");
  if (platform === "mac") {
    const order = ["Control", "Alt", "Shift", "Mod", "Meta"];
    return (
      parts
        .sort((a, b) => order.indexOf(a) - order.indexOf(b))
        .map((m) => MAC_MODS[m] ?? m)
        .join("") + text
    );
  }
  return [...parts.map((m) => OTHER_MODS[m] ?? m), text].join("+");
}

/** Editor settings (C25): three columns of plain controls over typed preferences, the Agent feed and the keymap. */
export function SettingsDialog({
  settings,
  composer,
  host = {},
  platform,
}: {
  settings: SettingsController;
  composer?: ComposerController | undefined;
  host?: SettingsHost;
  platform: "mac" | "windows" | "linux";
}) {
  useStore(settings.subscribe);
  useStore(settings.preferences.subscribe);
  useStore(composer?.subscribe ?? NO_STORE);
  const container = usePortalContainer();
  const prefs = settings.preferences;
  const p = prefs.state;
  // The backdrop the appearance gives, shown until the person picks their own.
  const backdropDefault = container
    ? getComputedStyle(container).getPropertyValue("--canvas-backdrop").trim().toUpperCase()
    : "";
  const set = <K extends PreferenceKey>(key: K, value: Preferences[K]) =>
    void prefs.set(key, value);
  if (!settings.state.open) return null;
  const cs = composer?.state;
  const choice = cs?.choice;
  const userSkills = cs?.skills.filter((s) => s.group === "user") ?? [];
  const skipAll = cs?.skipAll ?? false;
  return (
    <Dialog.Root open onOpenChange={(o) => !o && settings.close()}>
      <Dialog.Portal container={container}>
        <Dialog.Backdrop className="vc-resource-backdrop" />
        <Dialog.Popup className="vc-resource-dialog vs-settings">
          <Dialog.Title className="vs-title">Editor settings</Dialog.Title>
          <div className="vs-columns">
            <div className="vs-column">
              <Section title="Appearance">
                <Row label="Theme">
                  <Segmented
                    label="Theme"
                    value={p["appearance.theme"]}
                    options={[
                      ["follow", "Follow editor theme"],
                      ["light", "Light"],
                      ["dark", "Dark"],
                      ["high-contrast", "High contrast"],
                    ]}
                    onChange={(v) => set("appearance.theme", v)}
                  />
                </Row>
                <Row label="Match editor colours">
                  <Switch
                    label="Match editor colours"
                    on={p["appearance.matchEditorColours"]}
                    onChange={(v) => set("appearance.matchEditorColours", v)}
                  />
                </Row>
              </Section>
              <Section title="Agents">
                <Row label="Engine and model">
                  <button
                    type="button"
                    className="vs-link vc-mono"
                    disabled={!composer}
                    onClick={() => {
                      settings.close();
                      composer?.open({ type: "picker" });
                    }}
                  >
                    {choice
                      ? `${choice.provider ? `${choice.provider} · ` : ""}${choice.model}`
                      : "Choose"}
                  </button>
                </Row>
                <Row label="Reasoning effort">
                  <Select<Effort>
                    label="Reasoning effort"
                    value={choice?.effort ?? "medium"}
                    options={EFFORTS.map((e) => [e.id, e.label] as const)}
                    disabled={!composer || !choice}
                    onChange={(v) => composer?.setEffort(v)}
                  />
                </Row>
                <Row label="Parallel">
                  <span className="vc-mono">
                    {cs
                      ? `${cs.parallel.count}× · ${cs.parallel.mode === "split" ? "Split" : "Side by side"}`
                      : "1× · Split"}
                  </span>
                </Row>
                <Row label="Plan threshold">
                  <Select
                    label="Plan threshold"
                    value={p["agents.planThreshold"]}
                    options={PLAN_THRESHOLDS}
                    onChange={(v) => set("agents.planThreshold", v)}
                  />
                </Row>
                <Row label="External helper agents">
                  <Select
                    label="External helper agents"
                    value={p["agents.externalHelpers"]}
                    options={[
                      ["ask", "Ask"],
                      ["allow", "Allow"],
                      ["never", "Never"],
                    ]}
                    onChange={(v) => set("agents.externalHelpers", v)}
                  />
                </Row>
                <Row label="Background notifications">
                  <Switch
                    label="Background notifications"
                    on={p["agents.notifications"]}
                    onChange={(v) => set("agents.notifications", v)}
                  />
                </Row>
                <Row label="Auto-open History">
                  <Switch
                    label="Auto-open History"
                    on={p["agents.autoOpenHistory"]}
                    onChange={(v) => set("agents.autoOpenHistory", v)}
                  />
                </Row>
                <Row label="Insert highlight">
                  <Switch
                    label="Insert highlight"
                    on={p["agents.insertHighlight"]}
                    onChange={(v) => set("agents.insertHighlight", v)}
                  />
                </Row>
              </Section>
              <Section title="Canvas">
                <Row label="Backdrop">
                  <input
                    className="vs-input vc-mono"
                    aria-label="Backdrop"
                    defaultValue={p["canvas.backdrop"] ?? backdropDefault}
                    key={p["canvas.backdrop"] ?? backdropDefault}
                    onBlur={(e) => {
                      const v = e.target.value.trim();
                      if (v === backdropDefault && p["canvas.backdrop"] === null) return;
                      const r = prefs.set("canvas.backdrop", v);
                      if (!r.ok) e.target.value = p["canvas.backdrop"] ?? backdropDefault;
                    }}
                  />
                </Row>
                <Row label="Show pixel grid">
                  <Switch
                    label="Show pixel grid"
                    on={p["canvas.pixelGrid"]}
                    onChange={(v) => set("canvas.pixelGrid", v)}
                  />
                </Row>
                <Row label="Snap to pixel grid">
                  <Switch
                    label="Snap to pixel grid"
                    on={p["canvas.snapPixelGrid"]}
                    onChange={(v) => set("canvas.snapPixelGrid", v)}
                  />
                </Row>
                <Row label="Snap to objects">
                  <Switch
                    label="Snap to objects"
                    on={p["canvas.snapObjects"]}
                    onChange={(v) => set("canvas.snapObjects", v)}
                  />
                </Row>
                <Row label="Scroll wheel zooms">
                  <Switch
                    label="Scroll wheel zooms"
                    on={p["canvas.wheelZooms"]}
                    onChange={(v) => set("canvas.wheelZooms", v)}
                  />
                </Row>
                <Row label="Invert zoom direction">
                  <Switch
                    label="Invert zoom direction"
                    on={p["canvas.invertZoom"]}
                    onChange={(v) => set("canvas.invertZoom", v)}
                  />
                </Row>
                <Row label="Small nudge">
                  <NumberField
                    label="Small nudge"
                    prefKey="canvas.nudgeSmall"
                    settings={settings}
                  />
                </Row>
                <Row label="Big nudge">
                  <NumberField label="Big nudge" prefKey="canvas.nudgeBig" settings={settings} />
                </Row>
                <Row label="Animations and effects">
                  <Switch
                    label="Animations and effects"
                    on={p["canvas.animations"]}
                    onChange={(v) => set("canvas.animations", v)}
                  />
                </Row>
              </Section>
            </div>
            <div className="vs-column">
              <Section title="Permissions">
                {(
                  [
                    ["permissions.repoReads", "Repo reads"],
                    ["permissions.fileWrites", "File writes"],
                    ["permissions.commands", "Commands"],
                  ] as const
                ).map(([key, label]) => (
                  <Row key={key} label={label}>
                    <Segmented
                      label={label}
                      value={p[key]}
                      options={[
                        ["ask", "Ask"],
                        ["allow", "Allow"],
                      ]}
                      onChange={(v) => set(key, v)}
                    />
                  </Row>
                ))}
                <Row label="Engine auto mode">
                  <Switch
                    label="Engine auto mode"
                    on={p["permissions.engineAutoMode"]}
                    onChange={(v) => set("permissions.engineAutoMode", v)}
                  />
                </Row>
                <Row label="Skip all prompts" note="When on: persistent amber warning">
                  <Switch
                    label="Skip all prompts"
                    on={skipAll}
                    disabled={!composer}
                    onChange={(v) => void composer?.setSkipAll(v)}
                  />
                </Row>
              </Section>
              <Section title="Skills">
                <ul className="vs-list" aria-label="Skills">
                  {userSkills.map((s) => (
                    <li key={s.path}>
                      <span className="vc-mono">{s.path}</span>
                      <button
                        type="button"
                        className="vs-link"
                        disabled={!host.removeSkill}
                        aria-label={`Remove ${s.name}`}
                        onClick={() => host.removeSkill?.(s.path)}
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                </ul>
                <button
                  type="button"
                  className="vs-link"
                  disabled={!composer}
                  onClick={() => void composer?.addSkill()}
                >
                  Add skill…
                </button>
              </Section>
              <Section title="Assets">
                <Row label="Icons">
                  <fieldset className="vs-checks">
                    <legend className="va-sr">Icon sets</legend>
                    {ICON_SETS.map(([id, text]) => (
                      <label key={id}>
                        <input
                          type="checkbox"
                          checked={p["assets.iconSets"].includes(id)}
                          onChange={(e) =>
                            set(
                              "assets.iconSets",
                              e.target.checked
                                ? [...p["assets.iconSets"], id]
                                : p["assets.iconSets"].filter((x) => x !== id),
                            )
                          }
                        />
                        {text}
                      </label>
                    ))}
                  </fieldset>
                </Row>
                <Row label="Stock provider">
                  <Select
                    label="Stock provider"
                    value={p["assets.stockProvider"]}
                    options={(
                      host.stockProviders ?? [
                        { id: p["assets.stockProvider"], name: p["assets.stockProvider"] },
                      ]
                    ).map((s) => [s.id, s.name] as const)}
                    onChange={(v) => set("assets.stockProvider", v)}
                  />
                </Row>
                <Row label="Provider key" note={copy.keyStorage}>
                  <span className="vs-key">
                    <span>
                      {host.providerKeySet ? "••••••••" : "Not set"}
                      <span className="va-sr">{host.providerKeySet ? " Key stored" : ""}</span>
                    </span>
                    <button
                      type="button"
                      className="vs-link"
                      disabled={!host.changeProviderKey}
                      onClick={() => host.changeProviderKey?.()}
                    >
                      {host.providerKeySet ? "Change" : "Add key"}
                    </button>
                  </span>
                </Row>
                <Row label="Image generation model">
                  <Select
                    label="Image generation model"
                    value="default"
                    disabled
                    options={[["default", "Default"]]}
                    onChange={() => {}}
                  />
                </Row>
              </Section>
            </div>
            <div className="vs-column">
              <Section title="Renderer">
                <Segmented
                  label="Renderer"
                  value={p.renderer}
                  options={[
                    ["automatic", "Automatic"],
                    ["basic", "Basic"],
                  ]}
                  onChange={(v) => set("renderer", v)}
                />
              </Section>
              <Section title="Language">
                <Select
                  label="Language"
                  value={p.language}
                  options={LANGUAGES.map((l) => [l.id, l.label] as const)}
                  onChange={(v) => set("language", v)}
                />
                <p className="vs-row__note">Arabic UI mirrors chrome only</p>
              </Section>
              <Section title="Telemetry">
                <Row label="Send anonymous usage data">
                  <Switch
                    label="Telemetry"
                    on={p.telemetry}
                    onChange={(v) => set("telemetry", v)}
                  />
                </Row>
                {TELEMETRY_DISCLOSURE.map((line) => (
                  <p key={line} className="vs-disclosure">
                    {line}
                  </p>
                ))}
              </Section>
              <Section title="Keyboard shortcuts">
                <Shortcuts settings={settings} host={host} platform={platform} />
              </Section>
            </div>
          </div>
          <div className="vs-actions">
            <Dialog.Close className="vc-resource-button" data-primary="">
              Done
            </Dialog.Close>
          </div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
