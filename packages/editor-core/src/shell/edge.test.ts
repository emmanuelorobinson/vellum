// TEST DATA: Juniper, Lora, IBM Plex Sans Arabic, Teal, OpenRouter and Claude Code.
import { copy } from "@vellum/config";
import type { Document } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  agentNotice,
  composerLine,
  createEdgeUi,
  type EdgeEnvironment,
  edgeBanner,
  formatReset,
  missingFonts,
  PANELS,
  type PanelId,
  panelView,
  sessionProblem,
} from "./index";

const base = (extra: Partial<EdgeEnvironment> = {}): EdgeEnvironment => ({
  daemon: "live",
  renderer: "full",
  readOnly: null,
  unreadable: null,
  fonts: [],
  libraries: 0,
  layers: 40,
  problem: null,
  closing: null,
  ...extra,
});
const fonts = [
  { family: "Lora", layers: 4, substitute: "Inter", nodes: ["a"] },
  { family: "IBM Plex Sans Arabic", layers: 2, substitute: "System Arabic", nodes: ["b"] },
];

describe("agent panel notices (S01–S10, S16)", () => {
  it("S01 daemon offline: names the cause and the way out, and says hand editing still works", () => {
    expect(agentNotice(base({ daemon: "offline" }))).toMatchObject({
      heading: copy.daemonOffline,
      body: "Agents need the Vellum daemon. Hand editing and saving still work.",
      action: { id: "restartDaemon", label: "Restart" },
    });
  });
  it("S02 basic renderer", () => {
    expect(agentNotice(base({ renderer: "basic" }))).toMatchObject({
      heading: copy.basicRenderer,
      body: "Effects are simplified on screen. Exports are unaffected.",
      action: { id: "rendererSettings", label: "Renderer settings" },
    });
  });
  it("S03 missing fonts counts them and offers the review", () => {
    expect(agentNotice(base({ fonts }))).toMatchObject({
      heading: "Missing fonts",
      body: "2 fonts are unavailable. Choose a replacement or install them.",
      action: { id: "reviewFonts", label: "Review fonts" },
    });
    expect(agentNotice(base({ fonts: fonts.slice(0, 1) }))?.body).toBe(
      "1 font is unavailable. Choose a replacement or install it.",
    );
  });
  it("S04 read-only newer format: Appendix B wording, Update Vellum", () => {
    const notice = agentNotice(base({ readOnly: { newerFormat: 3 } }));
    expect(notice).toMatchObject({
      heading: "Read-only",
      body: "Made with a newer version of Vellum (format 3). Update to edit.",
      action: { id: "updateVellum", label: "Update Vellum" },
    });
    expect(`${notice?.body.replace(". Update", " · Update")}`).toContain(
      copy.newerFormat.replace("{n}", "3").replace(" · Update to edit", ""),
    );
  });
  it("S05 unreadable file: plain words with line and column, Open as text, the spec link", () => {
    const notice = agentNotice(
      base({ unreadable: { line: 18, column: 7, reason: "Unexpected token" } }),
    );
    expect(notice).toMatchObject({
      heading: "File can't be read",
      body: "Unexpected token at line 18, column 7. No partial design was loaded.",
      action: { id: "openAsText", label: "Open as text" },
      link: { id: "formatSpec", label: "View the file format spec" },
    });
  });
  it("S06 large file never blocks: a quiet suggestion", () => {
    expect(agentNotice(base({ layers: 18_420, largeFile: true }))).toMatchObject({
      heading: "Large file",
      body: "This file has 18,420 layers. The multi-file format can make changes easier to review.",
      action: { id: "learnMultiFile", label: "Learn about multi-file" },
    });
  });
  it("S08 key failed names the provider, not the key, and says other sessions are unaffected", () => {
    const notice = agentNotice(base({ problem: { kind: "key-failed", provider: "OpenRouter" } }));
    expect(notice).toMatchObject({
      heading: "Engine failed",
      body: "OpenRouter rejected the key. Other sessions are unaffected.",
      action: { id: "updateKey", label: "Update key" },
      tone: "error",
    });
    expect(copy.keyFailed.replace("{provider}", "OpenRouter")).toBe(
      "OpenRouter rejected the key · Update key",
    );
  });
  it("S09 usage limit says when it resets and offers another engine", () => {
    const notice = agentNotice(
      base({
        problem: {
          kind: "usage-limit",
          engine: "Claude Code",
          resets: formatReset(Date.UTC(2026, 9, 11, 18, 0), "UTC"),
        },
      }),
    );
    expect(notice).toMatchObject({
      heading: "Claude Code reached its usage limit",
      body: "Resets at 18:00 · October 11",
      action: { id: "switchEngine", label: "Switch engine" },
      footnote: "Other sessions are unaffected.",
    });
  });
  it("S16 not signed in: the command to run and Re-check", () => {
    expect(
      agentNotice(
        base({ problem: { kind: "not-signed-in", engine: "Claude Code", command: "claude" } }),
      ),
    ).toMatchObject({
      heading: "Claude Code isn't signed in",
      body: "This session is paused. Your other sessions are unaffected.",
      command: "Run claude in a terminal to sign in.",
      action: { id: "openTerminal", label: "Open terminal" },
      secondary: [
        { id: "recheck", label: "Re-check" },
        { id: "switchEngine", label: "Switch engine" },
      ],
    });
  });
  it("S11 unsaved while agents work: names how many", () => {
    expect(
      agentNotice(base({ closing: { fileName: "juniper.vellum", running: 3 } })),
    ).toMatchObject({
      heading: "Unsaved changes",
      body: "3 agents are still working. Stop them and close?",
      action: { id: "reviewChanges", label: "Review changes" },
    });
    expect(agentNotice(base({ closing: { fileName: "juniper.vellum", running: 1 } }))?.body).toBe(
      "1 agent is still working. Stop it and close?",
    );
  });
  it("nothing wrong: no notice", () => {
    expect(agentNotice(base())).toBeNull();
  });
  it("one notice at a time, the most important first, and none uses an exclamation mark", () => {
    const everything = base({
      daemon: "offline",
      renderer: "basic",
      readOnly: { newerFormat: 3 },
      unreadable: { line: 1, column: 1, reason: "Unexpected token" },
      fonts,
      layers: 20_000,
      largeFile: true,
      problem: { kind: "key-failed", provider: "OpenRouter" },
      closing: { fileName: "juniper.vellum", running: 2 },
    });
    expect(agentNotice(everything)?.heading).toBe("File can't be read");
    expect(agentNotice({ ...everything, unreadable: null })?.heading).toBe("Unsaved changes");
    expect(agentNotice({ ...everything, unreadable: null, closing: null })?.heading).toBe(
      "Read-only",
    );
    expect(
      agentNotice({ ...everything, unreadable: null, closing: null, readOnly: null })?.heading,
    ).toBe(copy.daemonOffline);
    expect(
      agentNotice({
        ...everything,
        unreadable: null,
        closing: null,
        readOnly: null,
        daemon: "live",
      })?.heading,
    ).toBe("Engine failed");
    for (const state of [everything, { ...everything, unreadable: null }]) {
      const n = agentNotice(state);
      expect(`${n?.heading} ${n?.body}`).not.toContain("!");
    }
  });
});

