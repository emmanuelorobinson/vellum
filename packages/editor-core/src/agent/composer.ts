/**
 * The composer (DESIGN.md "Composer", C01, C08, C11, C26, C27): the field and its chips, @ mentions and the / skills
 * menu, Parallel and Explore, the engine and model, Send ↔ Stop with the queue-or-stop prompt, prompt history, a draft
 * per session, the Permission card's answers, the usage meter and the inline prompt. Everything goes to the
 * AgentFeed; nothing here changes a document or holds an API key.
 */
import { copy, guardReasons } from "@vellum/config";
import type { AgentId, SessionId, TurnId } from "@vellum/types-ts";
import type { AgentController } from "./controller";
import type {
  Effort,
  EngineCatalog,
  EngineChoice,
  ProviderSetup,
  ProviderStatusName,
} from "./engines";
import { sameProvider } from "./engines";
import type {
  AgentFeed,
  ContextChip,
  ExploreVary,
  FeedUpdate,
  ParallelMode,
  PromptRequest,
  SkillGroupName,
  SkillInfo,
} from "./feed";
import { TERMINAL } from "./log";
import type { PermissionAnswer, PermissionQuestion } from "./permissions";
import type { SessionUsage } from "./usage";

/** What is selected and which page is open, as the composer names them. */
export interface ComposerScope {
  selection: { ids: readonly string[]; label: string } | null;
  page: { id: string; name: string } | null;
  context?: ContextChip[];
}

export type MentionKind = "file" | "frame" | "component" | "variable";
export interface Mention {
  kind: MentionKind;
  label: string;
  value: string;
}

export interface ComposerDeps {
  agent: AgentController;
  feed: AgentFeed;
  scope(): ComposerScope;
  /** Frames, components and variables of the open document that match an @ query. */
  documentMentions(query: string): Mention[];
  /** The engine and model a new composer starts with (Settings → Agents). */
  defaultEngine: EngineChoice | null;
  timeZone?: string;
}

/** A chip as the composer shows it: a context chip, or a chosen skill. */
export interface ChipView {
  key: string;
  kind: ContextChip["kind"] | "skill";
  label: string;
  value: string;
  /** Added from the selection or page, not by the person. */
  auto: boolean;
}

interface Draft {
  text: string;
  chips: ChipView[];
  /** Auto chips the person removed. */
  removed: string[];
}

export type ComposerMenu =
  | {
      type: "mention";
      query: string;
      /** Where the @ is in the text. */
      start: number;
      end: number;
      extensions: readonly string[] | null;
      files: string[];
      active: number;
    }
  | { type: "skills"; query: string; active: number };

export type InlineState =
  | { phase: "edit"; text: string }
  | { phase: "sent"; session: SessionId; turn: TurnId; text: string };

export type ComposerSurface =
  | { type: "picker" }
  | { type: "usage" }
  | { type: "parallel" }
  | { type: "explore" }
  | { type: "providers" }
  | { type: "setup"; provider: string };

export interface ComposerState {
  drafts: Record<string, Draft>;
  menu: ComposerMenu | null;
  skills: SkillInfo[];
  catalog: EngineCatalog | null;
  choice: EngineChoice | null;
  parallel: { count: number; mode: ParallelMode };
  /** `count` 0 is off. */
  explore: { count: number; vary: ExploreVary };
  /** The queue-or-stop prompt is open. */
  queue: boolean;
  history: number | null;
  questions: Record<string, PermissionQuestion[]>;
  answering: string[];
  skipAll: boolean;
  usages: Record<string, SessionUsage>;
  inline: InlineState | null;
  /** Bumped by ⌘L: the view focuses the field when it changes. */
  focus: number;
  /** The popover or dialog open from the composer; `setup` names the provider being set up. */
  surface: ComposerSurface | null;
  sending: boolean;
  error: string | null;
}

const KIND_WORDS: Record<MentionKind, string> = {
  file: "File",
  frame: "Frame",
  component: "Component",
  variable: "Variable",
};
const GROUPS: readonly [SkillGroupName, string][] = [
  ["builtin", "Built-in"],
  ["design_system", "Design system"],
  ["user", "Yours"],
];
const EMPTY: Draft = { text: "", chips: [], removed: [] };
const MENTION = /(^|\s)@([^\s@]*)$/;
const SKILL = /^\/(\S*)$/;

