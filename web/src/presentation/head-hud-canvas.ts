import type { HeadHudPreparationInput } from "../generated/head-hud-preparation-input.js";
import type { HeadHudPaintReply } from "../generated/head-hud-paint-reply.js";
import type { MenuFontMeasurer } from "../render/contracts/menu-layout.js";
import type { SurfaceDrawingContext } from "./browser-canvas.js";
import { paintSurface } from "./surface-canvas.js";

export function prepareHeadHudPresentation(context: SurfaceDrawingContext, input: HeadHudPreparationInput,
  prepare: (input: HeadHudPreparationInput, measure: MenuFontMeasurer) => HeadHudPaintReply): HeadHudPaintReply {
  context.save();
  try {
    let activeFont = "";
    context.setLocale(input.locale);
    context.setTextAlign("left");
    context.setTextBaseline("alphabetic");
    return prepare(input, (value, fontMeters, weight, width, height) => {
      const scale = 2048 / Math.max(width, height);
      const horizontalScale = width / Math.max(1, Math.round(width * scale));
      const verticalScale = height / Math.max(1, Math.round(height * scale));
      const font = `${input.font.style} ${String(weight)} ${String(fontMeters / verticalScale)}px ${input.font.family}`;
      if (font !== activeFont) { context.setFont(font); activeFont = font; }
      const metric = context.measureText(value);
      return Float64Array.of(metric.width * horizontalScale, metric.left * horizontalScale,
        metric.right * horizontalScale, metric.ascent * verticalScale, metric.descent * verticalScale);
    });
  } finally { context.restore(); }
}

export function drawHeadHud(context: SurfaceDrawingContext, preparation: HeadHudPaintReply,
  canvasWidth: number, canvasHeight: number): void {
  context.clearRect(0, 0, canvasWidth, canvasHeight);
  if (preparation.kind !== "ready") return;
  context.save();
  try {
    const projection = preparation.projection;
    paintSurface(context, projection.items, canvasWidth / projection.size.width, canvasHeight / projection.size.height);
  } finally { context.restore(); }
}
