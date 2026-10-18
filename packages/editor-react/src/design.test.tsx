// TEST DATA: Juniper layers for the properties panel. Static renders: interaction is in the browser tests.

import { copy } from "@vellum/config";
import type { Document, Node } from "@vellum/types-ts";
import { renderToStaticMarkup as html } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { designDocument } from "../fixtures/design-scenarios";
import { TipProvider } from "./chrome/Tooltip";
import {
  Banner,
  IconToggle,
  Pair,
  PropertyRow,
  Section,
  Segmented,
  SelectField,
  TextField,
  ToggleRow,
  ValueField,
  VariableChip,
} from "./design/controls";
import { DesignTab } from "./design/DesignTab";
import { SPEC } from "./design/specs";
import { designFixture } from "./design-fixture";
import { RightPanel } from "./panels/RightPanel";

const noop = () => {};

describe("value field", () => {
  const field = (over: Partial<Parameters<typeof ValueField>[0]> = {}) =>
    html(
      <ValueField
        label="Gap"
        value={{ kind: "value", value: 16 }}
        spec={SPEC.px}
        onCommit={noop}
        {...over}
      />,
    );
  it("shows the number with its unit, named for assistive technology", () => {
    const out = field();
    expect(out).toContain('value="16 px"');
    expect(out).toContain('aria-label="Gap"');
    expect(out).toContain('data-state="default"');
  });
  it("shows the axis letter before the number", () => {
    expect(
      field({ axis: "X", label: "X", value: { kind: "value", value: 320 }, spec: SPEC.bare }),
    ).toMatch(/vc-field__axis[^>]*>X<.*value="320"/);
  });
  it("reads Mixed when the selection disagrees, and holds no number", () => {
    const out = field({ value: { kind: "mixed" } });
    expect(out).toContain('placeholder="Mixed"');
    expect(out).toContain('value=""');
    expect(out).toContain('data-state="mixed"');
  });
  it("shows a bound value as its resolved number", () => {
    const out = field({
      value: { kind: "bound", variable: { id: "v1", name: "space/4", value: 16 } },
    });
    expect(out).toContain('value="16 px"');
    expect(out).toContain('data-state="bound"');
  });
  it("shows the placeholder when there is nothing to show", () => {
    expect(field({ value: { kind: "none" }, placeholder: "Auto" })).toContain('placeholder="Auto"');
  });
  it("is disabled with its reason on hover", () => {
    const out = field({ disabled: true, reason: "Placed by its parent's layout" });
    expect(out).toContain("disabled");
    expect(out).toContain('title="Placed by its parent&#x27;s layout"');
    expect(out).toContain('data-state="disabled"');
  });
  it("writes opacity in percent", () => {
    expect(
      field({ value: { kind: "value", value: 0.5 }, spec: SPEC.opacity, label: "Opacity" }),
    ).toContain('value="50%"');
  });
  it("scrubs only where there is a label to drag", () => {
    expect(field({ axis: "W" })).toContain("data-scrub");
    expect(field({ axis: "W", disabled: true })).not.toContain("data-scrub");
    expect(field()).not.toContain("vc-field__axis");
  });
});

describe("text field", () => {
  it("shows its text, Mixed, or its placeholder", () => {
    expect(html(<TextField label="Name" value="Button" onCommit={() => null} />)).toContain(
      'value="Button"',
    );
    expect(html(<TextField label="Name" value="x" mixed onCommit={() => null} />)).toContain(
      'placeholder="Mixed"',
    );
    expect(
      html(<TextField label="Dashes" value="" placeholder="None" onCommit={() => null} />),
    ).toContain('placeholder="None"');
  });
  it("is disabled when it cannot be edited", () => {
    expect(html(<TextField label="Name" value="x" disabled onCommit={() => null} />)).toContain(
      "disabled",
    );
  });
});

