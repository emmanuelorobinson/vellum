import { HANDLE_SIZE, type OverlayShape } from "@vellum/editor-core";

/** The colours the overlay uses, read from the page's tokens so it follows the appearance attributes. */
export interface OverlayPalette {
  selection: string;
  component: string;
  panel: string;
  border: string;
  text: string;
  secondary: string;
  attention: string;
  /** The pixel grid's lines. */
  grid: string;
}
export function readPalette(el: Element): OverlayPalette {
  const css = getComputedStyle(el);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    selection: v("--canvas-selection"),
    component: v("--canvas-component"),
    panel: v("--chrome-panel"),
    border: v("--chrome-border"),
    text: v("--chrome-text"),
    secondary: v("--chrome-secondary"),
    attention: v("--chrome-attention"),
    grid: v("--chrome-separator"),
  };
}

const FONT = '450 11px Inter, "Inter", system-ui, sans-serif';

function chip(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  anchor: "start" | "middle",
  palette: OverlayPalette,
  selected: boolean,
) {
  ctx.font = FONT;
  const w = ctx.measureText(text).width + 12;
  const h = 21;
  const left = anchor === "middle" ? x - w / 2 : x;
  const top = y - h;
  ctx.fillStyle = palette.panel;
  ctx.strokeStyle = selected ? palette.selection : palette.border;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.roundRect(left + 0.5, top + 0.5, w - 1, h - 1, 4);
  ctx.fill();
  ctx.stroke();
  ctx.fillStyle = palette.secondary;
  ctx.textBaseline = "middle";
  ctx.fillText(text, left + 6, top + h / 2 + 0.5);
}

/** Draws overlay shapes, in screen pixels back to front. Nothing here changes the document. */
export function drawOverlay(
  ctx: CanvasRenderingContext2D,
  shapes: readonly OverlayShape[],
  palette: OverlayPalette,
  dpr: number,
  /** Settings → Canvas → Show pixel grid, from 800%: lines between model pixels, under everything else. */
  grid?: { x: readonly number[]; y: readonly number[] } | null,
) {
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width / dpr, ctx.canvas.height / dpr);
  if (grid) {
    const width = ctx.canvas.width / dpr,
      height = ctx.canvas.height / dpr;
    ctx.strokeStyle = palette.grid;
    ctx.lineWidth = 1 / dpr;
    ctx.setLineDash([]);
    ctx.beginPath();
    for (const x of grid.x) {
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
    }
    for (const y of grid.y) {
      ctx.moveTo(0, y);
      ctx.lineTo(width, y);
    }
    ctx.stroke();
  }
  for (const s of shapes) {
    switch (s.kind) {
      case "outline": {
        const colour =
          s.role === "component" || s.role === "instance"
            ? palette.component
            : s.role === "preview"
              ? palette.selection
              : palette.selection;
        ctx.strokeStyle = colour;
        ctx.lineWidth = s.role === "instance" ? 1 : 1.5;
        ctx.setLineDash(s.role === "preview" ? [4, 3] : []);
        ctx.strokeRect(s.box.x, s.box.y, s.box.width, s.box.height);
        break;
      }
      case "marquee":
        ctx.fillStyle = `color-mix(in srgb, ${palette.selection} 12%, transparent)`;
        ctx.strokeStyle = palette.selection;
        ctx.lineWidth = 1;
        ctx.setLineDash([]);
        ctx.fillRect(s.box.x, s.box.y, s.box.width, s.box.height);
        ctx.strokeRect(s.box.x + 0.5, s.box.y + 0.5, s.box.width, s.box.height);
        break;
      case "line":
        ctx.strokeStyle = s.role === "guide" ? palette.attention : palette.selection;
        ctx.lineWidth = 1;
        ctx.setLineDash(s.role === "guide" ? [4, 3] : []);
        ctx.beginPath();
        ctx.moveTo(s.from.x, s.from.y);
        ctx.lineTo(s.to.x, s.to.y);
        ctx.stroke();
        break;
      case "handle": {
        const r = s.size / 2;
        ctx.fillStyle = palette.panel;
        ctx.strokeStyle = palette.selection;
        ctx.lineWidth = 1.5;
        ctx.setLineDash([]);
        ctx.beginPath();
        if (s.role === "in" || s.role === "out") ctx.arc(s.at.x, s.at.y, r - 1, 0, Math.PI * 2);
        else ctx.rect(s.at.x - r, s.at.y - r, HANDLE_SIZE, HANDLE_SIZE);
        ctx.fill();
        ctx.stroke();
        break;
      }
      case "paintPoint": {
        const r = s.size / 2;
        ctx.setLineDash([]);
        ctx.beginPath();
        if (s.role === "mesh") ctx.arc(s.at.x, s.at.y, r, 0, Math.PI * 2);
        else ctx.rect(s.at.x - r, s.at.y - r, s.size, s.size);
        ctx.fillStyle = s.colour ?? palette.panel;
        ctx.fill();
        // The ring is the panel colour so a point reads on any paint; the selected one adds the selection ring.
        ctx.strokeStyle = palette.panel;
        ctx.lineWidth = 2;
        ctx.stroke();
        if (s.selected) {
          ctx.beginPath();
          ctx.arc(s.at.x, s.at.y, r + 3, 0, Math.PI * 2);
          ctx.strokeStyle = palette.selection;
          ctx.lineWidth = 1.5;
          ctx.stroke();
        } else if (s.role === "crop") {
          ctx.strokeStyle = palette.selection;
          ctx.lineWidth = 1.5;
          ctx.strokeRect(s.at.x - r, s.at.y - r, s.size, s.size);
        }
        break;
      }
      case "label":
        if (s.role === "frame" && !s.selected) {
          ctx.font = FONT;
          ctx.fillStyle = palette.secondary;
          ctx.textBaseline = "alphabetic";
          ctx.fillText(s.text, s.at.x, s.at.y);
          break;
        }
        chip(ctx, s.text, s.at.x, s.at.y, s.anchor, palette, !!s.selected);
        break;
    }
  }
  ctx.restore();
}
