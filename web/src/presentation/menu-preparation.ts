import type { MenuFontIdentity, MenuFontMeasurer } from "../render/contracts/menu-layout.js";
import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, UiPanel } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { MenuPreparationInput } from "../generated/menu-preparation-input.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import { menuCanvasSize } from "./menu-canvas.js";
import type { MenuDrawingContext } from "./menu-canvas.js";

export function prepareMenuPresentation(context: MenuDrawingContext, panel: UiPanel, scroll: MenuScrollContext,
  progress: number, viewer: ViewerFrame, style: Readonly<{ locale: string; font: MenuFontIdentity; viewRevision: number }>,
  prepare: (input: MenuPreparationInput, measure: MenuFontMeasurer) => MenuLayoutReply): MenuPresentation {
  if (panel.anchor !== "menu") return { kind: "absent" };
  context.save();
  try {
    let activeFont = "";
    context.setLocale(style.locale);
    context.setTextAlign("left");
    context.setTextBaseline("alphabetic");
    const reply = prepare({ panelId: panel.id, viewRevision: style.viewRevision, viewer, font: style.font, locale: style.locale, progress, generation: scroll.generation },
      (value, fontMeters, weight, width, height) => {
        const size = menuCanvasSize({ width, height });
        const verticalScale = height / size.height;
        const horizontalScale = width / size.width;
        const font = `${style.font.style} ${String(weight)} ${String(fontMeters / verticalScale)}px ${style.font.family}`;
        if (font !== activeFont) { context.setFont(font); activeFont = font; }
        const metric = context.measureText(value);
        return Float64Array.of(metric.width * horizontalScale, metric.left * horizontalScale,
          metric.right * horizontalScale, metric.ascent * verticalScale, metric.descent * verticalScale);
      });
    if (reply.kind === "unavailable") return reply;
    const fittedPanel = { ...panel, size: reply.projection.size, title: reply.projection.title };
    return { kind: "ready", panel: fittedPanel, viewport: reply.projection, context: scroll };
  } catch (cause) {
    if (typeof cause === "string" || cause instanceof RangeError) return { kind: "unavailable", reason: "invalid-layout" };
    throw cause;
  } finally {
    context.restore();
  }
}