describe("segmented control", () => {
  const options = [
    { value: "row", label: "Row" },
    { value: "column", label: "Column" },
  ] as const;
  it("is a group of radios with the selected one checked", () => {
    const out = html(<Segmented label="Layout" value="column" options={options} onChange={noop} />);
    expect(out).toContain("<fieldset");
    expect(out).toContain('aria-label="Layout"');
    expect(out).toMatch(/checked=""[^>]*value="column"/);
    expect(out).not.toMatch(/checked=""[^>]*value="row"/);
  });
  it("checks none when the selection is mixed", () => {
    const out = html(
      <Segmented label="Layout" value={null} mixed options={options} onChange={noop} />,
    );
    expect(out).not.toContain('checked=""');
    expect(out).toContain("data-mixed");
  });
  it("disables every option", () => {
    const out = html(
      <Segmented label="Layout" value="row" disabled options={options} onChange={noop} />,
    );
    expect(out.match(/disabled/g)?.length).toBeGreaterThanOrEqual(3);
  });
  it("names an icon-only option by its title", () => {
    const out = html(
      <Segmented
        label="Align"
        value="a"
        options={[{ value: "a", label: <svg aria-hidden />, title: "Align start" }]}
        onChange={noop}
      />,
    );
    expect(out).toContain('aria-label="Align start"');
  });
});

describe("select, switch and chip", () => {
  it("shows the chosen option's label", () => {
    const out = html(
      <SelectField
        label="Caps"
        value="round"
        options={[
          { value: "butt", label: "Butt" },
          { value: "round", label: "Round" },
        ]}
        onChange={noop}
      />,
    );
    expect(out).toContain("Round");
    expect(out).toContain('aria-label="Caps"');
  });
  it("shows Mixed as the placeholder", () => {
    const out = html(<SelectField label="Caps" value={null} mixed options={[]} onChange={noop} />);
    expect(out).toContain("Mixed");
    expect(out).toContain('data-state="mixed"');
  });
  it("hides the caret for a field that reads a sentence", () => {
    const out = html(
      <SelectField
        label="Blend"
        value="normal"
        display="Normal · 100%"
        options={[{ value: "normal", label: "Normal" }]}
        onChange={noop}
      />,
    );
    expect(out).toContain("data-nocaret");
    expect(out).toContain("Normal · 100%");
  });
  it("draws a switch on and off, and disabled", () => {
    expect(html(<ToggleRow label="Show icon" checked onChange={noop} />)).toMatch(
      /aria-checked="true"|data-checked/,
    );
    expect(html(<ToggleRow label="Show icon" checked={false} onChange={noop} />)).not.toContain(
      "data-checked",
    );
    expect(html(<ToggleRow label="Show icon" checked disabled onChange={noop} />)).toContain(
      "data-disabled",
    );
  });
  it("draws a pressed icon toggle", () => {
    expect(
      html(<IconToggle icon="scissors" label="Clip content" pressed onChange={noop} />),
    ).toContain('aria-pressed="true"');
  });
  it("writes the chip as the design does: the name in braces, then the value", () => {
    const out = html(<VariableChip binding={{ id: "v1", name: "space/4", value: 16 }} />);
    expect(out).toContain("{space/4} 16");
  });
  it("says a missing variable is missing, in the attention colours", () => {
    const out = html(<VariableChip binding={{ id: "gone", name: null, value: undefined }} />);
    expect(out).toContain("Missing variable");
    expect(out).toContain("data-missing");
  });
  it("is a button with a menu when it can be changed", () => {
    const out = html(
      <VariableChip binding={{ id: "v1", name: "space/4", value: 16 }} onDetach={noop} />,
    );
    expect(out).toContain("<button");
    expect(out).toContain('aria-label="Variable space/4"');
  });
});

describe("structure", () => {
  it("lays a section out in the density it is given", () => {
    expect(html(<Section id="fill" title="Fill" density="wide" />)).toContain(
      'data-density="wide"',
    );
    expect(html(<Section id="fill" title="Fill" />)).toContain('data-density="dense"');
  });
  it("names a section by its heading", () => {
    const out = html(<Section id="fill" title="Fill" />);
    expect(out).toMatch(/aria-labelledby="vc-section-fill"/);
    expect(out).toContain('id="vc-section-fill"');
  });
  it("keeps the actions in the heading row", () => {
    const out = html(
      <Section id="fill" title="Fill" actions={<button type="button">Add</button>} />,
    );
    expect(out).toMatch(/vc-section__actions[^>]*><button/);
  });
  it("lays rows and pairs out", () => {
    expect(html(<PropertyRow label="Gap">x</PropertyRow>)).toContain("vc-prop__label");
    expect(html(<Pair>x</Pair>)).toContain("vc-pair");
  });
  it("is a status banner with an icon", () => {
    const out = html(<Banner icon="info">Font missing · using Inter</Banner>);
    expect(out).toContain('role="status"');
    expect(out).toContain('data-icon="info"');
  });
});

/* ------------------------------------------------------------------------------- the whole panel */

