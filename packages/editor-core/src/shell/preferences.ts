/**
 * Local preferences (DESIGN.md C25 Editor settings): the choices that belong to this person on this computer, not to
 * the document. Every change goes through one validator and one host `save`; nothing here is sent anywhere, and
 * telemetry stays off until the person turns it on. API keys never pass through here: they live in the keychain.
 */

export type Permission = "ask" | "allow";

export interface Preferences {
  "appearance.theme": "follow" | "light" | "dark" | "high-contrast";
  "appearance.matchEditorColours": boolean;
  "agents.planThreshold": "frames-2" | "frames-3" | "changes-20";
  "agents.externalHelpers": "ask" | "allow" | "never";
  "agents.notifications": boolean;
  "agents.autoOpenHistory": boolean;
  "agents.insertHighlight": boolean;
  "permissions.repoReads": Permission;
  "permissions.fileWrites": Permission;
  "permissions.commands": Permission;
  "permissions.engineAutoMode": boolean;
  "assets.iconSets": string[];
  "assets.stockProvider": string;
  "assets.imageModel": string;
  "assets.imageProvider": string;
  /** The canvas backdrop for this document, as `#rrggbb`; `null` follows the appearance. */
  "canvas.backdrop": string | null;
  "canvas.pixelGrid": boolean;
  "canvas.snapPixelGrid": boolean;
  "canvas.snapObjects": boolean;
  "canvas.wheelZooms": boolean;
  "canvas.invertZoom": boolean;
  "canvas.nudgeSmall": number;
  "canvas.nudgeBig": number;
  "canvas.animations": boolean;
  renderer: "automatic" | "basic";
  language: string;
  telemetry: boolean;
}
export type PreferenceKey = keyof Preferences;

export const DEFAULT_PREFERENCES: Readonly<Preferences> = Object.freeze({
  "appearance.theme": "follow",
  "appearance.matchEditorColours": false,
  "agents.planThreshold": "frames-3",
  "agents.externalHelpers": "ask",
  "agents.notifications": true,
  "agents.autoOpenHistory": false,
  "agents.insertHighlight": true,
  "permissions.repoReads": "ask",
  "permissions.fileWrites": "ask",
  "permissions.commands": "ask",
  "permissions.engineAutoMode": false,
  "assets.iconSets": ["lucide", "material", "phosphor", "feather"],
  "assets.stockProvider": "pixabay",
  "assets.imageModel": "default",
  "assets.imageProvider": "none",
  "canvas.backdrop": null,
  "canvas.pixelGrid": false,
  "canvas.snapPixelGrid": true,
  "canvas.snapObjects": true,
  "canvas.wheelZooms": false,
  "canvas.invertZoom": false,
  "canvas.nudgeSmall": 1,
  "canvas.nudgeBig": 10,
  "canvas.animations": true,
  renderer: "automatic",
  language: "en",
  telemetry: false,
});

/** What exactly is sent if telemetry is on (DESIGN.md C25). The Settings dialog shows these lines and no others. */
export const TELEMETRY_DISCLOSURE: readonly string[] = [
  "Off by default",
  "If on: version, OS, renderer failures, anonymous feature counts.",
  "Never files, prompts, keys or repo paths.",
];

export const LANGUAGES: readonly { id: string; label: string }[] = [
  { id: "en", label: "English" },
  { id: "ar", label: "Arabic" },
];

const oneOf = (values: readonly string[]) => (v: unknown) =>
  typeof v === "string" && values.includes(v) ? null : "Choose one of the options";
const flag = (v: unknown) => (typeof v === "boolean" ? null : "Choose on or off");
const nudge = (v: unknown) =>
  typeof v === "number" && Number.isInteger(v) && v >= 1 && v <= 100
    ? null
    : "Enter a whole number from 1 to 100";
const text = (v: unknown) =>
  typeof v === "string" && v.length > 0 && v.length <= 200 ? null : "Enter a value";

