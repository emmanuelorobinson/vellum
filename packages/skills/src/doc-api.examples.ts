// TEST DATA. Compiled against generated declarations, never executed on package import.
import type { VellumDoc } from "./generated/vellum-doc";
export function declarationExamples(doc: VellumDoc) {
  const page = doc.page("TEST DATA");
  const frame = doc.insert(page, {
    type: "frame",
    width: 100,
    height: 100,
    children: [{ type: "rect", width: 10, height: 10 }],
  });
  doc.update(frame, { name: "TEST DATA", agentNotes: "TEST DATA instruction" });
  const copy = doc.copy(frame, page);
  doc.move(copy, frame, 0);
  doc.delete(copy);
  doc.icon(page, "test:check", { width: 16, height: 16 });
  doc.setVariables({ count: { type: "number", name: "TEST DATA", values: [{ value: 3 }] } });
  const version: "1.0" = doc.apiVersion;
  // @ts-expect-error A rect needs both dimensions.
  doc.insert(page, { type: "rect", width: 10 });
  // @ts-expect-error No filesystem API exists.
  doc.readFile("TEST DATA");
  // @ts-expect-error Node fields use the generated file schema.
  doc.insert(page, { type: "rect", width: 10, height: 10, bogus: true });
  return [
    version,
    doc.get(frame),
    doc.query({ type: "rect" }),
    doc.layoutProblems(frame),
    doc.findSpace(page, { width: 100, height: 100 }),
    doc.screenshot(frame),
    doc.export(frame, { format: "svg" }),
  ];
}
