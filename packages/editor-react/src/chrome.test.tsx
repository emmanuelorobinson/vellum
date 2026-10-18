// TEST DATA: Juniper names are fixtures for the chrome components.
import { copy } from "@vellum/config";
import { layerIcon } from "@vellum/editor-core";
import type { Node } from "@vellum/types-ts";
import { renderToStaticMarkup as html } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ActorMarker } from "./chrome/ActorMarker";
import { BottomStrip, connectionLabel } from "./chrome/BottomStrip";
import { DocumentBar } from "./chrome/DocumentBar";
import { shortcutLabel } from "./chrome/shortcuts";
import { RAIL, SHAPE_MENU, ToolRail } from "./chrome/ToolRail";
import { TipProvider } from "./chrome/Tooltip";
import { TopRightBar } from "./chrome/TopRightBar";
import { ZoomPill } from "./chrome/ZoomPill";
import { hasIcon, Icon } from "./icons/Icon";

const teal = { id: "t", kind: "agent", name: "Teal", hue: "teal" } as const;
const sam = { id: "s", kind: "person", name: "Sam" } as const;

describe("icons", () => {
  it("has every icon the chrome and the layer rows can ask for", () => {
    const names = new Set<string>([
      ...RAIL.map((r) => r.icon),
      ...SHAPE_MENU.map((r) => r.icon),
      "message-square",
      "keyboard",
      "menu",
      "git-branch",
      "play",
      "settings",
      "chevron-down",
      "chevron-right",
      "eye",
      "eye-off",
      "lock",
      "lock-open",
      "file",
      "layers",
      "component",
      "history",
      "search",
      "triangle-alert",
      "library",
      "folder",
      "palette",
      "layout-template",
      "shapes",
    ]);
    const kinds = [
      "frame",
      "group",
      "rect",
      "ellipse",
      "polygon",
      "star",
      "line",
      "vector",
      "boolean",
      "text",
      "image",
      "icon",
      "instance",
      "script",
      "note",
      "context",
      "prompt",
      "embed",
      "conflict",
    ];
    for (const type of kinds) names.add(layerIcon({ type, id: "x" } as unknown as Node, false));
    for (const layout of ["row", "column", "grid"])
      names.add(layerIcon({ type: "frame", id: "x", layout } as unknown as Node, false));
    names.add(layerIcon({ type: "frame", id: "x", slot: {} } as unknown as Node, false));
    names.add(layerIcon({ type: "frame", id: "x" } as unknown as Node, true));
    expect([...names].filter((n) => !hasIcon(n))).toEqual([]);
  });
  it("draws at the design's 1.5 px stroke at any size, and says nothing when decorative", () => {
    const svg = html(<Icon name="frame" size={18} />);
    expect(svg).toContain('width="18"');
    expect(svg).toContain('stroke-width="1.5"');
    expect(svg).toContain("non-scaling-stroke");
    expect(svg).toContain('aria-hidden="true"');
    expect(html(<Icon name="frame" label="Frame" />)).toContain('aria-label="Frame"');
  });
  it("marks an unknown icon instead of drawing a wrong one", () => {
    expect(html(<Icon name="no-such-icon" />)).toContain("data-missing");
  });
});

describe("actor marker", () => {
  it("draws an agent as a rounded square in its hue and says who and what state in words", () => {
    const out = html(<ActorMarker actor={{ ...teal, state: "working" }} size={20} />);
    expect(out).toContain('aria-label="Teal · working"');
    expect(out).toContain("border-radius:6px");
    expect(out).toContain("var(--agent-teal)");
  });
  it("draws a person as a circle", () => {
    const out = html(<ActorMarker actor={sam} size={16} />);
    expect(out).toContain("border-radius:50%");
    expect(out).not.toContain("--agent-");
  });
});

describe("document bar", () => {
  const bar = (saveState: "saved" | "saving" | "unsaved") =>
    html(
      <TipProvider>
        <DocumentBar fileName="juniper.vellum" saveState={saveState} width={360} commands={[]} />
      </TipProvider>,
    );
  it("shows the name and the save state in words", () => {
    expect(bar("saved")).toContain("Saved");
    expect(bar("saving")).toContain("Saving…");
    expect(bar("unsaved")).toContain("Unsaved");
    expect(bar("saved")).toContain("juniper.vellum");
    expect(bar("saved")).toContain('data-state="saved"');
  });
  it("is as wide as the panel it sits over", () => {
    expect(bar("saved")).toContain("width:360px");
  });
});

