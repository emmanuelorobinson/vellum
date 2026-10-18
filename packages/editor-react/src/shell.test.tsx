// TEST DATA: Juniper, Teal, OpenRouter and Claude Code.
import { copy } from "@vellum/config";
import { agentNotice, type EdgeEnvironment, edgeBanner } from "@vellum/editor-core";
import { renderToStaticMarkup as html } from "react-dom/server";
import { expect, it } from "vitest";
import { AssetsTab } from "./panels/AssetsTab";
import { ContextMenu } from "./shell/ContextMenu";
import { AgentNotice, EdgeBannerView } from "./shell/EdgeLayer";

const env = (extra: Partial<EdgeEnvironment> = {}): EdgeEnvironment => ({
  daemon: "live",
  renderer: "full",
  readOnly: null,
  unreadable: null,
  fonts: [],
  libraries: 0,
  layers: 10,
  problem: null,
  closing: null,
  ...extra,
});
const teal = { id: "teal", kind: "agent" as const, name: "Teal", hue: "teal" };
const notice = (e: EdgeEnvironment) =>
  html(
    <AgentNotice
      notice={agentNotice(e) as NonNullable<ReturnType<typeof agentNotice>>}
      actor={teal}
      model="deepseek/deepseek-v4.1-flash"
      run={() => {}}
    />,
  );

it("a notice names the agent and its state, the model, the cause and the one next step", () => {
  const out = notice(env({ daemon: "offline" }));
  expect(out).toContain("Teal");
  expect(out).toContain("Unavailable");
  expect(out).toContain("deepseek/deepseek-v4.1-flash");
  expect(out).toContain(copy.daemonOffline);
  expect(out).toContain("Agents need the Vellum daemon. Hand editing and saving still work.");
  expect(out).toContain(">Restart<");
  expect(out).toMatch(/data-actor="agent"[^>]*data-hue="teal"/);
});
it("an error is marked by tone and by its words, never by colour alone", () => {
  const out = notice(env({ problem: { kind: "key-failed", provider: "OpenRouter" } }));
  expect(out).toContain('data-tone="error"');
  expect(out).toContain("Failed");
  expect(out).toContain("Engine failed");
  expect(out).not.toContain("!");
});
it("sign-in shows the command in mono and Open terminal, Re-check, Switch engine", () => {
  const out = notice(
    env({ problem: { kind: "not-signed-in", engine: "Claude Code", command: "claude" } }),
  );
  expect(out).toContain("Run claude in a terminal to sign in.");
  for (const label of ["Open terminal", "Re-check", "Switch engine"])
    expect(out).toContain(`>${label}<`);
});
it("the unreadable file's notice links to the format spec", () => {
  const out = notice(env({ unreadable: { line: 18, column: 7, reason: "Unexpected token" } }));
  expect(out).toContain("Unexpected token at line 18, column 7. No partial design was loaded.");
  expect(out).toContain("View the file format spec");
});
it("a banner carries its message and action and says whether it can be closed", () => {
  const banner = edgeBanner(env({ readOnly: { newerFormat: 3 } }));
  const out = html(
    <EdgeBannerView
      banner={banner as NonNullable<typeof banner>}
      top={44}
      inset={420}
      run={() => {}}
    />,
  );
  expect(out).toContain("Made with a newer version of Vellum (format 3) · Update to edit");
  expect(out).toContain('data-tone="attention"');
  expect(out).toContain(">Update<");
  expect(out).toContain("padding-inline-start:420px");
});

const menu = (readOnly = false) =>
  html(
    <ContextMenu
      menu={{ x: 640, y: 164, ids: ["step2"] }}
      enabled={() => true}
      shortcut={(c) =>
        ({ "edit.copy": "⌘C", "edit.group": "⌘G", "edit.autoLayout": "⇧A" })[c] ?? null
      }
      run={() => {}}
      onClose={() => {}}
      size={{ width: 1280, height: 800 }}
      mac
      readOnly={readOnly}
    />,
  );
it("the context menu lists the design's rows, with shortcuts in mono and Delete marked", () => {
  const out = menu();
  expect(out.indexOf("Ask about this…")).toBeLessThan(out.indexOf("Redesign this…"));
  expect(out.indexOf("Redesign this…")).toBeLessThan(out.indexOf(">Copy<"));
  expect(out.indexOf(">Group<")).toBeLessThan(out.indexOf("Create component"));
  expect(out.lastIndexOf(">Delete<")).toBeGreaterThan(out.indexOf("Copy node ID"));
  expect(out).toContain("⌘C");
  expect(out).toContain("⌫");
  expect(out).toMatch(/data-destructive[^>]*>.*Delete/);
  expect(out).toContain('role="menu"');
});
it("a read-only file greys out the changes in the menu", () => {
  expect(menu(true)).toMatch(/aria-disabled="true"[^>]*>(?:(?!<\/div>).)*Delete/s);
  expect(menu(false)).not.toMatch(/aria-disabled="true"/);
});

it("Assets says it is empty when there are no components and no libraries", () => {
  const out = html(
    <AssetsTab
      components={{}}
      libraries={[]}
      iconSets={[]}
      projectIconFolder={null}
      visualStyles={[]}
      designSystems={[]}
      readOnly={false}
      onDropComponent={() => {}}
    />,
  );
  expect(out).toContain("No components or libraries yet");
});