describe("banners", () => {
  it("S04 attention banner with Update, Appendix B wording", () => {
    expect(edgeBanner(base({ readOnly: { newerFormat: 3 } }))).toMatchObject({
      tone: "attention",
      message: copy.newerFormat.replace("{n}", "3"),
      action: { id: "updateVellum", label: "Update" },
      dismissible: false,
    });
  });
  it("S02 neutral banner that can be dismissed, without hiding the strip's renderer note", () => {
    expect(edgeBanner(base({ renderer: "basic" }))).toMatchObject({
      tone: "neutral",
      message: "Effects are simplified on screen. Exports are unaffected.",
      action: { id: "dismiss", label: "Dismiss" },
      dismissible: true,
    });
    expect(edgeBanner(base({ renderer: "basic" }), { dismissed: ["renderer"] })).toBeNull();
  });
  it("S06 neutral banner with Learn more", () => {
    expect(edgeBanner(base({ layers: 18_420, largeFile: true }))).toMatchObject({
      message:
        "This file has 18,420 layers. The multi-file format can make changes easier to review.",
      action: { id: "learnMultiFile", label: "Learn more" },
    });
  });
  it("shows nothing for a healthy file", () => {
    expect(edgeBanner(base())).toBeNull();
  });
});

describe("composer line", () => {
  it("is disabled with the reason, in Appendix B words where it has them", () => {
    expect(composerLine(base({ daemon: "offline" }))).toEqual({
      text: copy.composerWhenOffline,
      action: "restartDaemon",
    });
    expect(composerLine(base({ readOnly: { newerFormat: 3 } }))).toEqual({
      text: "Update Vellum to edit format 3",
      action: "updateVellum",
    });
    expect(composerLine(base({ unreadable: { line: 1, column: 1, reason: "x" } }))).toEqual({
      text: "Open a readable file to start a session",
      action: "openAsText",
    });
    expect(composerLine(base())).toBeNull();
  });
});

