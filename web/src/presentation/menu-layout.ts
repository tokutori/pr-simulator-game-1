import type { Pose } from "../render/contracts/math.js";
import { MENU_MINIMUM_INK_HEIGHT_DEGREES } from "../render/contracts/menu-layout.js";
import type { MenuClippedRect, MenuContentPoint, MenuControlLayout, MenuControlTextRole, MenuDocument, MenuDocumentResult,
  MenuFixedTextRole, MenuFontIdentity, MenuHit, MenuLayoutRequest,
  MenuLayoutRow, MenuMeasuredLine, MenuOpeningReadability, MenuPoint, MenuRect, MenuTextLayout, MenuTextMeasurement, MenuTextRequest,
  MenuTextSlot, MenuViewport } from "../render/contracts/menu-layout.js";
import { uiButtonLabel } from "../render/contracts/ui.js";
import type { PanelSize, UiChart, UiPanel } from "../render/contracts/ui.js";
import { xr_text_ink_fit_json } from "../../pkg/birdman_game_wasm.js";
import type { XrInkFit } from "../generated/xr-ink-fit.js";
import type { XrInkSegment } from "../generated/xr-ink-segment.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";

export function requestMenuLayout(panel: UiPanel, surfaceSize: PanelSize, style: Readonly<{
  theme: MenuLayoutRequest["theme"]; fontMeters: number; font: MenuFontIdentity; locale: string; previousLabel: string; nextLabel: string; caption: string;
}>): MenuLayoutRequest {
  if (![surfaceSize.width, surfaceSize.height, style.fontMeters].every((value) => Number.isFinite(value) && value > 0) ||
      style.locale.trim() === "" || style.previousLabel.trim() === "" || style.nextLabel.trim() === "" ||
      style.font.family.trim() === "" || !Number.isInteger(style.font.weight) || style.font.weight < 1 || style.font.weight > 1000 ||
      !Number.isSafeInteger(style.font.generation) || style.font.generation < 0 ||
      new Set(panel.controls.map((control) => control.id)).size !== panel.controls.length) {
    throw new RangeError("Menu layout requires finite dimensions, an explicit locale and unique controls");
  }
  const paddingMeters = style.fontMeters * 0.5;
  const gapMeters = style.fontMeters * 0.35;
  const contentWidth = surfaceSize.width - paddingMeters * 2;
  if (contentWidth <= paddingMeters * 2) throw new RangeError("Menu surface is narrower than its text padding");
  const rows = menuRows(panel, surfaceSize.width < surfaceSize.height, contentWidth, style.fontMeters);
  const font = Object.freeze({ ...style.font });
  const texts: MenuTextRequest[] = [];
  const addText = (slot: MenuTextSlot, value: string, widthMeters: number, chartText?: UiChart["presentation"]["x_axis"], legend?: UiChart["presentation"]["legend"][number]): void => {
    const chart = slot.kind === "control" ? panel.controls.find((control) => control.id === slot.controlId) : undefined;
    const fontMeters = chartText === undefined || chart?.kind !== "chart" ? style.fontMeters
      : style.fontMeters * chartText.font_size / chart.presentation.tick_font_size;
    const textFont = chartText === undefined ? font : Object.freeze({ ...font, weight: chartText.font_weight });
    const identity = JSON.stringify([panel.id, surfaceSize.width, surfaceSize.height, style.locale, style.fontMeters,
      textFont.family, textFont.weight, textFont.style, textFont.generation, slot, value, widthMeters, chartText, legend]);
    texts.push(Object.freeze({ identity, slot: Object.freeze(slot), value, widthMeters, fontMeters, font: textFont, locale: style.locale,
      ...(chartText === undefined ? {} : { chartText }), ...(legend === undefined ? {} : { legend }) }));
  };
  addText({ kind: "fixed", role: "title" }, panel.title, contentWidth);
  addText({ kind: "fixed", role: "caption" }, style.caption, contentWidth);
  const pageWidth = (contentWidth - gapMeters) / 2 - paddingMeters * 2;
  if (pageWidth <= 0) throw new RangeError("Menu surface has no space for page controls");
  addText({ kind: "fixed", role: "previous" }, style.previousLabel, pageWidth);
  addText({ kind: "fixed", role: "next" }, style.nextLabel, pageWidth);
  for (const row of rows) {
    if (row.heading !== null && row.heading.label !== "") addText({ kind: "section", role: "heading", sectionId: row.heading.id }, row.heading.label, contentWidth);
    for (const item of row.controls) {
    const control = item.control;
    const width = item.width - paddingMeters * 2;
    const addControlText = (role: MenuControlTextRole, index: number, value: string, textWidth = width, chartText?: UiChart["presentation"]["x_axis"]): void => {
      addText({ kind: "control", controlId: control.id, role, index }, value, textWidth, chartText);
    };
    addControlText("label", 0, control.kind === "button" ? uiButtonLabel(control) : control.label);
    switch (control.kind) {
      case "button": break;
      case "toggle": addControlText("value", 0, control.value ? "On" : "Off"); break;
      case "range": addControlText("value", 0, control.displayValue); break;
      case "status": addControlText("value", 0, control.value); break;
      case "chart": {
        const presentation = control.presentation;
        addControlText("x-axis", 0, presentation.x_axis.label, width, presentation.x_axis);
        addControlText("y-axis", 0, presentation.y_axis.label, width, presentation.y_axis);
        if (presentation.map_guides !== null) {
          addControlText("north", 0, presentation.map_guides.north_label.label, width, presentation.map_guides.north_label);
          addControlText("scale", 0, presentation.map_guides.scale_label.label, width, presentation.map_guides.scale_label);
        }
        presentation.x_ticks.forEach((tick, index) => { addControlText("x-tick", index, tick.text.label, width / presentation.x_ticks.length, tick.text); });
        presentation.y_ticks.forEach((tick, index) => { addControlText("y-tick", index, tick.text.label, width / presentation.y_ticks.length, tick.text); });
        presentation.legend.forEach((entry, index) => {
          addText({ kind: "control", controlId: control.id, role: "legend", index }, entry.label, width - style.fontMeters * 2, undefined, entry);
        });
        [...presentation.marker_labels, ...presentation.time_marker_labels, ...presentation.reference_labels]
          .forEach((annotation, index) => { addControlText("annotation", index, annotation.label, width, annotation); });
        break;
      }
    }
  }
  }
  return Object.freeze({ panel, theme: style.theme, surfaceSize: Object.freeze({ ...surfaceSize }), fontMeters: style.fontMeters, font,
    lineHeightMeters: style.fontMeters * 1.35, paddingMeters, gapMeters, rows, texts: Object.freeze(texts) });
}

