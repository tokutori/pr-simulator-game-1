import type { Pose } from "../render/contracts/math.js";
import { MENU_MINIMUM_INK_HEIGHT_DEGREES } from "../render/contracts/menu-layout.js";
import type { MenuClippedRect, MenuControlLayout, MenuDocumentResult, MenuDocument,
  MenuFontIdentity, MenuLayoutRequest, MenuMeasuredLine, MenuOpeningReadability, MenuPoint, MenuRect,
  MenuTextLayout, MenuTextMeasurement } from "../render/contracts/menu-layout.js";
import type { PanelSize, UiControl, UiPanel } from "../render/contracts/ui.js";
import { menu_layout_json, xr_text_ink_fit_json } from "../../pkg/birdman_game_wasm.js";
import type { MenuLayoutQuery } from "../generated/menu-layout-query.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import type { XrInkFit } from "../generated/xr-ink-fit.js";
import type { XrInkSegment } from "../generated/xr-ink-segment.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { MenuViewport } from "../render/contracts/menu-layout.js";

type Plan = Extract<MenuLayoutReply, { kind: "plan" }>["plan"];
type Source = Extract<MenuLayoutQuery, { kind: "plan" }>["source"];
type Geometry = Extract<MenuLayoutReply, { kind: "ready" }>["geometry"];
const plans = new Map<string, Plan>();
interface MeasuredDocument {
  readonly signature: string;
  readonly geometry: Geometry;
  readonly measurements: ReadonlyMap<string, MenuTextMeasurement>;
}
const documents = new WeakMap<readonly MenuTextMeasurement[], Map<Plan, MeasuredDocument>>();

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
  const key = JSON.stringify({ kind: "plan", source } satisfies MenuLayoutQuery);
  let shared = plans.get(key);
  if (shared === undefined) {
    let reply: MenuLayoutReply;
    try { reply = JSON.parse(menu_layout_json(key)) as MenuLayoutReply; }
    catch (cause) { throw new RangeError(String(cause), { cause }); }
    if (reply.kind !== "plan") throw new RangeError("Menu layout source is invalid");
    shared = reply.plan;
    if (plans.size >= 32) plans.clear();
    plans.set(key, shared);
  }
  return menuLayoutRequest(panel, style.theme, shared);
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
  const signature = JSON.stringify(measurements);
  let observed = documents.get(measurements)?.get(request.shared);
  if (observed?.signature !== signature) {
    let reply: MenuLayoutReply;
    try {
      const query: MenuLayoutQuery = { kind: "arrange", source: request.shared.source, measurements };
      reply = JSON.parse(menu_layout_json(JSON.stringify(query))) as MenuLayoutReply;
    } catch { return { kind: "unavailable", reason: "invalid-text-metrics" }; }
    if (reply.kind === "unavailable") return reply;
    if (reply.kind !== "ready") throw new Error("Shared Menu arrangement returned an unexpected phase");
    observed = { signature, geometry: reply.geometry, measurements: retainMeasurements(measurements) };
    const cached = documents.get(measurements) ?? new Map<Plan, MeasuredDocument>();
    cached.set(request.shared, observed);
    documents.set(measurements, cached);
  }
  return projectMenuDocument(request, observed.geometry, observed.measurements);
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

export function validateMenuOpeningInk(document: MenuDocument, viewer: ViewerFrame, headFromSurface: Pose): MenuOpeningReadability {
  if (viewer.source === "unavailable") return { kind: "unavailable", reason: viewer.reason };
  const size = document.request.surfaceSize;
  const lines: readonly MenuMeasuredLine[] = [...document.title.lines, ...document.caption.lines,
    ...document.sectionHeadings.flatMap((heading) => heading.lines),
    ...document.controls.flatMap((control) => control.texts.flatMap((text) => text.lines)), ...document.previous.lines, ...document.next.lines];
  const segments: XrInkSegment[] = [];
  for (const line of lines.filter((line) => line.value.trim() !== "")) {
    const height = line.ascentMeters + line.descentMeters;
    if (height <= 0 || height > size.height) return { kind: "unavailable", reason: "insufficient-ink-angle" };
    for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
      segments.push({ start: { x: horizontal * size.width / 2, y: vertical * size.height / 2, z: 0 },
        end: { x: horizontal * size.width / 2, y: vertical * (size.height / 2 - height), z: 0 } });
    }
  }
  try {
    const fit = JSON.parse(xr_text_ink_fit_json(JSON.stringify(viewer), JSON.stringify(segments), JSON.stringify(headFromSurface),
      MENU_MINIMUM_INK_HEIGHT_DEGREES)) as XrInkFit;
    return fit.kind === "empty" || fit.kind === "too-small" ? { kind: "unavailable", reason: "insufficient-ink-angle" } : fit;
  } catch (cause) {
    if (typeof cause === "string" || cause instanceof RangeError) return { kind: "unavailable", reason: "invalid-view-geometry" };
    throw cause;
  }
}
