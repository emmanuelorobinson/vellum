// TEST DATA.
import { createSlidesController } from "@vellum/editor-core";
import { FakeEngineClient } from "@vellum/engine-client";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { SlideActions, SlideStrip } from "./Slides";

it("numbers only frames, marks selection and disables document controls in read-only", () => {
  const engine = new FakeEngineClient({
    revision: 0,
    readOnly: true,
    document: {
      meta: { formatVersion: 1, name: "TEST DATA", createdWith: "fixture" },
      pages: [
        {
          id: "page000001",
          name: "Deck",
          kind: "slides",
          children: [
            { type: "frame", id: "slide00001", name: "First", width: 800, height: 450 },
            {
              type: "note",
              id: "note000001",
              name: "Private",
              content: "TEST DATA",
              width: 100,
              height: 100,
            },
            { type: "frame", id: "slide00002", name: "Second", width: 400, height: 600 },
          ],
        },
      ],
    },
  });
  const c = createSlidesController({
    engine,
    device: "device0001",
    page: () => "page000001",
    nextPageId: () => "page000002",
    onPage: () => {},
  });
  const html = renderToStaticMarkup(
    <SlideStrip controller={c} selection={["slide00002"]} onSelect={() => {}} readOnly />,
  );
  expect(html).toContain("01");
  expect(html).toContain("02");
  expect(html).not.toContain("Private");
  expect(html).toContain('aria-selected="true"');
  expect(html).toContain('draggable="false"');
  const actions = renderToStaticMarkup(<SlideActions controller={c} selection={[]} />);
  expect(actions).toContain("Presentation rendering is unavailable");
  expect(actions).toContain("disabled");
  c.dispose();
});