function menuRows(panel: UiPanel, portrait: boolean, width: number, fontMeters: number): readonly MenuLayoutRow[] {
  const sections = panel.sections ?? [{ id: "", label: "", layout: "column" as const }];
  if (new Set(sections.map((section) => section.id)).size !== sections.length ||
      panel.sections !== undefined && panel.controls.some((control) => !sections.some((section) => section.id === control.section))) {
    throw new RangeError("Menu controls must belong to unique declared sections");
  }
  return Object.freeze(sections.flatMap((section) => {
    const controls = panel.sections === undefined ? panel.controls : panel.controls.filter((control) => control.section === section.id);
    const limit = portrait ? 1 : Math.max(1, Math.floor(width / (fontMeters * 8)));
    let columns: number;
    switch (section.layout) {
      case "column": columns = 1; break;
      case "row": columns = Math.min(limit, Math.max(1, controls.length)); break;
      case "choice_grid": columns = Math.min(limit, 3); break;
    }
    return Array.from({ length: Math.ceil(controls.length / columns) }, (_, row) => Object.freeze({
      heading: row === 0 ? Object.freeze({ id: section.id, label: section.label }) : null,
      controls: Object.freeze(controls.slice(row * columns, (row + 1) * columns).map((control, column) =>
        Object.freeze({ control, left: column * width / columns, width: width / columns })))
    }));
  }));
}

