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

// --- Design tokens (scripts/design-tokens, Prompt 43) ---
// Prints the theme axes (@T), one line per variable with its definition (@V), and one line per
// foreground/background pair the design uses (@P: kind, foreground variable, nearest ancestor fill, count).
// A pair is a text or icon whose fill is a variable, matched with the closest enclosing node whose fill
// is a single colour; nodes under a hidden ancestor are skipped, and instances are expanded so component
// internals count. Read-only: Get, GetVariables
// and Print only.
export const TOKENS_BEGIN = "@@vellum-tokens-begin";
export const TOKENS_END = "@@vellum-tokens-end";

export const TOKENS_QUERY = [
  `Print(${JSON.stringify(TOKENS_BEGIN)})`,
  `v=GetVariables()`,
  `Print("@T",JSON.stringify(v.themes||{}))`,
  `Object.keys(v.variables).sort().forEach(k=>Print("@V",JSON.stringify([k,v.variables[k]])))`,
  `pairs={}`,
  `Get((n,c)=>{if((n.type!=="text"&&n.type!=="icon")||typeof n.fill!=="string"||!n.fill.startsWith("$")||n.enabled===false)return;let bg=null;for(let p=c.parentCtx;p;p=p.parentCtx){if(p.node.enabled===false)return;if(bg===null&&typeof p.node.fill==="string")bg=p.node.fill}const k=JSON.stringify([n.type,n.fill.slice(1),bg]);pairs[k]=(pairs[k]||0)+1},{resolveInstances:true})`,
  `Object.keys(pairs).sort().forEach(k=>Print("@P",JSON.stringify([...JSON.parse(k),pairs[k]])))`,
  `Print(${JSON.stringify(TOKENS_END)})`,
].join(";\n");

/** The Tokens sheet ("Foundations · Tokens · …") in an inventory, or null. */
export const tokensFrame = (inventory) =>
  inventory.frames.find((f) => f.kind === "foundations" && /^Tokens\b/.test(f.title)) ?? null;

export function parseTokensRaw(text) {
  const start = text.indexOf(TOKENS_BEGIN);
  const end = text.indexOf(TOKENS_END);
  if (start === -1 || end === -1 || end < start)
    throw new Error(`the query output has no ${TOKENS_BEGIN} … ${TOKENS_END} block`);
  let themes = null;
  const variables = [];
  const pairs = [];
  for (const line of text.slice(start + TOKENS_BEGIN.length, end).split("\n")) {
    const l = line.trim();
    if (l.startsWith("@T ")) themes = JSON.parse(l.slice(3));
    else if (l.startsWith("@V ")) variables.push(JSON.parse(l.slice(3)));
    else if (l.startsWith("@P ")) pairs.push(JSON.parse(l.slice(3)));
  }
  if (themes === null) throw new Error("the query output has no themes line");
  if (variables.length === 0) throw new Error("the query output lists no variables");
  return { themes, variables, pairs };
}
