import type { MenuClippedRect, MenuControlLayout, MenuDocumentResult, MenuDocument,
  MenuLayoutRequest, MenuPoint, MenuRect,
  MenuTextLayout, MenuTextMeasurement } from "../render/contracts/menu-layout.js";
import type { UiPanel } from "../render/contracts/ui.js";
import { menu_layout_json } from "../../pkg/birdman_game_wasm.js";
import type { MenuLayoutQuery } from "../generated/menu-layout-query.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import type { MenuViewport } from "../render/contracts/menu-layout.js";

type Plan = Extract<MenuLayoutReply, { kind: "ready" }>["plan"];
type Geometry = Extract<MenuLayoutReply, { kind: "ready" }>["geometry"];

export function menuLayoutRequest(panel: UiPanel, theme: MenuLayoutRequest["theme"], shared: Plan): MenuLayoutRequest {
  return Object.freeze({ panel, theme, surfaceSize: shared.source.surfaceSize,
    fontMeters: shared.source.fontMeters, font: shared.source.font, lineHeightMeters: shared.lineHeightMeters,
    paddingMeters: shared.paddingMeters, gapMeters: shared.gapMeters, texts: shared.texts });
}

export function menuDocumentFromGeometry(request: MenuLayoutRequest, measurements: readonly MenuTextMeasurement[], geometry: Geometry): MenuDocumentResult {
  return projectMenuDocument(request, geometry, new Map(measurements.map(measured => [measured.identity, measured])));
}

function projectMenuDocument(request: MenuLayoutRequest, geometry: Geometry, retained: ReadonlyMap<string, MenuTextMeasurement>): MenuDocumentResult {
  const placeText = (placed: Geometry["title"]): MenuTextLayout => {
    const text = request.texts[placed.textIndex];
    if (text === undefined) throw new Error("Shared Menu text lost its measurement request");
    const measured = retained.get(text.identity);
    if (measured === undefined) throw new Error("Shared Menu text lost its validated measurement");
    return { request: text, lines: measured.lines, bounds: placed.bounds, lineGeometry: placed.lineGeometry,
      ...(placed.chartProjection == null ? {} : { chartProjection: placed.chartProjection }) };
  };
  const controls = geometry.controls.map((placed): MenuControlLayout => {
    const control = request.panel.controls[placed.controlIndex];
    if (control === undefined) throw new Error("Shared Menu placement lost its source control");
    const base = { bounds: placed.bounds, texts: placed.texts.map(placeText) };
    if (control.kind !== "chart") return { ...base, kind: "control", control };
    if (placed.plot === null) throw new Error("Shared Menu chart lost its plot geometry");
    return { ...base, kind: "chart", control, plot: placed.plot };
  });
  const measurementAt = (index: number): MenuTextMeasurement => {
    const text = request.texts[index];
    const measurement = text === undefined ? undefined : retained.get(text.identity);
    if (measurement === undefined) throw new Error("Shared Menu page lost its measurement");
    return measurement;
  };
  return { kind: "ready", document: Object.freeze({ request, title: placeText(geometry.title), caption: placeText(geometry.caption),
    sectionHeadings: geometry.sectionHeadings.map(placeText), controls: Object.freeze(controls), contentHeightMeters: geometry.contentHeightMeters,
    footerHeightMeters: geometry.footerHeightMeters, previous: measurementAt(geometry.previousIndex), next: measurementAt(geometry.nextIndex) }) };
}

type ViewportProjection = Extract<MenuLayoutReply, { kind: "viewport" }>["projection"];

function viewportProjection(document: MenuDocument, progress: number, rectangles: readonly MenuRect[],
  panelPoints: readonly MenuPoint[], documentPoints: readonly MenuPoint[]): ViewportProjection {
  const request = document.request;
  const query: MenuLayoutQuery = { kind: "viewport", source: { surfaceSize: request.surfaceSize,
    paddingMeters: request.paddingMeters, gapMeters: request.gapMeters,
    contentHeightMeters: document.contentHeightMeters, footerHeightMeters: document.footerHeightMeters, progress },
    rectangles, panel_points: panelPoints, document_points: documentPoints };
  let reply: MenuLayoutReply;
  try { reply = JSON.parse(menu_layout_json(JSON.stringify(query))) as MenuLayoutReply; }
  catch (cause) { throw new RangeError("Menu viewport geometry is invalid", { cause }); }
  if (reply.kind !== "viewport") throw new RangeError("Menu viewport geometry is invalid");
  return reply.projection;
}

export function projectMenuViewport(viewport: MenuViewport, rectangles: readonly MenuRect[] = [],
  panelPoints: readonly MenuPoint[] = [], documentPoints: readonly MenuPoint[] = []): ViewportProjection {
  return viewportProjection(viewport.document, viewport.progress, rectangles, panelPoints, documentPoints);
}

export function menuViewport(document: MenuDocument, progress: number): MenuViewport {
  const projection = viewportProjection(document, progress, [], [], []);
  return Object.freeze({ document, ...projection });
}

export function menuRectInViewport(viewport: MenuViewport, rectangle: MenuRect): MenuClippedRect {
  const projected = projectMenuViewport(viewport, [rectangle]).rectangles[0];
  if (projected === undefined) throw new Error("Menu rectangle lost its projection");
  return projected.visible === null ? { kind: "outside" } : { kind: "visible", rect: projected.visible };
}