export function layoutMeasuredMenu(request: MenuLayoutRequest, measurements: readonly MenuTextMeasurement[]): MenuDocumentResult {
  const expected = new Map(request.texts.map((text) => [text.identity, text]));
  const measured = new Map<string, MenuTextMeasurement>();
  for (const measurement of measurements) {
    const specification = expected.get(measurement.identity);
    if (specification === undefined || measured.has(measurement.identity)) return unavailable("stale-text-metrics");
    if (!validMeasurement(specification, measurement, request.lineHeightMeters)) return unavailable("invalid-text-metrics");
    measured.set(measurement.identity, Object.freeze({ identity: measurement.identity,
      lines: Object.freeze(measurement.lines.map((line) => Object.freeze({ ...line }))) }));
  }
  if (measured.size !== expected.size) return unavailable("missing-text-metrics");
  const textLayout = (specification: MenuTextRequest, left: number, top: number): MenuTextLayout => {
    const measurement = measured.get(specification.identity);
    if (measurement === undefined) throw new Error("Validated Menu measurement is missing");
    return Object.freeze({ request: specification, lines: measurement.lines,
      bounds: Object.freeze({ x: left, y: top, width: specification.widthMeters,
        height: specification.value === "" ? 0 : measurement.lines.length * request.lineHeightMeters }) });
  };
  const required = (role: MenuFixedTextRole): MenuTextRequest => {
    const specification = request.texts.find((text) => text.slot.kind === "fixed" && text.slot.role === role);
    if (specification === undefined) throw new Error("Menu layout request is missing fixed text");
    return specification;
  };
  const title = textLayout(required("title"), 0, 0);
  let top = title.bounds.height + request.gapMeters;
  const controls: MenuControlLayout[] = [];
  const sectionHeadings: MenuTextLayout[] = [];
  for (const row of request.rows) {
    if (row.heading !== null && row.heading.label !== "") {
      const specification = request.texts.find((text) => text.slot.kind === "section" && text.slot.sectionId === row.heading?.id);
      if (specification === undefined) return unavailable("invalid-layout");
      const heading = textLayout(specification, 0, top);
      sectionHeadings.push(heading);
      top += heading.bounds.height + request.gapMeters;
    }
    const layouts = row.controls.map((item): MenuControlLayout => {
      const specifications = request.texts.filter((text) => text.slot.kind === "control" && text.slot.controlId === item.control.id);
      const texts: MenuTextLayout[] = [];
      let textTop = top + request.paddingMeters;
      const placeText = (specification: MenuTextRequest): void => {
        const text = textLayout(specification, item.left + request.paddingMeters + (specification.legend === undefined ? 0 : request.fontMeters * 2), textTop);
        texts.push(text);
        textTop += text.bounds.height;
      };
      specifications.filter((text) => text.slot.role === "label" || text.slot.role === "value").forEach(placeText);
      if (item.control.kind !== "chart") return Object.freeze({ kind: "control", control: item.control,
        bounds: Object.freeze({ x: item.left, y: top, width: item.width, height: textTop - top + request.paddingMeters }), texts: Object.freeze(texts) });
      const presentation = item.control.presentation;
      const scale = (item.width - request.paddingMeters * 2) / presentation.viewport.width;
      const [left, chartTop, right, bottom] = presentation.plot;
      const origin = { x: item.left + request.paddingMeters, y: textTop };
      const plot = Object.freeze({ x: item.left + request.paddingMeters + left * scale,
        y: textTop + chartTop * scale, width: (right - left) * scale, height: (bottom - chartTop) * scale });
      for (const specification of specifications.filter((text) => text.chartText !== undefined)) {
        const source = specification.chartText;
        if (source === undefined) throw new Error("Chart label is missing its shared descriptor");
        const text = textLayout(specification, 0, 0);
        const baseline = Object.freeze({ x: origin.x + source.position.x * scale, y: origin.y + source.position.y * scale });
        const chartProjection = Object.freeze({ baseline, rotationDegrees: source.rotation_degrees, anchor: source.anchor });
        const rectangles = text.lines.map((line, index) => menuChartTextLineGeometry(chartProjection, line, index, request.lineHeightMeters).bounds);
        const left = Math.min(...rectangles.map((rect) => rect.x));
        const right = Math.max(...rectangles.map((rect) => rect.x + rect.width));
        const top = Math.min(...rectangles.map((rect) => rect.y));
        const bottom = Math.max(...rectangles.map((rect) => rect.y + rect.height));
        texts.push(Object.freeze({ ...text, bounds: Object.freeze({ x: left, y: top, width: right - left, height: bottom - top }),
          chartProjection }));
      }
      textTop += presentation.viewport.height * scale + request.gapMeters;
      specifications.filter((text) => text.slot.role === "legend").forEach(placeText);
      return Object.freeze({ kind: "chart", control: item.control, plot,
        bounds: Object.freeze({ x: item.left, y: top, width: item.width, height: textTop - top + request.paddingMeters }), texts: Object.freeze(texts) });
    });
    controls.push(...layouts);
    top += Math.max(...layouts.map((layout) => layout.bounds.height)) + request.gapMeters;
  }
  const caption = textLayout(required("caption"), 0, top);
  const previous = measured.get(required("previous").identity);
  const next = measured.get(required("next").identity);
  if (previous === undefined || next === undefined) throw new Error("Validated page labels are missing");
  const footerHeightMeters = Math.max(previous.lines.length, next.lines.length) * request.lineHeightMeters + request.paddingMeters * 2;
  const contentHeightMeters = top + caption.bounds.height;
  if (![contentHeightMeters, footerHeightMeters].every(Number.isFinite) ||
      request.surfaceSize.height - footerHeightMeters - request.paddingMeters * 2 - request.gapMeters <= 0 ||
      controls.some((control) => !finiteRect(control.bounds) || (control.kind === "chart" && !finiteRect(control.plot)))) return unavailable("invalid-layout");
  return Object.freeze({ kind: "ready", document: Object.freeze({ request, title, caption, sectionHeadings: Object.freeze(sectionHeadings), controls: Object.freeze(controls),
    contentHeightMeters, previous, next, footerHeightMeters }) });
}