describe("bottom strip", () => {
  const strip = (over: Partial<Parameters<typeof BottomStrip>[0]> = {}) =>
    html(
      <BottomStrip
        centerX={640}
        y={760}
        branch="feat/onboarding"
        dirty={false}
        connection={{ mode: "live", agents: 3 }}
        basicRenderer={false}
        {...over}
      />,
    );
  it("says the connection mode in words", () => {
    expect(connectionLabel({ mode: "live", agents: 1 })).toBe("Live · 1 agent");
    expect(connectionLabel({ mode: "live", agents: 3 })).toBe("Live · 3 agents");
    expect(connectionLabel({ mode: "headless" })).toBe("Headless only");
    expect(connectionLabel({ mode: "offline" })).toBe(copy.daemonOffline);
  });
  it("shows the branch, and a dirty marker only when there are uncommitted changes", () => {
    expect(strip()).toContain("feat/onboarding");
    expect(strip()).not.toContain("Uncommitted changes");
    expect(strip({ dirty: true })).toContain('aria-label="Uncommitted changes"');
  });
  it("leaves the branch out when the folder is not a repository", () => {
    expect(strip({ branch: null })).not.toContain("git-branch");
  });
  it("marks an offline daemon as needing attention and shows its note", () => {
    const out = strip({
      connection: { mode: "offline", note: "Editing and saving available" },
    });
    expect(out).toContain(copy.daemonOffline);
    expect(out).toContain("Editing and saving available");
    expect(out).toContain("data-attention");
  });
  it("names the basic renderer when it is in use", () => {
    expect(strip({ basicRenderer: true })).toContain(copy.basicRenderer);
    expect(strip()).not.toContain(copy.basicRenderer);
  });
});

describe("tool rail", () => {
  const rail = (tool: Parameters<typeof ToolRail>[0]["tool"], enabledCommands: string[] = []) =>
    html(
      <TipProvider>
        <ToolRail
          x={376}
          y={56}
          tool={tool}
          selectTool={() => {}}
          run={() => {}}
          enabled={(c) => enabledCommands.includes(c)}
          shortcut={() => null}
        />
      </TipProvider>,
    );
  it("lists the design's tools in order, then Ask and the shortcuts", () => {
    const out = rail("move");
    const order = [
      "Move",
      "Frame",
      "Shapes",
      "Pen",
      "Text",
      "Image",
      "Hand",
      "Ask",
      "Keyboard shortcuts",
    ].map((n) => out.indexOf(`aria-label="${n}"`));
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });
  it("shows the active tool as pressed and the others not", () => {
    const out = rail("frame");
    expect(out).toMatch(/aria-label="Frame"[^>]*aria-pressed="true"/);
    expect(out).toMatch(/aria-label="Move"[^>]*aria-pressed="false"/);
  });
  it("shows the current shape on the Rectangle slot while another shape is active", () => {
    expect(rail("ellipse")).toContain('data-icon="circle"');
    expect(rail("ellipse")).toMatch(/aria-label="Shapes"[^>]*aria-pressed="true"/);
    expect(rail("move")).toContain('data-icon="square"');
  });
  it("disables actions nothing handles instead of pretending", () => {
    expect(rail("move")).toMatch(/aria-label="Ask"[^>]*aria-disabled="true"/);
    expect(rail("move", ["agent.inline"])).not.toMatch(/aria-label="Ask"[^>]*aria-disabled/);
  });
});

describe("top-right bar", () => {
  it("stacks the active agents and hides Share when nobody can share", () => {
    const out = html(
      <TipProvider>
        <TopRightBar
          right={8}
          agents={[teal, { ...teal, id: "c", name: "Claude Code", hue: "saffron" }]}
        />
      </TipProvider>,
    );
    expect(out).toContain('aria-label="Teal"');
    expect(out).toContain('aria-label="Claude Code"');
    expect(out).not.toContain(">Share<");
    expect(out).toContain('aria-label="Editor settings"');
    expect(out).toContain('aria-label="Preview prototype"');
  });
  it("shows Share when it is on offer", () => {
    expect(
      html(
        <TipProvider>
          <TopRightBar right={8} agents={[]} onShare={() => {}} />
        </TipProvider>,
      ),
    ).toContain(">Share<");
  });
});

describe("zoom pill", () => {
  it("shows the zoom as a whole percentage with a menu", () => {
    const out = html(<ZoomPill x={1212} y={760} label="50%" items={[]} />);
    expect(out).toContain("50%");
    expect(out).toContain('aria-label="Zoom 50%"');
  });
});

describe("shortcut labels", () => {
  const mac = { platform: "mac", host: "desktop" } as const;
  it("writes Mac chords with symbols in the usual order", () => {
    expect(shortcutLabel("agent.focus", mac)).toBe("⌘L");
    expect(shortcutLabel("component.toggle", mac)).toBe("⌥⌘K");
    expect(shortcutLabel("edit.redo", mac)).toBe("⇧⌘Z");
    expect(shortcutLabel("tool.frame", mac)).toBe("F");
    expect(shortcutLabel("view.togglePanels", mac)).toBe("⌘\\");
    expect(shortcutLabel("panel.layers", mac)).toBe("⌥2");
  });
  it("writes Windows and Linux chords with names", () => {
    expect(shortcutLabel("agent.focus", { platform: "windows", host: "desktop" })).toBe("Ctrl+L");
    expect(shortcutLabel("edit.redo", { platform: "linux", host: "browser" })).toBe("Ctrl+Shift+Z");
  });
  it("uses the IDE chord where VS Code owns the desktop one", () => {
    expect(shortcutLabel("agent.inline", { platform: "mac", host: "vscode" })).toBe("⌥K");
    expect(shortcutLabel("agent.inline", mac)).toBe("⌘K");
  });
  it("has no label for a command without a key", () => {
    expect(shortcutLabel("tool.polygon", mac)).toBeNull();
  });
});
