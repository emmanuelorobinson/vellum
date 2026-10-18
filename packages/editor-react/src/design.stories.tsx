// TEST DATA: the properties panel over the Juniper scenarios of design screens C13, C14, C16, C33.
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { Document, Node } from "@vellum/types-ts";
import type { DesignScenario } from "../fixtures/design-scenarios";
import { PortalContainer } from "./chrome/portal";
import { TipProvider } from "./chrome/Tooltip";
import { DesignTab } from "./design/DesignTab";
import { designFixture } from "./design-fixture";
import { RightPanel } from "./panels/RightPanel";
import "./editor.css";
import { useState } from "react";

const meta = {
  title: "Editor/Properties panel",
  parameters: { layout: "centered" },
} satisfies Meta;
export default meta;
type Story = StoryObj;

/** The right panel at the design's size, with its own appearance so menus and popovers follow it. */
function Panel(props: {
  scenario: DesignScenario;
  appearance?: "light" | "dark" | "hc-light" | "hc-dark";
  options?: Parameters<typeof designFixture>[1];
}) {
  const [fixture] = useState(() => designFixture(props.scenario, props.options));
  const [container, setContainer] = useState<HTMLElement | null>(null);
  return (
    <div
      ref={setContainer}
      className="vc-workspace"
      data-appearance={props.appearance ?? "light"}
      data-canvas-appearance={props.appearance ?? "light"}
      style={{ width: 296, height: 736, position: "relative" }}
    >
      <PortalContainer.Provider value={container}>
        <TipProvider>
          <aside className="vc-bar vc-right" style={{ left: 8, top: 0, width: 280, height: 736 }}>
            <RightPanel
              design={
                <DesignTab
                  design={fixture.design}
                  session={fixture.session}
                  paintEditor={fixture.paintEditor}
                  document={fixture.doc}
                  readOnly={fixture.readOnly}
                  host={{ fonts: fixture.fonts, onAction: () => {} }}
                />
              }
            />
          </aside>
        </TipProvider>
      </PortalContainer.Provider>
    </div>
  );
}
const withCard = (doc: Document) => {
  (doc.pages[0] as { children: Node[] }).children.push({
    type: "rect",
    id: "card",
    name: "Habit card",
    width: 100,
    height: 40,
    position: { type: "absolute", x: 600, y: 200 },
    opacity: 0.5,
    cornerRadius: 12,
  } as unknown as Node);
};

export const FrameAutoLayout: Story = { render: () => <Panel scenario="c13" /> };
export const FrameDark: Story = { render: () => <Panel scenario="c13" appearance="dark" /> };
export const FrameHighContrast: Story = {
  render: () => <Panel scenario="c13" appearance="hc-light" />,
};
export const FrameReadOnly: Story = {
  render: () => <Panel scenario="c13" options={{ readOnly: true }} />,
};
export const FramesMixed: Story = {
  render: () => (
    <Panel scenario="c13" options={{ change: withCard, selection: ["step2", "card"] }} />
  ),
};
export const FrameUnlinkedPadding: Story = {
  render: () => (
    <Panel
      scenario="c13"
      options={{
        change: (d) => {
          (d.pages[0]?.children?.[0] as unknown as { padding: number[] }).padding = [8, 16, 8, 16];
        },
      }}
    />
  ),
};
export const TextMissingFont: Story = { render: () => <Panel scenario="c14" /> };
export const MainComponent: Story = { render: () => <Panel scenario="c16" /> };
export const Instance: Story = { render: () => <Panel scenario="c16i" /> };
export const InstanceDark: Story = { render: () => <Panel scenario="c16i" appearance="dark" /> };
export const MeshFill: Story = { render: () => <Panel scenario="c33" /> };
export const StrokeAndEffects: Story = { render: () => <Panel scenario="c33e" /> };
export const NothingSelected: Story = {
  render: () => <Panel scenario="c13" options={{ selection: [] }} />,
};