const checks: { [K in PreferenceKey]: (v: unknown) => string | null } = {
  "appearance.theme": oneOf(["follow", "light", "dark", "high-contrast"]),
  "appearance.matchEditorColours": flag,
  "agents.planThreshold": oneOf(["frames-2", "frames-3", "changes-20"]),
  "agents.externalHelpers": oneOf(["ask", "allow", "never"]),
  "agents.notifications": flag,
  "agents.autoOpenHistory": flag,
  "agents.insertHighlight": flag,
  "permissions.repoReads": oneOf(["ask", "allow"]),
  "permissions.fileWrites": oneOf(["ask", "allow"]),
  "permissions.commands": oneOf(["ask", "allow"]),
  "permissions.engineAutoMode": flag,
  "assets.iconSets": (v) =>
    Array.isArray(v) && v.every((x) => typeof x === "string") ? null : "Choose icon sets",
  "assets.stockProvider": text,
  "assets.imageModel": text,
  "assets.imageProvider": text,
  "canvas.backdrop": (v) =>
    v === null || (typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v))
      ? null
      : "Enter a colour like #EDEDEB",
  "canvas.pixelGrid": flag,
  "canvas.snapPixelGrid": flag,
  "canvas.snapObjects": flag,
  "canvas.wheelZooms": flag,
  "canvas.invertZoom": flag,
  "canvas.nudgeSmall": nudge,
  "canvas.nudgeBig": nudge,
  "canvas.animations": flag,
  renderer: oneOf(["automatic", "basic"]),
  language: oneOf(LANGUAGES.map((l) => l.id)),
  telemetry: flag,
};

/** The one door to storage: the browser's local storage, a desktop settings file, VS Code's memento. */
export interface PreferencesHost {
  load(): Promise<Partial<Record<string, unknown>> | null>;
  save(values: Readonly<Preferences>): Promise<void>;
}

export type SetResult = { ok: true } | { ok: false; reason: string };

export function createPreferences(host: PreferencesHost) {
  let values: Preferences = {
    ...DEFAULT_PREFERENCES,
    "assets.iconSets": [...DEFAULT_PREFERENCES["assets.iconSets"]],
  };
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const persist = () => {
    void host.save({ ...values }).catch(() => {
      // A failed save keeps the value for this session; the next change tries again.
    });
  };
  return {
    get state(): Readonly<Preferences> {
      return values;
    },
    get<K extends PreferenceKey>(key: K): Preferences[K] {
      return values[key];
    },
    set<K extends PreferenceKey>(key: K, value: Preferences[K]): SetResult {
      const problem = checks[key]?.(value);
      if (problem === undefined) return { ok: false, reason: "Unknown setting" };
      if (problem) return { ok: false, reason: problem };
      values = { ...values, [key]: value };
      persist();
      notify();
      return { ok: true };
    },
    reset(key: PreferenceKey) {
      values = { ...values, [key]: DEFAULT_PREFERENCES[key] };
      persist();
      notify();
    },
    /** Saved values it does not know, or that no longer validate, are dropped, never trusted. */
    async load() {
      const saved = (await host.load().catch(() => null)) ?? {};
      const next = { ...values } as Record<string, unknown>;
      for (const key of Object.keys(checks) as PreferenceKey[])
        if (key in saved && checks[key](saved[key]) === null) next[key] = saved[key];
      values = next as unknown as Preferences;
      notify();
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
  };
}
export type PreferencesController = ReturnType<typeof createPreferences>;

/** An in-memory host for tests and stories: keeps what was saved, newest last. */
export class FakePreferencesHost implements PreferencesHost {
  saved: Preferences[] = [];
  constructor(private initial: Partial<Record<string, unknown>> | null = null) {}
  async load(): Promise<Partial<Record<string, unknown>> | null> {
    const last = this.saved.at(-1);
    return last ? { ...last } : this.initial;
  }
  async save(values: Readonly<Preferences>) {
    this.saved.push({ ...values });
  }
}
