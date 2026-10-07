import { composePose } from "../render/contracts/math.js";
import type { MenuFontIdentity, MenuLayoutRequest, MenuTextMeasurement } from "../render/contracts/menu-layout.js";
import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, UiPanel } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import { menuCanvasSize, measureMenuLayout } from "./menu-canvas.js";
import type { HeadHudDrawingContext } from "./head-hud-canvas.js";
import { layoutMeasuredMenu, menuViewport, requestMenuLayout, validateMenuOpeningInk } from "./menu-layout.js";
import { fitMenuView } from "./menu-view-fit.js";

export type MenuMeasurementCache = Map<string, readonly MenuTextMeasurement[]>;

export function prepareMenuPresentation(context: HeadHudDrawingContext, panel: UiPanel, scroll: MenuScrollContext,
  progress: number, viewer: ViewerFrame, style: Readonly<{ locale: string; caption: string; font: MenuFontIdentity }>, cache: MenuMeasurementCache): MenuPresentation {
  if (panel.anchor !== "menu") return Object.freeze({ kind: "absent" });
  if (viewer.source === "unavailable") return Object.freeze({ kind: "unavailable", reason: viewer.reason });
  let surface = panel.size;
  let fit = fitMenuView(viewer, surface, panel.localPose);
  if (fit.kind === "unavailable") return fit;
  for (let attempt = 0; attempt < 12 && fit.distanceMeters > 2.6 && surface.width > 0.35; attempt++) {
    surface = Object.freeze({ width: Math.max(0.35, surface.width * 0.85), height: surface.height });
    fit = fitMenuView(viewer, surface, panel.localPose);
    if (fit.kind === "unavailable") return fit;
  }
  const compact = panel.size.height < 0.6;
  const fittedPanel: UiPanel = Object.freeze({ ...panel, size: surface });
  const layoutPanel: UiPanel = compact ? Object.freeze({ ...fittedPanel, title: "" }) : fittedPanel;
  const dimensions = menuCanvasSize(surface);
  const headFromSurface = composePose(fit.headFromMenu, panel.localPose);
  let fontMeters = 0.035;
  for (let attempt = 0; attempt < 12; attempt++, fontMeters *= 1.15) {
    let request: MenuLayoutRequest;
    try {
      request = requestMenuLayout(layoutPanel, surface, { ...style, caption: compact ? "" : style.caption,
        fontMeters, previousLabel: "前頁", nextLabel: "次頁" });
    } catch (error) {
      if (error instanceof RangeError) return Object.freeze({ kind: "unavailable", reason: "invalid-layout" });
      throw error;
    }
    const measurementKey = JSON.stringify([dimensions.width, dimensions.height, ...request.texts.map((text) => text.identity)]);
    const stored = cache.get(measurementKey);
    const measurement = stored === undefined ? measureMenuLayout(context, request, dimensions.width, dimensions.height)
      : Object.freeze({ kind: "ready" as const, measurements: stored });
    if (measurement.kind === "unavailable") return measurement;
    if (stored === undefined) {
      if (cache.size >= 32) cache.clear();
      cache.set(measurementKey, measurement.measurements);
    }
    const document = layoutMeasuredMenu(request, measurement.measurements);
    if (document.kind === "unavailable") return document;
    const readability = validateMenuOpeningInk(document.document, viewer, headFromSurface);
    if (readability.kind === "ready") return Object.freeze({ kind: "ready", panel: fittedPanel,
      viewport: menuViewport(document.document, progress), context: scroll });
    if (readability.reason !== "insufficient-ink-angle") return readability;
  }
  return Object.freeze({ kind: "unavailable", reason: "insufficient-ink-angle" });
}
