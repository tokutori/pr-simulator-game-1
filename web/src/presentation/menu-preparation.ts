import type { MenuFontIdentity, MenuFontMeasurer, MenuLayoutRequest } from "../render/contracts/menu-layout.js";
import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, UiPanel } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { MenuPreparationInput } from "../generated/menu-preparation-input.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import { menuCanvasSize } from "./menu-canvas.js";
import type { MenuDrawingContext } from "./menu-canvas.js";
import { menuDocumentFromGeometry, menuLayoutRequest, menuViewport } from "./menu-layout.js";

export function prepareMenuPresentation(context: MenuDrawingContext, panel: UiPanel, scroll: MenuScrollContext,
  progress: number, viewer: ViewerFrame, style: Readonly<{ theme: MenuLayoutRequest["theme"]; locale: string; font: MenuFontIdentity; viewRevision: number }>,
  prepare: (input: MenuPreparationInput, measure: MenuFontMeasurer) => MenuLayoutReply): MenuPresentation {
  if (panel.anchor !== "menu") return { kind: "absent" };
  context.save();
  try {
    let activeFont = "";
    context.setLocale(style.locale);
    context.setTextAlign("left");
    context.setTextBaseline("alphabetic");
    const reply = prepare({ panelId: panel.id, viewRevision: style.viewRevision, viewer, font: style.font, locale: style.locale },
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
    if (reply.kind !== "ready") throw new Error("Shared Menu preparation returned an unexpected reply");
    const fittedPanel = { ...panel, size: reply.plan.source.surfaceSize, title: reply.plan.source.title };
    const request = menuLayoutRequest(fittedPanel, style.theme, reply.plan);
    const result = menuDocumentFromGeometry(request, reply.measurements, reply.geometry);
    return result.kind === "unavailable" ? result
      : { kind: "ready", panel: fittedPanel, viewport: menuViewport(result.document, progress), context: scroll };
  } catch (cause) {
    if (typeof cause === "string" || cause instanceof RangeError) return { kind: "unavailable", reason: "invalid-layout" };
    throw cause;
  } finally {
    context.restore();
  }
}