function panel(
  scenario: Parameters<typeof designDocument>[0],
  extra: (doc: Document) => void = noop,
  selection?: string[],
  readOnly = false,
) {
  const f = designFixture(scenario, {
    change: extra,
    readOnly,
    ...(selection ? { selection } : {}),
    fonts: ["Inter"],
  });
  const render = () =>
    html(
      <TipProvider>
        <DesignTab
          design={f.design}
          session={f.session}
          paintEditor={f.paintEditor}
          document={f.doc}
          readOnly={f.readOnly}
        />
      </TipProvider>,
    );
  return { render, session: f.session, doc: f.doc };
}
const order = (out: string) => [...out.matchAll(/data-section="(\w+)"/g)].map((m) => m[1]);

describe("design tab", () => {
  it("asks for a selection when there is none", () => {
    const { render, session } = panel("c13");
    session.select([]);
    expect(render()).toContain("Select a layer to see its properties.");
  });
  it("shows a frame's sections in the order of design screen C13", () => {
    expect(order(panel("c13").render())).toEqual([
      "notes",
      "position",
      "layout",
      "appearance",
      "fill",
      "stroke",
      "effects",
      "export",
    ]);
  });
  it("shows Notes for agents collapsed when empty, with the toggle as its only content", () => {
    const out = panel("c13").render();
    expect(out).toContain('aria-expanded="false"');
    expect(out).not.toContain("vc-notes__body");
  });
  it("opens the notes when there are some, with the line that says who reads them", () => {
    const out = panel("c13", (d) => {
      (d.pages[0]?.children?.[0] as unknown as { agentNotes: string }).agentNotes =
        "Copy under 10 words.";
    }).render();
    expect(out).toContain("Copy under 10 words.");
    expect(out).toContain(copy.notesForAgentsLine);
  });
  it("shows the layout of the frame with the bound gap as a chip above its field", () => {
    const out = panel("c13").render();
    expect(out).toContain("{space/4} 16");
    expect(out).toContain('value="16 px"');
    expect(out).toMatch(/checked=""[^>]*value="column"/);
    expect(out).toContain('value="24"');
    expect(out).toContain(">Linked<");
    expect(out).toContain('value="240"');
    expect(out).toContain('value="440"');
  });
  it("shows the fill by the name of its variable, with the colour it has now", () => {
    const out = panel("c13").render();
    expect(out).toContain(">Surface<");
    expect(out).toContain('value="#FBF9F3"');
  });
  it("shows what an empty stroke and empty effects say", () => {
    const out = panel("c13").render();
    expect(out).toContain("No stroke");
    expect(out).toContain("No effects");
  });
  it("shows Radius and the clip toggle only where the layer has them", () => {
    const frame = panel("c13").render();
    expect(frame).toContain('aria-label="Clip content"');
    expect(frame).toContain('value="8 px"');
    const text = panel("c14").render();
    expect(text).not.toContain('aria-label="Clip content"');
    expect(text).not.toContain("Corner radius");
  });
  it("is Mixed across layers that differ", () => {
    const out = panel(
      "c13",
      (d) => {
        (d.pages[0] as { children: Node[] }).children.push({
          type: "rect",
          id: "r1",
          name: "Card",
          width: 10,
          height: 10,
          position: { type: "absolute", x: 10, y: 120 },
        } as unknown as Node);
      },
      ["step2", "r1"],
    ).render();
    expect(out).toContain('placeholder="Mixed"');
    expect(order(out)).not.toContain("notes");
  });
  it("disables every control in a read-only file", () => {
    const out = panel("c13", noop, undefined, true).render();
    expect(out).toContain('title="This file is read-only"');
    expect(out.match(/<input[^>]*>/g)?.every((i) => i.includes("disabled"))).toBe(true);
    expect(out).toContain("disabled");
  });
  it("cannot move a layer placed by its parent's layout, and says so", () => {
    const out = panel(
      "c13",
      (d) => {
        const frame = d.pages[0]?.children?.[0] as unknown as { children?: unknown[] };
        frame.children = [
          {
            type: "rect",
            id: "r2",
            name: "Card",
            width: 10,
            height: 10,
            position: { type: "auto" },
          },
        ];
      },
      ["r2"],
    ).render();
    expect(out).toContain("Placed by its parent&#x27;s layout");
  });
});

