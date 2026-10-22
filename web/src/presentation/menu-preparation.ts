import type { MenuFontIdentity, MenuLayoutRequest, MenuTextMeasurement } from "../render/contracts/menu-layout.js";
import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, UiPanel } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { MenuLayoutQuery } from "../generated/menu-layout-query.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import { menu_layout_json } from "../../pkg/birdman_game_wasm.js";
import { menuCanvasSize, measureMenuLayout } from "./menu-canvas.js";
import type { MenuDrawingContext } from "./menu-canvas.js";
import { menuDocumentFromGeometry, menuLayoutRequest, menuLayoutSource, menuViewport } from "./menu-layout.js";

export type MenuMeasurementCache = Map<string, readonly MenuTextMeasurement[]>;

export function prepareMenuPresentation(context: MenuDrawingContext, panel: UiPanel, scroll: MenuScrollContext,
  progress: number, viewer: ViewerFrame, style: Readonly<{ theme: MenuLayoutRequest["theme"]; locale: string; caption: string; font: MenuFontIdentity }>, cache: MenuMeasurementCache): MenuPresentation {
  if (panel.anchor !== "menu") return { kind: "absent" };
  const source = menuLayoutSource(panel, panel.size, { ...style, fontMeters: 0.035, previousLabel: "前頁", nextLabel: "次頁" });
  let observation: Extract<MenuLayoutQuery, { kind: "prepare" }>["observation"] = null;
  let request: MenuLayoutRequest | null = null;
  for (;;) {
    const query: MenuLayoutQuery = { kind: "prepare", source, viewer, menu_from_panel: panel.localPose, observation };
    let reply: MenuLayoutReply;
    try { reply = JSON.parse(menu_layout_json(JSON.stringify(query))) as MenuLayoutReply; }
    catch (cause) {
      if (typeof cause === "string" || cause instanceof RangeError) return { kind: "unavailable", reason: "invalid-layout" };
      throw cause;
    }
    if (reply.kind === "unavailable") return reply;
    if (reply.kind === "ready") {
      const measured = observation?.measurements;
      if (request === null || measured === undefined) throw new Error("Menu preparation lost its measurement request");
      const document = menuDocumentFromGeometry(request, measured, reply.geometry);
      if (document.kind === "unavailable") return document;
      return { kind: "ready", panel: { ...panel, size: request.surfaceSize }, viewport: menuViewport(document.document, progress), context: scroll };
    }
    if (reply.kind !== "measure") throw new Error("Menu preparation returned an unexpected phase");
    const fittedPanel = { ...panel, size: reply.plan.source.surfaceSize, title: reply.plan.source.title };
    request = menuLayoutRequest(fittedPanel, style.theme, reply.plan);
    const dimensions = menuCanvasSize(request.surfaceSize);
    const key = JSON.stringify([dimensions.width, dimensions.height, ...request.texts.map(text => text.identity)]);
    const stored = cache.get(key);
    const measurement = stored === undefined ? measureMenuLayout(context, request, dimensions.width, dimensions.height)
      : { kind: "ready" as const, measurements: stored };
    if (measurement.kind === "unavailable") return measurement;
    if (stored === undefined) {
      if (cache.size >= 32) cache.clear();
      cache.set(key, measurement.measurements);
    }
    observation = { attempt: reply.attempt, measurements: measurement.measurements };
  }
}
