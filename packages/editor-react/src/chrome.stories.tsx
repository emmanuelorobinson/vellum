// TEST DATA: Juniper names and Sam, Teal and Claude Code are story fixtures.
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { Mode } from "@vellum/ui-tokens";
import type { ReactNode } from "react";
import { ActorMarker } from "./chrome/ActorMarker";
import { BottomStrip } from "./chrome/BottomStrip";
import { DocumentBar } from "./chrome/DocumentBar";
import { ToolRail } from "./chrome/ToolRail";
import { TipProvider } from "./chrome/Tooltip";
import { TopRightBar } from "./chrome/TopRightBar";
import { ZoomPill } from "./chrome/ZoomPill";
import "./editor.css";

function Stage({ mode, children }: { mode: Mode; children: ReactNode }) {
  return (
    <div
      className="vc-workspace"
      data-appearance={mode}
      data-canvas-appearance={mode}
      style={{ width: 480, height: 260 }}
    >
      <TipProvider>{children}</TipProvider>
    </div>
  );
}
const modes: Mode[] = ["light", "dark", "hc-light", "hc-dark"];
const meta = { title: "Editor/Floating chrome", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj;

export const DocumentBars: Story = {
  render: () => (
    <>
      {modes.map((m) => (
        <Stage key={m} mode={m}>
          <DocumentBar fileName="juniper.vellum" saveState="saved" width={360} commands={[]} />
          <div style={{ position: "absolute", top: 56 }}>
            <DocumentBar fileName="juniper.vellum" saveState="saving" width={360} commands={[]} />
          </div>
          <div style={{ position: "absolute", top: 104 }}>
            <DocumentBar fileName="juniper.vellum" saveState="unsaved" width={360} commands={[]} />
          </div>
        </Stage>
      ))}
    </>
  ),
};
export const ToolRails: Story = {
  render: () => (
    <>
      {(["move", "frame", "ellipse"] as const).map((tool) => (
        <Stage key={tool} mode="light">
          <ToolRail
            x={8}
            y={8}
            tool={tool}
            selectTool={() => {}}
            run={() => {}}
            enabled={(c) => c !== "image.insert"}
            shortcut={() => null}
          />
        </Stage>
      ))}
    </>
  ),
};
export const TopRight: Story = {
  render: () => (
    <Stage mode="light">
      <TopRightBar
        right={8}
        agents={[
          { id: "t", kind: "agent", name: "Teal", hue: "teal", state: "working" },
          { id: "c", kind: "agent", name: "Claude Code", hue: "saffron", state: "idle" },
        ]}
        onShare={() => {}}
        onPreview={() => {}}
        onSettings={() => {}}
      />
    </Stage>
  ),
};
export const BottomStrips: Story = {
  render: () => (
    <>
      {(
        [
          { mode: "live", agents: 3 },
          { mode: "headless" },
          { mode: "offline", note: "Editing and saving available" },
        ] as const
      ).map((connection) => (
        <Stage key={connection.mode} mode="light">
          <BottomStrip
            centerX={240}
            y={110}
            branch="feat/onboarding"
            dirty={connection.mode === "live"}
            connection={connection}
            basicRenderer={connection.mode === "offline"}
          />
        </Stage>
      ))}
    </>
  ),
};
export const ZoomPills: Story = {
  render: () => (
    <Stage mode="light">
      <ZoomPill
        x={400}
        y={200}
        label="72%"
        items={[{ label: "Zoom in", run: () => {}, key: "⌘+" }]}
      />
    </Stage>
  ),
};
export const Markers: Story = {
  render: () => (
    <Stage mode="light">
      <div style={{ display: "flex", gap: 12, padding: 12 }}>
        {[16, 20, 24, 32].map((size) => (
          <ActorMarker
            key={size}
            actor={{ id: "t", kind: "agent", name: "Teal", hue: "teal" }}
            size={size}
          />
        ))}
        <ActorMarker actor={{ id: "s", kind: "person", name: "Sam" }} size={20} />
      </div>
    </Stage>
  ),
};