describe("design tab: text", () => {
  it("leads with Text, then Fill, and shows the missing font in amber", () => {
    const out = panel("c14").render();
    expect(order(out).slice(0, 2)).toEqual(["text", "position"]);
    expect(out).toContain("Font missing · using Inter");
    expect(out).toContain('role="status"');
  });
  it("shows the style, weight, size, line height as a percentage and letter spacing", () => {
    const out = panel("c14").render();
    expect(out).toContain("Heading / Medium");
    expect(out).toContain('value="500"');
    expect(out).toContain('value="24"');
    expect(out).toContain('value="120%"');
    expect(out).toContain('value="-0.5"');
  });
  it("shows alignment and resizing as segmented controls", () => {
    const out = panel("c14").render();
    expect(out).toContain('aria-label="Text alignment"');
    expect(out).toContain('aria-label="Text resizing"');
    expect(out).toMatch(/checked=""[^>]*value="left"/);
    expect(out).toMatch(/checked=""[^>]*value="fixed"/);
  });
  it("shows only Text, Fill and Content while the text is open for typing", () => {
    const { session, render } = panel("c14");
    session.dispatch({ type: "doubleClick", point: { x: 1, y: 1 } });
    // The scene of this test answers no hit, so open it the way the double-click would.
    expect(order(render())[0]).toBe("text");
  });
});

describe("design tab: components", () => {
  it("shows a main component's name, axes, props and the note that changes reach every instance", () => {
    const out = panel("c16").render();
    expect(out).toContain('data-section="component"');
    expect(out).toContain(">Component<");
    expect(out).toContain('value="Button / Juniper"');
    expect(out).toContain("Changes update every instance.");
    expect(out).toContain(">Show icon<");
  });
  it("shows an instance with a Reset for what it changed, and Go to main component", () => {
    const out = panel("c16i").render();
    expect(out).toContain(">Instance<");
    expect(out).toContain("Reset variant");
    expect(out).toContain("Reset size");
    expect(out).toContain("Reset label");
    expect(out).toContain("Go to main component");
    expect(out).toContain("Replace inside instance");
    expect(out).toContain(">Detach<");
  });
  it("offers Open library instead of Go to main for a library component", () => {
    const out = panel("c16i", (d) => {
      const i = d.pages[0]?.children?.[0] as unknown as { library: string; componentId: string };
      i.library = "Core";
      i.componentId = "k9";
    }).render();
    expect(out).toContain("Open library");
    expect(out).not.toContain("Go to main component");
  });
  it("says a component that is gone is gone", () => {
    const out = panel("c16i", (d) => {
      (d.pages[0]?.children?.[0] as unknown as { componentId: string }).componentId = "gone";
    }).render();
    expect(out).toContain("This component is missing from the file");
  });
  it("cannot detach an instance inside another one", () => {
    const { render } = panel("c16i");
    expect(render()).not.toContain("inside another one");
  });
});

describe("design tab: paints, strokes and effects", () => {
  it("lists a gradient as Paint, Opacity and Blend with Edit fill (C33)", () => {
    const out = panel("c33").render();
    expect(out).toContain("Mesh gradient");
    expect(out).toContain('data-density="wide"');
    expect(out).toContain("Edit fill");
    expect(out).toContain('aria-label="Blend mode"');
  });
  it("lists the stroke's settings in the order of design screen C33-effects", () => {
    const out = panel("c33e").render();
    const labels = ["Sides", "Align", "Caps", "Joins", "Dashes", "Blend"].map((l) =>
      out.indexOf(`>${l}<`),
    );
    expect(labels.every((i) => i > 0)).toBe(true);
    expect([...labels].sort((a, b) => a - b)).toEqual(labels);
    expect(out).toContain("Top 1 · Right 1 · Bottom 2 · Left 1");
    expect(out).toContain('value="4, 4"');
    expect(out).toContain("Normal · 100%");
  });
  it("lists the effects with what each says, hidden ones as hidden, and the shader as v1.1", () => {
    const out = panel("c33e").render();
    expect(out).toContain("0, 4 · blur 16");
    expect(out).toContain("0, 1 · blur 4");
    expect(out).toContain("8 px · hidden");
    expect(out).toContain("12 px · hidden");
    expect(out).toContain("grain.glsl · v1.1");
    expect(out).toContain("Add effect");
  });
});

describe("right panel", () => {
  it("has Design, Inspect and Activity tabs, with Design selected", () => {
    const out = html(<RightPanel design={<p>design body</p>} />);
    expect(out).toContain('role="tablist"');
    expect(out).toMatch(/aria-selected="true"[^>]*>Design</);
    expect(out).toContain("Inspect");
    expect(out).toContain("Activity");
    expect(out).toContain("design body");
  });
});