export function createComposerController(deps: ComposerDeps) {
  const { agent, feed } = deps;
  let state: ComposerState = {
    drafts: {},
    menu: null,
    skills: [],
    catalog: null,
    choice: deps.defaultEngine,
    parallel: { count: 1, mode: "split" },
    explore: { count: 0, vary: "layout" },
    queue: false,
    history: null,
    questions: {},
    answering: [],
    skipAll: false,
    usages: {},
    inline: null,
    focus: 0,
    surface: null,
    sending: false,
    error: null,
  };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<ComposerState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const pending = new Set<Promise<unknown>>();
  const track = <T>(p: Promise<T>): Promise<T> => {
    pending.add(p);
    void p.finally(() => pending.delete(p)).catch(() => {});
    return p;
  };
  const attempt = async <T>(work: () => Promise<T>): Promise<T | undefined> => {
    try {
      return await work();
    } catch (e) {
      change({ error: e instanceof Error ? e.message : String(e) });
      return undefined;
    }
  };
  /** Texts sent from this composer, oldest first (any session). */
  const sentTexts: string[] = [];
  /** The engine each session last ran on, for the switch warning. */
  const lastSent: Record<string, EngineChoice> = {};

  const key = () => agent.state.current ?? "new";
  const draft = (): Draft => state.drafts[key()] ?? EMPTY;
  const setDraft = (d: Partial<Draft>) =>
    change({ drafts: { ...state.drafts, [key()]: { ...draft(), ...d } } });

  const autoChip = (): ChipView | null => {
    const scope = deps.scope();
    if (scope.selection)
      return {
        key: `sel:${scope.selection.ids.join(",")}`,
        kind: "selection",
        label: scope.selection.label,
        value: scope.selection.ids.join(","),
        auto: true,
      };
    if (scope.page)
      return {
        key: `page:${scope.page.id}`,
        kind: "page",
        label: scope.page.name,
        value: scope.page.id,
        auto: true,
      };
    return null;
  };
  const chips = (): ChipView[] => {
    const d = draft();
    const auto = autoChip();
    const context = (deps.scope().context ?? []).map((c) => ({
      ...c,
      key: `context:${c.value}`,
      auto: true,
    }));
    return [
      ...(auto && !d.removed.includes(auto.key) ? [auto] : []),
      ...context.filter((c) => !d.removed.includes(c.key)),
      ...d.chips,
    ];
  };
  const addChip = (c: Omit<ChipView, "key" | "auto">) => {
    const k = `${c.kind}:${c.value}`;
    const d = draft();
    if (!d.chips.some((x) => x.key === k))
      setDraft({ chips: [...d.chips, { ...c, key: k, auto: false }] });
  };

  const working = (): AgentId[] | null => {
    const s = agent.session();
    if (!s || s.kind.type === "external") return null;
    const last = agent.turns().at(-1);
    if (last && !TERMINAL.includes(last.state))
      return last.agents.length ? [...last.agents] : [s.agent];
    // The session list can say it runs before its log has arrived.
    if (!last && s.state && !TERMINAL.includes(s.state)) return [s.agent];
    return null;
  };

  const history = (): string[] => {
    const prompts = agent.state.events.flatMap((e) =>
      e.body.type === "prompt" ? [e.body.text] : [],
    );
    return [...prompts, ...sentTexts.filter((t) => !prompts.includes(t))];
  };

  const loadSession = async (id: SessionId | null) => {
    if (!id) return;
    await attempt(async () => {
      const [questions, usage] = await Promise.all([feed.questions(id), feed.usage(id)]);
      const usages = { ...state.usages };
      if (usage) usages[id] = usage;
      change({ questions: { ...state.questions, [id]: questions }, usages });
    });
  };

  const onUpdate = (u: FeedUpdate) => {
    if (u.type === "question") {
      const list = (state.questions[u.session] ?? []).filter((q) => q.id !== u.question.id);
      change({ questions: { ...state.questions, [u.session]: [...list, u.question] } });
    } else if (u.type === "answered")
      change({
        questions: {
          ...state.questions,
          [u.session]: (state.questions[u.session] ?? []).filter((q) => q.id !== u.question),
        },
        answering: state.answering.filter((a) => a !== u.question),
      });
    else if (u.type === "usage") change({ usages: { ...state.usages, [u.session]: u.usage } });
    else if (u.type === "engines") change({ catalog: u.catalog });
    else if (u.type === "permissionSettings") change({ skipAll: u.skipAll });
  };
  const feedSubscription = feed.subscribe(onUpdate);
  let seen = agent.state.current;
  const agentSubscription = agent.subscribe(() => {
    if (agent.state.current !== seen) {
      seen = agent.state.current;
      change({ menu: null, queue: false, history: null });
      void track(loadSession(seen));
    } else for (const l of [...listeners]) l();
  });

  const fetchFiles = (menu: Extract<ComposerMenu, { type: "mention" }>) =>
    track(
      attempt(async () => {
        const files = await feed.files(menu.query, menu.extensions ?? undefined);
        const now = state.menu;
        if (now?.type === "mention" && now.query === menu.query && now.start === menu.start)
          change({ menu: { ...now, files } });
      }),
    );

  const mentionItems = (m: Extract<ComposerMenu, { type: "mention" }>): Mention[] => [
    ...m.files.map((f) => ({ kind: "file" as const, label: f, value: f })),
    ...(m.extensions ? [] : deps.documentMentions(m.query)),
  ];
  const skillMatches = (query: string) => {
    const q = query.toLowerCase();
    return state.skills.filter((s) => s.name.toLowerCase().includes(q));
  };
  const skillGroups = () => {
    const query = state.menu?.type === "skills" ? state.menu.query : "";
    const matches = skillMatches(query);
    return GROUPS.map(([group, label]) => ({
      id: group,
      group: label,
      skills: matches.filter((s) => s.group === group),
    })).filter((g) => g.skills.length > 0);
  };
  /** The skills in menu order (grouped), which the arrows walk. */
  const orderedSkills = () => skillGroups().flatMap((g) => g.skills);

  const menuItems = (): { line: string; kind: string }[] => {
    const m = state.menu;
    if (!m) return [];
    if (m.type === "skills") return orderedSkills().map((s) => ({ line: s.name, kind: "skill" }));
    return mentionItems(m).map((i) => ({
      line: `${i.label} · ${KIND_WORDS[i.kind]}`,
      kind: i.kind,
    }));
  };

  const pick = (index: number) => {
    const m = state.menu;
    if (!m) return;
    if (m.type === "skills") {
      const s = orderedSkills()[index];
      if (!s) return;
      setDraft({ text: "" });
      addChip({ kind: "skill", label: `Skill · ${s.name}`, value: s.path });
    } else {
      const item = mentionItems(m)[index];
      if (!item) return;
      const text = draft().text;
      const inserted = `@${item.label} `;
      setDraft({ text: text.slice(0, m.start) + inserted + text.slice(m.end) });
      addChip({ kind: item.kind, label: item.label, value: item.value });
    }
    change({ menu: null });
  };

  const setText = (text: string, caret: number, extensions: readonly string[] | null = null) => {
    setDraft({ text });
    const before = text.slice(0, caret);
    const skill = SKILL.exec(text);
    const mention = MENTION.exec(before);
    let menu: ComposerMenu | null = null;
    if (skill) menu = { type: "skills", query: skill[1] ?? "", active: 0 };
    else if (mention) {
      const query = mention[2] ?? "";
      const prev = state.menu;
      menu = {
        type: "mention",
        query,
        start: caret - query.length - 1,
        end: caret,
        extensions: extensions ?? (prev?.type === "mention" ? prev.extensions : null),
        files: prev?.type === "mention" && prev.query === query ? prev.files : [],
        active: 0,
      };
    }
    change({ menu, history: null });
    if (menu?.type === "mention") void fetchFiles(menu);
  };

  const request = (
    text: string,
    list: ChipView[],
    queue: PromptRequest["queue"],
  ): PromptRequest => ({
    text,
    context: list
      .filter((c) => c.kind !== "skill")
      .map((c) => ({ kind: c.kind as ContextChip["kind"], label: c.label, value: c.value })),
    skills: list.filter((c) => c.kind === "skill").map((c) => c.value),
    parallel: { ...state.parallel },
    explore: state.explore.count ? { ...state.explore } : null,
    engine: state.choice ? { ...state.choice } : null,
    queue,
  });

  const ensureSession = async (): Promise<SessionId | null> => {
    if (!agent.state.current) await agent.newSession();
    return agent.state.current;
  };

  const dispatch = async (queue: PromptRequest["queue"]) => {
    const d = draft();
    const list = chips();
    const text = d.text.trim();
    const draftKey = key();
    change({ sending: true, queue: false });
    await attempt(async () => {
      const session = await ensureSession();
      if (!session) return;
      await feed.send(session, request(text, list, queue));
      if (text) sentTexts.push(text);
      if (state.choice) lastSent[session] = { ...state.choice };
      const drafts = { ...state.drafts };
      drafts[draftKey === "new" ? "new" : draftKey] = { text: "", chips: [], removed: d.removed };
      if (draftKey !== session) drafts[session] = { text: "", chips: [], removed: d.removed };
      change({ drafts, history: null, menu: null });
    });
    change({ sending: false });
  };

  const send = async () => {
    const d = draft();
    if (!d.text.trim() && !d.chips.length) return;
    if (state.sending) return;
    if (working()) {
      change({ queue: true, menu: null });
      return;
    }
    await dispatch(null);
  };

  const walkHistory = (step: -1 | 1): boolean => {
    const list = history();
    if (!list.length) return false;
    let i = state.history;
    if (step < 0) i = i === null ? list.length - 1 : Math.max(0, i - 1);
    else {
      if (i === null) return false;
      i = i + 1 >= list.length ? null : i + 1;
    }
    setDraft({ text: i === null ? "" : (list[i] ?? "") });
    change({ history: i, menu: null });
    return true;
  };

  const controller = {
    get state() {
      return state;
    },
    async load() {
      await attempt(async () => {
        const [skills, catalog, settings] = await Promise.all([
          feed.skills(),
          feed.engines(),
          feed.permissionSettings(),
        ]);
        change({ skills, catalog, skipAll: settings.skipAll });
      });
      seen = agent.state.current;
      await loadSession(seen);
    },
    /** Resolves once every request the composer started has answered. */
    async idle() {
      while (pending.size) await Promise.allSettled([...pending]);
    },
    draft,
    chips,
    placeholder: () =>
      deps.scope().selection ? copy.selectionPlaceholder : copy.composerPlaceholder,
    setText,
    /** Opens the @ picker narrowed to these file types (Design from a spec: `.md`). */
    mention(extensions: readonly string[]) {
      const text = draft().text;
      const sep = text && !/\s$/.test(text) ? " " : "";
      const next = `${text}${sep}@`;
      setText(next, next.length, extensions);
    },
    removeChip(k: string) {
      const d = draft();
      const auto = autoChip();
      if (auto?.key === k || chips().some((c) => c.key === k && c.auto))
        setDraft({ removed: [...d.removed, k] });
      else setDraft({ chips: d.chips.filter((c) => c.key !== k) });
    },
    attach(list: ContextChip[]) {
      for (const c of list) addChip(c);
    },
    menuItems,
    skillGroups,
    pick,
    setActive(index: number) {
      if (state.menu) change({ menu: { ...state.menu, active: index } });
    },
    closeMenu() {
      change({ menu: null });
    },
    async addSkill() {
      const skill = await attempt(() => feed.addSkill());
      if (skill) change({ skills: [...state.skills.filter((s) => s.path !== skill.path), skill] });
    },
    /** A key in the field. `true`: handled here, so the field must not act on it. */
    key(name: string, mods: { shift?: boolean }): boolean {
      const m = state.menu;
      if (m) {
        const count = menuItems().length;
        if (name === "ArrowDown" || name === "ArrowUp") {
          if (!count) return true;
          const step = name === "ArrowDown" ? 1 : -1;
          change({ menu: { ...m, active: (m.active + step + count) % count } });
          return true;
        }
        if (((name === "Enter" && !mods.shift) || name === "Tab") && count) {
          pick(m.active);
          return true;
        }
        if (name === "Escape") {
          change({ menu: null });
          return true;
        }
      }
      if (state.queue && name === "Escape") {
        change({ queue: false });
        return true;
      }
      if (name === "Enter" && !mods.shift) {
        void track(send());
        return true;
      }
      if (name === "ArrowUp" && (draft().text === "" || state.history !== null))
        return walkHistory(-1);
      if (name === "ArrowDown" && state.history !== null) return walkHistory(1);
      return false;
    },
    send,
    async resolveQueue(choice: "after" | "now") {
      change({ queue: false });
      if (choice === "now") {
        const session = agent.state.current;
        if (session) await attempt(() => feed.stop(session));
      }
      await dispatch(choice);
    },
    cancelQueue() {
      change({ queue: false });
    },
    /** Send, or Stop while any agent in this session works (with each agent when several do). */
    action(): { type: "send" } | { type: "stop"; agents: AgentId[] } {
      const agents = working();
      return agents ? { type: "stop", agents } : { type: "send" };
    },
    async stopAgent(id: AgentId) {
      const session = agent.state.current;
      if (session) await attempt(() => feed.stopAgent(session, id));
    },
    async stopAll() {
      const session = agent.state.current;
      if (session) await attempt(() => feed.stop(session));
    },
    setParallel(p: { count: number; mode: ParallelMode }) {
      change({ parallel: { count: Math.min(6, Math.max(1, p.count)), mode: p.mode } });
    },
    setExplore(e: { count: number; vary: ExploreVary }) {
      const count = e.count < 2 ? 0 : Math.min(6, e.count);
      change({ explore: { count, vary: e.vary } });
    },
    /** "1× Split", "3× Side by side". */
    parallelLabel: () =>
      `${state.parallel.count}× ${state.parallel.mode === "split" ? "Split" : "Side by side"}`,
    /** "Explore off", "Explore 4 · Style". */
    exploreLabel: () =>
      state.explore.count
        ? `Explore ${state.explore.count} · ${state.explore.vary === "layout" ? "Layout" : "Style"}`
        : "Explore off",

    // Engine and model.
    choose(choice: EngineChoice) {
      const effort = choice.effort ?? state.choice?.effort;
      change({ choice: { ...choice, ...(effort ? { effort } : {}) } });
    },
    setEffort(effort: Effort) {
      if (state.choice) change({ choice: { ...state.choice, effort } });
    },
    model() {
      const c = state.choice;
      if (c?.engine !== "builtin") return null;
      return (
        state.catalog?.builtin
          .find((p) => p.id === c.provider)
          ?.models.find((m) => m.id === c.model) ?? null
      );
    },
    /** "OpenRouter · deepseek/deepseek-v4.1-flash", or the installed engine's name. */
    modelLine(): string | null {
      const c = state.choice;
      if (!c) return null;
      if (c.engine !== "builtin") {
        const name = state.catalog?.installed.find((e) => e.id === c.engine)?.name ?? c.engine;
        return c.model && c.model !== "default" ? `${name} · ${c.model}` : name;
      }
      const provider = state.catalog?.builtin.find((p) => p.id === c.provider);
      const model = provider?.models.find((m) => m.id === c.model);
      return `${provider?.name ?? c.provider ?? ""} · ${model?.label ?? c.model}`;
    },
    /** A new provider won't see earlier turns: say so when the open session already has turns. */
    switchWarning(): boolean {
      const session = agent.state.current;
      if (!session || !agent.turns().length) return false;
      const before = lastSent[session] ?? deps.defaultEngine;
      return !!before && !!state.choice && !sameProvider(before, state.choice);
    },
    providerStatus: (id: string): ProviderStatusName | null =>
      state.catalog?.builtin.find((p) => p.id === id)?.status ?? null,
    /** The key is handed straight to the daemon and forgotten here. */
    async saveProvider(setup: ProviderSetup): Promise<ProviderStatusName | null> {
      const status = await attempt(() => feed.saveProvider(setup));
      if (status) await controller.reloadEngines(setup.provider, status);
      return status ?? null;
    },
    async recheck(provider: string) {
      const status = await attempt(() => feed.recheckProvider(provider));
      if (status) await controller.reloadEngines(provider, status);
    },
    async clearKey(provider: string) {
      const status = await attempt(() => feed.clearKey(provider));
      if (status) await controller.reloadEngines(provider, status);
    },
    async reloadEngines(provider?: string, status?: ProviderStatusName) {
      const catalog = (await attempt(() => feed.engines())) ?? state.catalog;
      if (!catalog) return;
      if (provider && status)
        for (const p of catalog.builtin) if (p.id === provider) p.status = status;
      change({ catalog });
    },

    // Permission card.
    questionFor(turn: TurnId | null): PermissionQuestion | null {
      const session = agent.state.current;
      if (!session) return null;
      return (state.questions[session] ?? []).find((q) => q.turn === turn) ?? null;
    },
    answering: (id: string) => state.answering.includes(id),
    async answer(id: string, answer: PermissionAnswer) {
      const session = agent.state.current;
      if (!session || state.answering.includes(id)) return;
      change({ answering: [...state.answering, id] });
      const ok = await attempt(async () => {
        await feed.answer(session, id, answer);
        return true;
      });
      if (!ok) change({ answering: state.answering.filter((a) => a !== id) });
    },
    async turnPromptsOn() {
      const ok = await attempt(async () => {
        await feed.setSkipAll(false);
        return true;
      });
      if (ok) change({ skipAll: false });
    },

    // Usage.
    usage(): SessionUsage | null {
      const session = agent.state.current;
      return session ? (state.usages[session] ?? null) : null;
    },

    // Inline prompt.
    openInline() {
      if (!deps.scope().selection) return;
      change({ inline: { phase: "edit", text: "" } });
    },
    setInlineText(text: string) {
      if (state.inline?.phase === "edit") change({ inline: { phase: "edit", text } });
    },
    closeInline() {
      change({ inline: null });
    },
    async sendInline() {
      const inline = state.inline;
      const scope = deps.scope();
      if (inline?.phase !== "edit" || !inline.text.trim() || !scope.selection) return;
      const chip: ChipView = {
        key: "inline",
        kind: "selection",
        label: scope.selection.label,
        value: scope.selection.ids.join(","),
        auto: true,
      };
      await attempt(async () => {
        const session = await ensureSession();
        if (!session) return;
        const req = request(inline.text.trim(), [chip], working() ? "after" : null);
        const { turn } = await feed.send(session, {
          ...req,
          parallel: { count: 1, mode: "split" },
          explore: null,
        });
        sentTexts.push(inline.text.trim());
        change({ inline: { phase: "sent", session, turn, text: inline.text.trim() } });
      });
    },
    /** The inline prompt's progress line after sending. */
    inlineLine(): string | null {
      const inline = state.inline;
      if (inline?.phase !== "sent") return null;
      const t =
        agent.state.current === inline.session
          ? agent.turns().find((x) => x.id === inline.turn)
          : null;
      if (t && TERMINAL.includes(t.state)) {
        if (t.state === "stopped") return guardReasons.stopped;
        if (t.state === "failed") return "Failed · see the Agent panel";
        return t.footerLines[0] ? `Done · ${t.footerLines[0]}` : "Done";
      }
      const step = t?.items
        .flatMap((i) => (i.type === "step" ? [i.step] : []))
        .reverse()
        .find((s) => s.state === "working");
      return step?.line ?? "Refining selected layers…";
    },
    async undoInline() {
      const inline = state.inline;
      if (inline?.phase !== "sent") return;
      await attempt(() => feed.undoTurn(inline.session, inline.turn));
      change({ inline: null });
    },

    requestFocus() {
      change({ focus: state.focus + 1 });
    },
    open(surface: ComposerSurface | null) {
      change({ surface, menu: null });
    },
    clearError() {
      change({ error: null });
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
    dispose() {
      feedSubscription.dispose();
      agentSubscription.dispose();
      listeners.clear();
    },
  };
  return controller;
}
export type ComposerController = ReturnType<typeof createComposerController>;