describe("session problems", () => {
  const catalog = {
    builtin: [{ id: "openrouter", name: "OpenRouter", status: "failed" as const, models: [] }],
    installed: [
      { id: "claude-code", name: "Claude Code", state: "signed_out" as const, command: "claude" },
    ],
  };
  it("a failed provider is a key problem", () => {
    expect(
      sessionProblem({
        catalog,
        choice: { engine: "builtin", provider: "openrouter", model: "m" },
      }),
    ).toEqual({ kind: "key-failed", provider: "OpenRouter" });
  });
  it("a signed-out installed engine is a sign-in problem with its command", () => {
    expect(sessionProblem({ catalog, choice: { engine: "claude-code", model: "m" } })).toEqual({
      kind: "not-signed-in",
      engine: "Claude Code",
      command: "claude",
    });
  });
  it("a usage limit names when it resets, in the viewer's zone", () => {
    const limited = {
      ...catalog,
      installed: [
        {
          id: "claude-code",
          name: "Claude Code",
          state: "usage_limit" as const,
          resetsAt: Date.UTC(2026, 9, 11, 18, 0),
        },
      ],
    };
    expect(
      sessionProblem({
        catalog: limited,
        choice: { engine: "claude-code", model: "m" },
        timeZone: "UTC",
      }),
    ).toEqual({ kind: "usage-limit", engine: "Claude Code", resets: "18:00 · October 11" });
  });
  it("a healthy engine has no problem", () => {
    const ok = {
      ...catalog,
      builtin: [{ id: "openrouter", name: "OpenRouter", status: "connected" as const, models: [] }],
    };
    expect(
      sessionProblem({
        catalog: ok,
        choice: { engine: "builtin", provider: "openrouter", model: "m" },
      }),
    ).toBeNull();
    expect(sessionProblem({ catalog: null, choice: null })).toBeNull();
  });
});

describe("missing fonts", () => {
  const doc = {
    pages: [
      {
        id: "p1",
        name: "Onboarding",
        children: [
          { type: "text", id: "a", content: [{ text: "x", font: "Lora" }] },
          { type: "text", id: "b", content: [{ text: "y", font: "Lora" }] },
          { type: "text", id: "c", content: [{ text: "z", font: "Inter" }] },
        ],
      },
    ],
    fonts: {},
  } as unknown as Document;
  it("lists families the computer does not have, with their layers and the substitute", () => {
    expect(missingFonts(doc, ["Inter"], "Inter")).toEqual([
      { family: "Lora", layers: 2, substitute: "Inter", nodes: ["a", "b"] },
    ]);
  });
  it("a font stored in the file is never missing; unknown availability reports nothing", () => {
    const stored = { ...doc, fonts: { f: { family: "Lora" } } } as unknown as Document;
    expect(missingFonts(stored, ["Inter"], "Inter")).toEqual([]);
    expect(missingFonts(doc, undefined, "Inter")).toEqual([]);
  });
});

/**
 * The global state matrix: every panel in every state shows the exact copy, or says plainly that it is
 * unchanged. A cell that is neither is a bug.
 */
