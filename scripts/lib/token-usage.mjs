// The token usage lint (guide Prompt 43, AGENTS.md "Visual language"). Pure: takes file paths and texts.
//
// 1. No colour literal in UI code: hex colours and CSS colour functions belong in tokens only.
// 2. Selection blue and component violet are never on buttons.
// 3. Agent hues appear only on agent markers, cursors, leases, placeholders and session dots.
//
// The checks are textual, so they look at what is near a token reference: the CSS rule's selector, the
// JSX element it sits in, and the file name. packages/ui-tokens defines and displays every token, so it is
// exempt from 2 and 3 (its generated files are exempt from all three).

// Where UI code lives (guide §8). Packages without UI (core, hosts, renderers, adapters) are not linted.
export const UI_ROOTS = [
  "packages/editor-react",
  "packages/ui-tokens",
  "apps/extension",
  "apps/desktop",
  "apps/web",
  "apps/account",
  "apps/docs",
];
export const UI_EXTENSIONS = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".css", ".html"];
const SKIP_DIRS = [
  "node_modules",
  "dist",
  "storybook-static",
  ".next",
  ".turbo",
  "generated",
  "__tests__",
  "fixtures",
];

export function isLinted(path) {
  if (!UI_ROOTS.some((root) => path.startsWith(`${root}/`))) return false;
  const parts = path.split("/");
  if (parts.some((p) => SKIP_DIRS.includes(p))) return false;
  if (/\.test\.[a-z]+$/.test(path)) return false;
  return UI_EXTENSIONS.some((ext) => path.endsWith(ext));
}

const AGENT_CONTEXT = /marker|cursor|lease|placeholder|session[-_]?dot/i;
const BUTTON_CONTEXT = /button|btn|\[role=["']?button/i;
const COLOR_FUNCTION = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/;
const HEX = /(?<![\w&$-])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])/g;

// Keeps offsets: comments become spaces.
function stripComments(text, css) {
  const blank = (m) => m.replace(/[^\n]/g, " ");
  let out = text.replace(/\/\*[\s\S]*?\*\//g, blank);
  if (!css) out = out.replace(/^(\s*)\/\/.*$/gm, blank);
  return out;
}

// String literal bodies in JS/TS (quotes and template text), with their offsets.
function stringSpans(text) {
  const spans = [];
  const re = /"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`/g;
  for (const m of text.matchAll(re)) spans.push([m.index, m.index + m[0].length]);
  return spans;
}

const lineOf = (text, offset) => text.slice(0, offset).split("\n").length;
const baseName = (path) => path.slice(path.lastIndexOf("/") + 1);

// The selector of the CSS rule an offset sits in.
function cssSelector(text, offset) {
  const open = text.lastIndexOf("{", offset);
  if (open === -1) return "";
  const before = Math.max(text.lastIndexOf("}", open), text.lastIndexOf(";", open));
  return text.slice(before + 1, open).trim();
}

// The JSX element an offset sits in: the nearest "<Name" before it with no element end in between.
function jsxElement(text, offset) {
  const before = text.slice(0, offset);
  const start = before.search(/<([A-Za-z][\w.]*)[^<]*$/);
  if (start === -1) return "";
  const name = before.slice(start + 1).match(/^[\w.]+/)?.[0] ?? "";
  return /\/>|<\//.test(before.slice(start)) ? "" : name;
}

function context(path, text, offset) {
  if (path.endsWith(".css")) return cssSelector(text, offset);
  if (path.endsWith(".html")) return "";
  return jsxElement(text, offset);
}

/**
 * files: [{ path, text }] (repo-relative paths). hues: agent hue names (from tokens.json, never hardcoded
 * here). Returns { errors: [{ path, line, rule, message }] }.
 */
export function checkUsage({ files, hues }) {
  const errors = [];
  const hueAlt = hues.join("|");
  const agentRef = new RegExp(
    `--agent-(?:${hueAlt}|\\$\\{)|agent/(?:${hueAlt}|\\$\\{)|\\bagent(?:${hues.map((h) => h[0].toUpperCase() + h.slice(1)).join("|")})\\b`,
    "g",
  );
  const markRef = /--canvas-(?:selection|component)\b|canvas\/(?:selection|component)\b/g;
  for (const { path, text: raw } of files) {
    if (!isLinted(path)) continue;
    const css = path.endsWith(".css") || path.endsWith(".html");
    const text = stripComments(raw, css);
    const push = (offset, rule, message) =>
      errors.push({ path, line: lineOf(raw, offset), rule, message });

    // 1. Colour literals: anywhere in CSS and HTML, inside string literals in scripts.
    const spans = css ? [[0, text.length]] : stringSpans(text);
    for (const [from, to] of spans) {
      const body = text.slice(from, to);
      for (const m of body.matchAll(HEX))
        push(
          from + m.index,
          "color-literal",
          `colour literal ${m[0]}: use a token from @vellum/ui-tokens`,
        );
      const fn = body.match(COLOR_FUNCTION);
      if (fn)
        push(
          from + fn.index,
          "color-literal",
          `colour function ${fn[0]}…): use a token from @vellum/ui-tokens`,
        );
    }

    if (path.startsWith("packages/ui-tokens/")) continue;
    const fileName = baseName(path);

    // 2. Selection blue and component violet never on buttons.
    for (const m of text.matchAll(markRef)) {
      const where = context(path, text, m.index);
      if (BUTTON_CONTEXT.test(where) || BUTTON_CONTEXT.test(fileName))
        push(
          m.index,
          "mark-on-button",
          `${m[0]} on a button (${where || fileName}): selection blue and component violet are never used for buttons`,
        );
    }

    // 3. Agent hues only where an agent is shown.
    for (const m of text.matchAll(agentRef)) {
      const where = context(path, text, m.index);
      if (!AGENT_CONTEXT.test(where) && !AGENT_CONTEXT.test(fileName))
        push(
          m.index,
          "agent-hue",
          `${m[0]} outside an agent marker, cursor, lease, placeholder or session dot (${where || fileName})`,
        );
    }
  }
  return { errors };
}
