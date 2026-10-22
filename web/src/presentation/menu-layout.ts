import type { MenuClippedRect, MenuControlLayout, MenuDocumentResult, MenuDocument,
  MenuFontIdentity, MenuLayoutRequest, MenuPoint, MenuRect,
  MenuTextLayout, MenuTextMeasurement } from "../render/contracts/menu-layout.js";
import type { PanelSize, UiControl, UiPanel } from "../render/contracts/ui.js";
import { menu_layout_json } from "../../pkg/birdman_game_wasm.js";
import type { MenuLayoutQuery } from "../generated/menu-layout-query.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import type { MenuViewport } from "../render/contracts/menu-layout.js";

type Plan = Extract<MenuLayoutReply, { kind: "plan" }>["plan"];
type Source = Extract<MenuLayoutQuery, { kind: "plan" }>["source"];
type Geometry = Extract<MenuLayoutReply, { kind: "ready" }>["geometry"];
function widget(control: UiControl): Source["controls"][number]["widget"] {
  switch (control.kind) {
    case "button": {
      const presentation = control.presentation;
      return { kind: "button", presentation: presentation?.kind === "choice"
        ? { kind: "choice", group: presentation.group, group_label: presentation.groupLabel,
          selected: presentation.selected, description: presentation.description }
        : presentation ?? { kind: "action", emphasis: "normal" } };
    }
    case "toggle": return { kind: "toggle", value: control.value };
    case "range": return { kind: "range", displayValue: control.displayValue };
    case "status": return { kind: "status", value: control.value };
    case "chart": {
      const chart = control.presentation;
      const labels: Extract<Source["controls"][number]["widget"], { kind: "chart" }>["layout"]["labels"][number][] = [
        { role: "x-axis", index: 0, text: chart.x_axis }, { role: "y-axis", index: 0, text: chart.y_axis }
      ];
      if (chart.map_guides !== null) labels.push({ role: "north", index: 0, text: chart.map_guides.north_label },
        { role: "scale", index: 0, text: chart.map_guides.scale_label });
      chart.x_ticks.forEach((tick, index) => { labels.push({ role: "x-tick", index, text: tick.text }); });
      chart.y_ticks.forEach((tick, index) => { labels.push({ role: "y-tick", index, text: tick.text }); });
      [...chart.marker_labels, ...chart.time_marker_labels, ...chart.reference_labels]
        .forEach((text, index) => { labels.push({ role: "annotation", index, text }); });
      return { kind: "chart", layout: { size: [chart.viewport.width, chart.viewport.height], plot: chart.plot,
        tickFontSize: chart.tick_font_size, labels, legend: chart.legend } };
    }
  }
}

type MenuLayoutStyle = Readonly<{
  theme: MenuLayoutRequest["theme"]; fontMeters: number; font: MenuFontIdentity; locale: string; previousLabel: string; nextLabel: string; caption: string;
}>;

export function menuLayoutSource(panel: UiPanel, surfaceSize: PanelSize, style: MenuLayoutStyle): Source {
  return { panelId: panel.id, title: panel.title, sections: panel.sections ?? null,
    controls: panel.controls.map(control => ({ id: control.id, label: control.label, section: control.section ?? null, widget: widget(control) })),
    surfaceSize, fontMeters: style.fontMeters, font: style.font, locale: style.locale,
    previousLabel: style.previousLabel, nextLabel: style.nextLabel, caption: style.caption };
}

export function requestMenuLayout(panel: UiPanel, surfaceSize: PanelSize, style: MenuLayoutStyle): MenuLayoutRequest {
  const source = menuLayoutSource(panel, surfaceSize, style);
  let reply: MenuLayoutReply;
  try { reply = JSON.parse(menu_layout_json(JSON.stringify({ kind: "plan", source } satisfies MenuLayoutQuery))) as MenuLayoutReply; }
  catch (cause) { throw new RangeError(String(cause), { cause }); }
  if (reply.kind !== "plan") throw new RangeError("Menu layout source is invalid");
  return menuLayoutRequest(panel, style.theme, reply.plan);
}

export function menuLayoutRequest(panel: UiPanel, theme: MenuLayoutRequest["theme"], shared: Plan): MenuLayoutRequest {
  const rows = shared.rows.map(row => ({ heading: row.heading, controls: row.controls.map(item => {
    const control = panel.controls[item.index];
    if (control === undefined) throw new Error("Shared Menu row lost its source control");
    return { control, left: item.left, width: item.width };
  }) }));
  const texts = shared.texts.map(({ chartText, legend, ...text }) => ({ ...text,
    ...(chartText == null ? {} : { chartText }), ...(legend == null ? {} : { legend }) }));
  return Object.freeze({ panel, theme, shared, surfaceSize: shared.source.surfaceSize,
    fontMeters: shared.source.fontMeters, font: shared.source.font, lineHeightMeters: shared.lineHeightMeters,
    paddingMeters: shared.paddingMeters, gapMeters: shared.gapMeters, rows, texts });
}

export function layoutMeasuredMenu(request: MenuLayoutRequest, measurements: readonly MenuTextMeasurement[]): MenuDocumentResult {
  let reply: MenuLayoutReply;
  try {
    const query: MenuLayoutQuery = { kind: "arrange", source: request.shared.source, measurements };
    reply = JSON.parse(menu_layout_json(JSON.stringify(query))) as MenuLayoutReply;
  } catch { return { kind: "unavailable", reason: "invalid-text-metrics" }; }
  if (reply.kind === "unavailable") return reply;
  if (reply.kind !== "ready") throw new Error("Shared Menu arrangement returned an unexpected phase");
  return menuDocumentFromGeometry(request, measurements, reply.geometry);
}

function retainMeasurements(measurements: readonly MenuTextMeasurement[]): ReadonlyMap<string, MenuTextMeasurement> {
  return new Map(measurements.map(measurement => [measurement.identity, { identity: measurement.identity,
    lines: Object.freeze(measurement.lines.map(line => Object.freeze({ ...line }))) }]));
}

export function menuDocumentFromGeometry(request: MenuLayoutRequest, measurements: readonly MenuTextMeasurement[], geometry: Geometry): MenuDocumentResult {
  return projectMenuDocument(request, geometry, retainMeasurements(measurements));
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
