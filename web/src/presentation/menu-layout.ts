import type { Pose } from "../render/contracts/math.js";
import { MENU_MINIMUM_INK_HEIGHT_DEGREES } from "../render/contracts/menu-layout.js";
import type { MenuClippedRect, MenuContentPoint, MenuControlLayout, MenuDocumentResult, MenuDocument,
  MenuFontIdentity, MenuHit, MenuLayoutRequest, MenuMeasuredLine, MenuOpeningReadability, MenuPoint, MenuRect,
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

export function requestMenuLayout(panel: UiPanel, surfaceSize: PanelSize, style: Readonly<{
  theme: MenuLayoutRequest["theme"]; fontMeters: number; font: MenuFontIdentity; locale: string; previousLabel: string; nextLabel: string; caption: string;
}>): MenuLayoutRequest {
  const source: Source = { panelId: panel.id, title: panel.title, sections: panel.sections ?? null,
    controls: panel.controls.map(control => ({ id: control.id, label: control.label, section: control.section ?? null, widget: widget(control) })),
    surfaceSize, fontMeters: style.fontMeters, font: style.font, locale: style.locale,
    previousLabel: style.previousLabel, nextLabel: style.nextLabel, caption: style.caption };
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
  const rows = shared.rows.map(row => ({ heading: row.heading, controls: row.controls.map(item => {
    const control = panel.controls[item.index];
    if (control === undefined) throw new Error("Shared Menu row lost its source control");
    return { control, left: item.left, width: item.width };
  }) }));
  const texts = shared.texts.map(({ chartText, legend, ...text }) => ({ ...text,
    ...(chartText == null ? {} : { chartText }), ...(legend == null ? {} : { legend }) }));
  return Object.freeze({ panel, theme: style.theme, shared, surfaceSize: shared.source.surfaceSize,
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
    observed = { signature, geometry: reply.geometry, measurements: new Map(measurements.map(measurement => [
      measurement.identity, { identity: measurement.identity, lines: Object.freeze(measurement.lines.map(line => Object.freeze({ ...line }))) }
    ])) };
    const cached = documents.get(measurements) ?? new Map<Plan, MeasuredDocument>();
    cached.set(request.shared, observed);
    documents.set(measurements, cached);
  }
  const { geometry, measurements: retained } = observed;
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

export function menuViewport(document: MenuDocument, progress: number): MenuViewport {
  if (!Number.isFinite(progress) || progress < 0 || progress > 1) throw new RangeError("Menu scroll progress must be in [0, 1]");
  const request = document.request;
  const contentFits = document.contentHeightMeters <= request.surfaceSize.height - request.paddingMeters * 2;
  const footerHeight = contentFits ? 0 : document.footerHeightMeters;
  const footerTop = request.surfaceSize.height - request.paddingMeters - footerHeight;
  const contentClip = Object.freeze({ x: request.paddingMeters, y: request.paddingMeters,
    width: request.surfaceSize.width - request.paddingMeters * 2, height: footerTop - (contentFits ? 0 : request.gapMeters) - request.paddingMeters });
  const pageWidth = (contentClip.width - request.gapMeters) / 2;
  const maximumOffsetMeters = Math.max(0, document.contentHeightMeters - contentClip.height);
  return Object.freeze({ document, contentClip, progress, maximumOffsetMeters, offsetMeters: progress * maximumOffsetMeters,
    previousBounds: Object.freeze({ x: contentClip.x, y: footerTop, width: pageWidth, height: footerHeight }),
    nextBounds: Object.freeze({ x: contentClip.x + pageWidth + request.gapMeters, y: footerTop, width: pageWidth, height: footerHeight }) });
}

export function menuPanelPointToDocument(viewport: MenuViewport, point: MenuPoint): MenuContentPoint {
  if (![point.x, point.y].every(Number.isFinite)) return Object.freeze({ kind: "outside" });
  const surface = viewport.document.request.surfaceSize;
  const surfacePoint = { x: point.x + surface.width / 2, y: surface.height / 2 - point.y };
  return contains(viewport.contentClip, surfacePoint) ? Object.freeze({ kind: "inside", point: Object.freeze({ x: surfacePoint.x - viewport.contentClip.x,
    y: surfacePoint.y - viewport.contentClip.y + viewport.offsetMeters }) }) : Object.freeze({ kind: "outside" });
}

export function menuDocumentPointToPanel(viewport: MenuViewport, point: MenuPoint): MenuPoint {
  if (![point.x, point.y].every(Number.isFinite)) throw new RangeError("Menu document points must be finite");
  const surface = viewport.document.request.surfaceSize;
  return Object.freeze({ x: point.x + viewport.contentClip.x - surface.width / 2,
    y: surface.height / 2 - (point.y - viewport.offsetMeters + viewport.contentClip.y) });
}

export function menuRectInViewport(viewport: MenuViewport, rectangle: MenuRect): MenuClippedRect {
  const left = rectangle.x + viewport.contentClip.x;
  const top = rectangle.y - viewport.offsetMeters + viewport.contentClip.y;
  const clippedLeft = Math.max(left, viewport.contentClip.x);
  const clippedTop = Math.max(top, viewport.contentClip.y);
  const right = Math.min(left + rectangle.width, viewport.contentClip.x + viewport.contentClip.width);
  const bottom = Math.min(top + rectangle.height, viewport.contentClip.y + viewport.contentClip.height);
  return right > clippedLeft && bottom > clippedTop ? Object.freeze({ kind: "visible", rect: Object.freeze({ x: clippedLeft, y: clippedTop,
    width: right - clippedLeft, height: bottom - clippedTop }) }) : Object.freeze({ kind: "outside" });
}

export function hitMenuViewport(viewport: MenuViewport, point: MenuPoint): MenuHit {
  if (![point.x, point.y].every(Number.isFinite)) return Object.freeze({ kind: "none" });
  const size = viewport.document.request.surfaceSize;
  const surfacePoint = { x: point.x + size.width / 2, y: size.height / 2 - point.y };
  if (contains(viewport.previousBounds, surfacePoint)) return viewport.offsetMeters > 0 ? Object.freeze({ kind: "page", direction: "previous" }) : Object.freeze({ kind: "none" });
  if (contains(viewport.nextBounds, surfacePoint)) return viewport.offsetMeters < viewport.maximumOffsetMeters ? Object.freeze({ kind: "page", direction: "next" }) : Object.freeze({ kind: "none" });
  const documentPoint = menuPanelPointToDocument(viewport, point);
  if (documentPoint.kind === "outside") return Object.freeze({ kind: "none" });
  const layout = viewport.document.controls.find((control) => control.control.enabled && contains(control.bounds, documentPoint.point));
  return layout === undefined ? Object.freeze({ kind: "none" }) : Object.freeze({ kind: "control", layout, documentPoint: documentPoint.point });
}

function contains(rectangle: MenuRect, point: MenuPoint): boolean {
  return point.x >= rectangle.x && point.x <= rectangle.x + rectangle.width && point.y >= rectangle.y && point.y <= rectangle.y + rectangle.height;
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
