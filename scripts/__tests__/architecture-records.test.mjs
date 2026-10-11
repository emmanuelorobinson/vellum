// ADR-000 (guide §6.1 and Prompt 2): the six verified deltas from ARCHITECTURE.md are recorded as Proposed,
// each naming the prompt that confirms or rejects it and the ADR that prompt will write.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { at, read } from "./helpers.mjs";

const ADR = "docs/architecture/decisions/ADR-000-deltas.md";

const CONFIRMED_BY = {
  D1: { prompt: /Prompt 4\b/, adr: "ADR-001" },
  D2: { prompt: /Prompt 6\b/, adr: "ADR-002" },
  D3: { prompt: /Prompt 27\b/, adr: "ADR-003" },
  D4: { prompt: /Prompts 36–37/, adr: "ADR-007" },
  D5: { prompt: /Prompts 25–26/, adr: "ADR-005" },
  D6: { prompt: /Prompt 34\b/, adr: "ADR-006" },
};

// The text under a "## D<n> ..." heading, up to the next level-2 heading.
function sectionFor(markdown, id) {
  return markdown
    .split(/^## /m)
    .find((part) => part.startsWith(`${id} `) || part.startsWith(`${id}\n`));
}

test("ADR-000 exists under docs/architecture/decisions/", () => {
  assert.ok(existsSync(at(ADR)), `${ADR} is missing`);
});

test("ADR-000 records D1 to D6 as Proposed, each with its confirming prompt and revisit condition", () => {
  const text = read(ADR);
  for (const [id, { prompt, adr }] of Object.entries(CONFIRMED_BY)) {
    const section = sectionFor(text, id);
    assert.ok(section, `ADR-000 has no "## ${id}" section`);
    assert.match(section, /Status:\s*Proposed/, `${id} must be Proposed`);
    assert.match(section, prompt, `${id} must name its confirming prompt`);
    assert.match(section, new RegExp(adr), `${id} must name ${adr}`);
    assert.match(
      section,
      /Revisit when/,
      `${id} needs a "Revisit when" condition (ARCHITECTURE §2.6)`,
    );
  }
});
