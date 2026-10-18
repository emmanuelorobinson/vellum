// TEST DATA: Prompt 48 surfaces over a scripted engine. No filesystem, network or document engine.
import type { Meta, StoryObj } from "@storybook/react-vite";
import { createResourceController } from "@vellum/editor-core";
import type { EditablePath } from "@vellum/engine-client";
import { useState } from "react";
import { resourceDocument, resourceHost } from "../fixtures/resource-scenarios";
import { PortalContainer } from "./chrome/portal";
import { TipProvider } from "./chrome/Tooltip";
import { designFixture } from "./design-fixture";
import { ResourceDialogs } from "./resources/ResourceDialogs";
import { ScalePanel, ThemeSection, VectorPanel } from "./resources/ToolPanels";
import "./editor.css";

const meta = {
  title: "Editor/Variables and resources",
  parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj;
function Surface({
  screen,
  appearance = "light",
  readOnly = false,
}: {
  screen: string;
  appearance?: "light" | "dark";
  readOnly?: boolean;
}) {
  const [f] = useState(() => {
    const fixture = designFixture("c13", {
      change: (d) => Object.assign(d, resourceDocument(screen)),
      selection: [screen === "vector" ? "vector" : "card"],
      readOnly,
    });
    fixture.engine.scene.path = (id) =>
      (fixture.engine.scene.get(id)?.node as { metadata?: { testPath?: EditablePath } } | undefined)
        ?.metadata?.testPath ?? null;
    const r = createResourceController({
      engine: fixture.engine,
      device: fixture.session.device,
      host: resourceHost,
      nextVariableId: () => "story-variable",
    });
    if (["variables", "themes", "libraries", "fonts", "manageFonts"].includes(screen))
      r.open(screen as "variables");
    if (screen === "paste" || screen === "conflicts")
      r.requestPaste({
        id: "clipboard",
        ...(screen === "paste" ? { libraryPath: "../libraries/juniper.lib.vellum" } : {}),
        conflicts:
          screen === "conflicts"
            ? [{ id: "space", name: "space/4", existing: "16", pasted: "20", sameType: true }]
            : [],
        omissions: ["images/hero.png (missing)"],
      });
    if (screen === "convert") r.requestConversion();
    if (screen === "scale") {
      fixture.session.dispatch({ type: "tool", tool: "scale" });
      fixture.session.dispatch({ type: "scaleFactor", factor: "200%" });
    }
    if (screen === "vector") fixture.session.dispatch({ type: "tool", tool: "path" });
    return { ...fixture, resources: r };
  });
  const [container, setContainer] = useState<HTMLElement | null>(null);
  return (
    <div
      ref={setContainer}
      className="vc-workspace"
      data-appearance={appearance}
      style={{ width: 1280, height: 800 }}
    >
      <PortalContainer.Provider value={container}>
        <TipProvider>
          <aside
            className="vc-resource-panel"
            style={{ position: "absolute", right: 8, top: 56, width: 280, height: 736 }}
          >
            {screen === "frame" ? (
              <ThemeSection
                design={f.design}
                session={f.session}
                document={f.doc}
                disabled={readOnly}
              />
            ) : screen === "scale" ? (
              <ScalePanel session={f.session} disabled={readOnly} />
            ) : screen === "vector" ? (
              <VectorPanel session={f.session} design={f.design} disabled={readOnly} />
            ) : null}
          </aside>
          <ResourceDialogs
            resources={f.resources}
            design={f.design}
            session={f.session}
            readOnly={readOnly}
          />
        </TipProvider>
      </PortalContainer.Provider>
    </div>
  );
}
export const Variables: Story = { render: () => <Surface screen="variables" /> };
export const VariablesDark: Story = {
  render: () => <Surface screen="variables" appearance="dark" />,
};
export const VariablesReadOnly: Story = { render: () => <Surface screen="variables" readOnly /> };
export const ThemeAxes: Story = { render: () => <Surface screen="themes" /> };
export const ThemeAxesDark: Story = { render: () => <Surface screen="themes" appearance="dark" /> };
export const FrameTheme: Story = { render: () => <Surface screen="frame" /> };
export const Libraries: Story = { render: () => <Surface screen="libraries" /> };
export const LibrariesDark: Story = {
  render: () => <Surface screen="libraries" appearance="dark" />,
};
export const LibraryConfirmation: Story = { render: () => <Surface screen="convert" /> };
export const FontPicker: Story = { render: () => <Surface screen="fonts" /> };
export const ManageFonts: Story = { render: () => <Surface screen="manageFonts" /> };
export const PasteLibrary: Story = { render: () => <Surface screen="paste" /> };
export const PasteConflicts: Story = { render: () => <Surface screen="conflicts" /> };
export const Scale: Story = { render: () => <Surface screen="scale" /> };
export const Vector: Story = { render: () => <Surface screen="vector" /> };