describe("global state matrix", () => {
  const STATES = [
    "loading",
    "empty",
    "offline daemon",
    "read-only",
    "basic renderer",
    "missing fonts",
    "missing library",
  ] as const;
  const envFor = (
    state: (typeof STATES)[number],
  ): [EdgeEnvironment, { loading: boolean; empty: boolean }] => {
    const content = { loading: state === "loading", empty: state === "empty" };
    const env = base(
      state === "offline daemon"
        ? { daemon: "offline" }
        : state === "read-only"
          ? { readOnly: { newerFormat: 3 } }
          : state === "basic renderer"
            ? { renderer: "basic" }
            : state === "missing fonts"
              ? { fonts }
              : state === "missing library"
                ? { libraries: 1 }
                : {},
    );
    return [env, content];
  };
  const expected: Record<PanelId, Record<(typeof STATES)[number], unknown>> = {
    agent: {
      loading: { kind: "loading", text: "Loading sessions" },
      empty: { kind: "content", disabled: false },
      "offline daemon": { kind: "notice", heading: copy.daemonOffline },
      "read-only": { kind: "notice", heading: "Read-only" },
      "basic renderer": { kind: "notice", heading: copy.basicRenderer },
      "missing fonts": { kind: "notice", heading: "Missing fonts" },
      "missing library": { kind: "content", disabled: false },
    },
    layers: {
      loading: { kind: "loading", text: "Loading layers" },
      empty: { kind: "empty", text: copy.layersEmpty },
      "offline daemon": { kind: "content", disabled: false },
      "read-only": { kind: "content", disabled: true },
      "basic renderer": { kind: "content", disabled: false },
      "missing fonts": { kind: "content", disabled: false },
      "missing library": { kind: "content", disabled: false },
    },
    assets: {
      loading: { kind: "loading", text: "Loading assets" },
      empty: { kind: "empty", text: "No components or libraries yet" },
      "offline daemon": { kind: "content", disabled: false },
      "read-only": { kind: "content", disabled: true },
      "basic renderer": { kind: "content", disabled: false },
      "missing fonts": { kind: "content", disabled: false },
      "missing library": { kind: "content", disabled: false, flag: "missing-library" },
    },
    history: {
      loading: { kind: "loading", text: "Loading history" },
      empty: { kind: "empty", text: copy.historyEmpty },
      "offline daemon": { kind: "content", disabled: false },
      "read-only": { kind: "content", disabled: true },
      "basic renderer": { kind: "content", disabled: false },
      "missing fonts": { kind: "content", disabled: false },
      "missing library": { kind: "content", disabled: false },
    },
    design: {
      loading: { kind: "loading", text: "Loading properties" },
      empty: { kind: "empty", text: "Select a layer to see its properties." },
      "offline daemon": { kind: "content", disabled: false },
      "read-only": { kind: "content", disabled: true },
      "basic renderer": { kind: "content", disabled: false },
      "missing fonts": { kind: "content", disabled: false, flag: "missing-fonts" },
      "missing library": { kind: "content", disabled: false, flag: "missing-library" },
    },
  };
  for (const panel of PANELS)
    for (const state of STATES)
      it(`${panel} · ${state}`, () => {
        const [env, content] = envFor(state);
        expect(panelView(panel, env, content)).toMatchObject(expected[panel][state] as object);
      });
  it("covers every panel and every state", () => {
    expect(PANELS).toEqual(["agent", "layers", "assets", "history", "design"]);
    for (const panel of PANELS) expect(Object.keys(expected[panel])).toEqual([...STATES]);
  });
  it("no copy in any cell uses emoji, an exclamation mark or the word magic", () => {
    for (const panel of PANELS)
      for (const state of STATES) {
        const [env, content] = envFor(state);
        const text = JSON.stringify(panelView(panel, env, content));
        expect(text).not.toMatch(/!|magic|stunning|beautiful|\p{Extended_Pictographic}/u);
      }
  });
});

describe("edge surfaces the person closed", () => {
  it("stay closed until the cause clears and returns", () => {
    const ui = createEdgeUi();
    ui.dismiss("renderer");
    ui.dismiss("renderer");
    expect(ui.state.dismissed).toEqual(["renderer"]);
    expect(edgeBanner(base({ renderer: "basic" }), { dismissed: ui.state.dismissed })).toBeNull();
    ui.restore("renderer");
    expect(
      edgeBanner(base({ renderer: "basic" }), { dismissed: ui.state.dismissed }),
    ).not.toBeNull();
  });
  it("opens and closes the Missing fonts dialog and tells subscribers", () => {
    const ui = createEdgeUi();
    let n = 0;
    ui.subscribe(() => n++);
    ui.openFonts();
    expect(ui.state.fonts).toBe(true);
    ui.closeFonts();
    expect(ui.state.fonts).toBe(false);
    expect(n).toBe(2);
  });
});
