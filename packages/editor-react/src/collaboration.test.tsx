// TEST DATA: rendering assertions, no browser or network needed.

import { createCollaboration } from "@vellum/editor-core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { CollaborationToolbar, CommentsPanel } from "./collaboration/Collaboration";

it("offers comments as a contextual surface and preserves a restricted draft", () => {
  const c = createCollaboration({
    link: {
      state: "offline",
      pending: 0,
      connect: async () => {},
      disconnect() {},
      request: async () => {},
      ephemeral() {},
      subscribeState: () => ({ dispose() {} }),
      subscribeFrames: () => ({ dispose() {} }),
    },
    document: () => null,
    local: {
      name: "TEST DATA",
      cursor: null,
      viewport: null,
      selection: [],
      page: null,
      hue: null,
      activity: "",
    },
    now: () => 0,
    nextId: () => "test",
    viewport: () => ({ x: 0, y: 0, zoom: 1 }),
    follow() {},
  });
  c.open();
  c.draft("TEST DATA: retained draft");
  const html = renderToStaticMarkup(
    <>
      <CollaborationToolbar controller={c} />
      <CommentsPanel controller={c} />
    </>,
  );
  expect(html).toContain("Comments");
  expect(html).toContain("Copy draft");
  expect(html).toContain("TEST DATA: retained draft");
  expect(html).toContain("readOnly");
  expect(html).not.toContain("Stop this person");
  c.dispose();
});
