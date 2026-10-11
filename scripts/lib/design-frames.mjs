// The frame-name grammar of docs/design/vellum.pen.
//
// Guide §2.1 describes "<ScreenID> <Title> / <Theme> / <Width>" on named pages. The design file as made
// uses "<ScreenID> · <Title> · [<Width> · ]<theme>" in one flat document (pen.dev has no pages), with the
// appearances light, dark, hc-light and hc-dark. The guide ranks vellum.pen above itself (§2), so this
// module reads the file's convention; docs/design/DESIGN-GAPS.md records each difference.

export const THEMES = ["light", "dark", "hc-light", "hc-dark"];
export const DEFAULT_WIDTH = 1280;
// Frame height for each width DESIGN.md asks for ("Deliverables").
export const HEIGHT_FOR_WIDTH = { 1280: 800, 1512: 982, 960: 640, 760: 640 };

const SEPARATOR = " · ";
const SCREEN_ID = /^[CAWS]\d{2}(?:-[a-z0-9]+(?:-[a-z0-9]+)*)?$/;
const STATE_ID = /^State-[a-z]+(?:-[a-z]+)*$/;
const FIXTURE_ID = /^Fixture-[A-Za-z]+(?:-[A-Za-z]+)*$/;
const SHEET_KINDS = {
  Library: "library",
  Components: "components",
  Flow: "flow",
  CLI: "cli",
  Foundations: "foundations",
  Vellum: "vellum",
};

// Frames that carry an id and a theme are screens in the sense of the snapshot: they are exported.
export const SNAPSHOT_KINDS = ["screen", "state", "fixture"];

export function parseFrameName(name) {
  const tokens = name.split(SEPARATOR).map((t) => t.trim());
  const head = tokens[0];
  let kind;
  if (SCREEN_ID.test(head)) kind = "screen";
  else if (STATE_ID.test(head)) kind = "state";
  else if (FIXTURE_ID.test(head)) kind = "fixture";
  else if (Object.hasOwn(SHEET_KINDS, head)) kind = SHEET_KINDS[head];
  else {
    return {
      ok: false,
      reason: `"${head}" is not a screen id (C01, A01, W01, S01, State-…, Fixture-…) or a sheet kind (${Object.keys(SHEET_KINDS).join(", ")})`,
    };
  }

  const rest = tokens.slice(1);
  let theme = null;
  if (rest.length > 0 && THEMES.includes(rest.at(-1))) theme = rest.pop();
  let width = null;
  if (rest.length > 0 && /^\d{3,4}$/.test(rest.at(-1))) width = Number(rest.pop());
  const title = rest.join(SEPARATOR);

  const snapshotted = SNAPSHOT_KINDS.includes(kind);
  if (snapshotted && theme === null) {
    return {
      ok: false,
      reason: `no theme: a screen frame ends with one of ${THEMES.slice(0, -1).join(", ")} or ${THEMES.at(-1)}`,
    };
  }
  if (snapshotted && title === "") {
    return { ok: false, reason: "no title after the screen id" };
  }
  if (width !== null && !Object.hasOwn(HEIGHT_FOR_WIDTH, width)) {
    return {
      ok: false,
      reason: `width ${width} is not one of ${Object.keys(HEIGHT_FOR_WIDTH).join(", ")}`,
    };
  }
  return {
    ok: true,
    kind,
    id: snapshotted ? head : null,
    sheet: snapshotted ? null : head,
    title,
    theme,
    width: snapshotted ? (width ?? DEFAULT_WIDTH) : width,
  };
}

// File stem for a screen's snapshots: C03-hc-dark-1512 (+ .png, @2x.png).
export const screenSlug = ({ id, theme, width }) => `${id}-${theme}-${width}`;
export const snapshotFiles = (parsed) => {
  const slug = screenSlug(parsed);
  return { "1x": `${slug}.png`, "2x": `${slug}@2x.png` };
};
