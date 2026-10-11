// Everything scripts/design-snapshot knows about pen.dev's `execute` tool lives in this file, so a change
// in pen.dev touches one place. The snippets are read-only: they use Get, GetVariables, Print and Export
// only. verifyExecuteApi is the guard that runs read_skill first (guide §2.1 rule 2): if the current
// execute page no longer documents those functions, the snapshot stops instead of guessing.
import { parseFrameName, SNAPSHOT_KINDS, snapshotFiles } from "./design-frames.mjs";

export const BEGIN = "@@vellum-inventory-begin";
export const END = "@@vellum-inventory-end";
export const REQUIRED_API = ["Get", "GetVariables", "Print", "Export"];

// Prints one line per reusable component (@C), per top-level frame (@F: id, name, x, y, width, height,
// ids of the components it instances) and one for the variables (@V), between two markers.
export const INVENTORY_QUERY = [
  `Print(${JSON.stringify(BEGIN)})`,
  `comps=Get(n=>n.reusable?[n.id,n.name]:undefined)`,
  `comps.forEach(r=>Print("@C",JSON.stringify(r)))`,
  `tops=[]`,
  `Get((n,c)=>{c.skipChildren();tops.push([n.id,n.name,Math.round(c.bounds.x),Math.round(c.bounds.y),Math.round(c.bounds.width),Math.round(c.bounds.height)])})`,
  `tops.forEach(t=>{refs=Get(t[0],n=>n.type==="ref"?n.ref:undefined);Print("@F",JSON.stringify([...t,[...new Set(refs)]]))})`,
  `v=GetVariables()`,
  `Print("@V",JSON.stringify({themes:v.themes||{},names:Object.keys(v.variables)}))`,
  `Print(${JSON.stringify(END)})`,
].join(";\n");

export const exportSnippet = ({ ids, scale, dir }) =>
  `Export(${JSON.stringify(ids)},"png",${JSON.stringify(dir)},{scale:${scale}})`;

// pen interactive reads `tool({ key: value })` lines; a JSON string literal is a valid JS string.
export const executeLine = (snippet) => `execute({ input: ${JSON.stringify(snippet)} })`;

export function verifyExecuteApi(skillText) {
  const missing = REQUIRED_API.filter((name) => !new RegExp(`\\b${name}\\(`).test(skillText));
  if (missing.length === 0) return;
  throw new Error(
    `read_skill({ path: "execute.md" }) no longer documents ${missing.join(", ")}. The execute API changed; update scripts/lib/pen-queries.mjs before running design-snapshot again.`,
  );
}

export function parseRaw(text) {
  const start = text.indexOf(BEGIN);
  const end = text.indexOf(END);
  if (start === -1 || end === -1 || end < start)
    throw new Error(`the query output has no ${BEGIN} … ${END} block`);
  const components = [];
  const frames = [];
  let variables = null;
  for (const line of text.slice(start + BEGIN.length, end).split("\n")) {
    const l = line.trim();
    if (l.startsWith("@C ")) components.push(JSON.parse(l.slice(3)));
    else if (l.startsWith("@F ")) frames.push(JSON.parse(l.slice(3)));
    else if (l.startsWith("@V ")) variables = JSON.parse(l.slice(3));
  }
  if (frames.length === 0) throw new Error("the query output lists no frames");
  if (variables === null) throw new Error("the query output has no variables line");
  return { components, frames, variables };
}

export function buildInventory({ raw, sourceSha256, exporter }) {
  const names = new Map(raw.components.map(([id, name]) => [id, name]));
  const frames = raw.frames.map(([frameId, name, x, y, width, height, refs]) => {
    const parsed = parseFrameName(name);
    const frame = {
      frame_id: frameId,
      name,
      kind: parsed.ok ? parsed.kind : "unrecognised",
      screen_id: parsed.ok ? parsed.id : null,
      title: parsed.ok ? parsed.title : name,
      theme: parsed.ok ? parsed.theme : null,
      width: parsed.ok ? parsed.width : null,
      page: null,
      bounds: { x, y, width, height },
      components: [...new Set(refs.map((id) => names.get(id)).filter(Boolean))].sort(),
    };
    if (!parsed.ok) frame.reason = parsed.reason;
    if (parsed.ok && SNAPSHOT_KINDS.includes(parsed.kind)) frame.snapshots = snapshotFiles(parsed);
    return frame;
  });
  return {
    version: 1,
    source_sha256: sourceSha256,
    exporter,
    frames,
    components: raw.components.map(([id, name]) => ({ id, name })),
    variables: raw.variables,
  };
}