export function menuChartTextLineGeometry(projection: NonNullable<MenuTextLayout["chartProjection"]>, line: MenuMeasuredLine,
  index: number, lineHeightMeters: number): Readonly<{ anchor: MenuPoint; bounds: MenuRect }> {
  const rotation = projection.rotationDegrees * Math.PI / 180;
  const shift = projection.anchor === "middle" ? -line.advanceMeters / 2 : projection.anchor === "end" ? -line.advanceMeters : 0;
  const project = (coordinateX: number, coordinateY: number) => ({ x: projection.baseline.x + coordinateX * Math.cos(rotation) - coordinateY * Math.sin(rotation),
    y: projection.baseline.y + coordinateX * Math.sin(rotation) + coordinateY * Math.cos(rotation) });
  const corners = [shift - line.leftMeters, shift + line.rightMeters].flatMap((coordinateX) =>
    [index * lineHeightMeters - line.ascentMeters, index * lineHeightMeters + line.descentMeters].map((coordinateY) => project(coordinateX, coordinateY)));
  const left = Math.min(...corners.map((point) => point.x));
  const top = Math.min(...corners.map((point) => point.y));
  return Object.freeze({ anchor: Object.freeze(project(shift, index * lineHeightMeters)),
    bounds: Object.freeze({ x: left, y: top, width: Math.max(...corners.map((point) => point.x)) - left,
      height: Math.max(...corners.map((point) => point.y)) - top }) });
}

function validMeasurement(request: MenuTextRequest, measurement: MenuTextMeasurement, lineHeight: number): boolean {
  const paragraphs = request.value.split("\n");
  const reconstructed: string[] = [];
  for (const line of measurement.lines) {
    if (!Number.isInteger(line.paragraphIndex) || line.paragraphIndex < 0 || line.paragraphIndex >= paragraphs.length ||
        line.paragraphIndex > reconstructed.length || line.paragraphIndex < reconstructed.length - 1 || line.value.includes("\n")) return false;
    if (line.value === "" && (paragraphs[line.paragraphIndex] !== "" || line.paragraphIndex < reconstructed.length)) return false;
    if (line.paragraphIndex === reconstructed.length) reconstructed.push("");
    reconstructed[line.paragraphIndex] = (reconstructed[line.paragraphIndex] ?? "") + line.value;
  }
  return reconstructed.length === paragraphs.length && reconstructed.every((paragraph, index) => paragraph === paragraphs[index]) &&
    measurement.lines.every((line) => [line.advanceMeters, line.leftMeters, line.rightMeters, line.ascentMeters, line.descentMeters].every(Number.isFinite) &&
      line.advanceMeters >= 0 && line.leftMeters + line.rightMeters >= 0 &&
      Math.max(line.advanceMeters, Math.max(0, line.leftMeters) + line.rightMeters) <= request.widthMeters + 1e-10 &&
      line.ascentMeters + line.descentMeters <= lineHeight &&
      (line.value.trim() === "" ? line.ascentMeters + line.descentMeters >= 0 : line.ascentMeters + line.descentMeters > 0));
}

function unavailable(reason: Extract<MenuDocumentResult, { kind: "unavailable" }>["reason"]): MenuDocumentResult {
  return Object.freeze({ kind: "unavailable", reason });
}

function finiteRect(rectangle: MenuRect): boolean {
  return [rectangle.x, rectangle.y, rectangle.width, rectangle.height].every(Number.isFinite) && rectangle.width > 0 && rectangle.height > 0;
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
