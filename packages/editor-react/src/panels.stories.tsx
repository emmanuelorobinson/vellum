// TEST DATA: a small Juniper-like document.
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { Document, Node } from "@vellum/types-ts";
import { TipProvider } from "./chrome/Tooltip";
import { AssetsTab } from "./panels/AssetsTab";
import { LayersTab } from "./panels/LayersTab";
import "./editor.css";

const n = (type: string, id: string, extra: Record<string, unknown> = {}) =>
  ({ type, id, name: extra.name ?? id, width: 10, height: 10, ...extra }) as unknown as Node;
const document_ = {
  meta: { formatVersion: 1, name: "juniper.vellum", createdWith: "story" },
  pages: [
    {
      id: "p1",
      name: "Onboarding",
      children: [
        n("frame", "welcome", { name: "Welcome" }),
        n("frame", "step2", {
          name: "Step 2",
          layout: "column",
          children: [
            n("instance", "card", { name: "Habit card" }),
            n("text", "title", { name: "Title" }),
          ],
        }),
      ],
    },
    { id: "p2", name: "Settings", children: [] },
  ],
} as unknown as Document;

const meta = { title: "Editor/Panels", parameters: { layout: "centered" } } satisfies Meta;
export default meta;
type Story = StoryObj;

const Frame = ({ children }: { children: React.ReactNode }) => (
  <div
    className="vc-workspace"
    data-appearance="light"
    data-canvas-appearance="light"
    style={{ width: 360, height: 520, position: "relative" }}
  >
    <TipProvider>
      <div className="vc-bar vc-panel" style={{ left: 0, top: 0, width: 360, height: 520 }}>
        <div className="vc-panel__body">{children}</div>
      </div>
    </TipProvider>
  </div>
);
const base = {
  document: document_,
  page: "p1",
  onPage: () => {},
  onSelect: () => {},
  commit: () => {},
  device: "d",
  now: () => 1000,
  recent: { welcome: { agent: "teal", at: 999 } },
  actors: { teal: { id: "teal", kind: "agent" as const, name: "Teal", hue: "teal" } },
  readOnly: false,
};
export const Layers: Story = {
  render: () => (
    <Frame>
      <LayersTab {...base} selection={["step2"]} />
    </Frame>
  ),
};
export const LayersEmpty: Story = {
  render: () => (
    <Frame>
      <LayersTab
        {...base}
        document={{ ...document_, pages: [{ id: "p1", name: "Onboarding", children: [] }] }}
        selection={[]}
        onDrawFrame={() => {}}
      />
    </Frame>
  ),
};
export const LayersReadOnly: Story = {
  render: () => (
    <Frame>
      <LayersTab {...base} selection={[]} readOnly />
    </Frame>
  ),
};
export const Assets: Story = {
  render: () => (
    <Frame>
      <AssetsTab
        components={{ c1: { name: "Button / Primary" } }}
        libraries={[
          { alias: "Juniper Core", state: "synced", components: 8 },
          { alias: "Old kit", state: "missing", components: 3 },
        ]}
        iconSets={[{ id: "lucide", label: "Lucide" }]}
        projectIconFolder={{ path: "design/icons" }}
        visualStyles={[{ id: "minimal", name: "Minimal" }]}
        designSystems={[{ id: "s", name: "Starter Minimal" }]}
        onDropComponent={() => {}}
        onAction={() => {}}
        readOnly={false}
      />
    </Frame>
  ),
};
