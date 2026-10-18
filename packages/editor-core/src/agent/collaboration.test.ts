// TEST DATA: the existing .vellum collaboration node kinds.
import { expect, it } from "vitest";
import { collaborationTransaction, contextForSelection, promptNodeRequest } from "./collaboration";

const context = {
  type: "context",
  id: "rules",
  name: "Rules",
  content: "One primary action",
  pointsAt: "frame",
};
const tree = {
  pages: [
    {
      id: "p",
      children: [
        context,
        { type: "frame", id: "frame", children: [{ type: "rect", id: "child" }] },
      ],
    },
  ],
};
it("selecting a descendant includes targeted context, unrelated selections do not", () => {
  expect(contextForSelection(tree, "p", ["child"])).toEqual([
    { kind: "context", label: "Rules · automatic", value: "rules" },
  ]);
  expect(contextForSelection(tree, "p", ["other"])).toEqual([]);
});
it("note changes are person Ops and prompt Run preserves saved engine, model and scope", () => {
  expect(collaborationTransaction("device", "note", "Copy")).toMatchObject({
    origin: { actor: { kind: "person", device: "device" } },
    ops: [{ type: "update", id: "note", patch: { content: "Copy" } }],
  });
  const request = promptNodeRequest(
    { content: "Tablet", engine: "codex", model: "m" },
    null,
    contextForSelection(tree, "p", ["child"]),
  );
  expect(request).toMatchObject({
    text: "Tablet",
    engine: { engine: "codex", model: "m" },
    parallel: { count: 1, mode: "split" },
    context: [{ kind: "context", value: "rules" }],
  });
});
