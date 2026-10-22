import { menu_measure_text_json } from "../../pkg/birdman_game_wasm.js";
import type { MenuLayoutReply } from "../generated/menu-layout-reply.js";
import type { MenuLayoutRequest, MenuRect, MenuTextLayout, MenuTextMeasurement,
  MenuTextRequest, MenuViewport } from "../render/contracts/menu-layout.js";
import { uiControlBackground } from "../render/contracts/ui.js";
import type { UiChart } from "../render/contracts/ui.js";
import { browserHeadHudContext } from "./head-hud-canvas.js";
import type { HeadHudDrawingContext } from "./head-hud-canvas.js";
import { projectMenuViewport } from "./menu-layout.js";

export type MenuMeasurementResult =
  | { readonly kind: "ready"; readonly measurements: readonly MenuTextMeasurement[] }
  | { readonly kind: "unavailable"; readonly reason: "invalid-text-metrics" | "measurement-unavailable" };

export interface MenuTextInk {
  readonly identity: string;
  readonly value: string;
  readonly anchorX: number;
  readonly baselineY: number;
  readonly rotationDegrees: number;
  readonly fullRect: MenuRect;
  readonly visible: { readonly kind: "outside" } | { readonly kind: "visible"; readonly rect: MenuRect };
}

export interface MenuDrawingContext extends HeadHudDrawingContext {
  fillGradientRect(left: number, top: number, width: number, height: number, colors: readonly [string, string]): void;
  setTextBaseline(value: "middle" | "alphabetic"): void;
  translate(coordinateX: number, coordinateY: number): void;
  rotate(radians: number): void;
}

export function browserMenuContext(context: CanvasRenderingContext2D): MenuDrawingContext {
  context.direction = "ltr";
  context.fontKerning = "normal";
  context.fontStretch = "normal";
  context.fontVariantCaps = "normal";
  context.letterSpacing = "0px";
  context.wordSpacing = "0px";
  return Object.freeze({ ...browserHeadHudContext(context),
    fillGradientRect: (left: number, top: number, width: number, height: number, colors: readonly [string, string]): void => {
      const gradient = context.createLinearGradient(left, top, left + width, top);
      gradient.addColorStop(0, colors[0]);
      gradient.addColorStop(1, colors[1]);
      context.fillStyle = gradient;
      context.fillRect(left, top, width, height);
    },
    setTextBaseline: (value: "middle" | "alphabetic"): void => { context.textBaseline = value; },
    translate: (coordinateX: number, coordinateY: number): void => { context.translate(coordinateX, coordinateY); },
    rotate: (radians: number): void => { context.rotate(radians); }
  });
}

export function menuCanvasSize(size: MenuLayoutRequest["surfaceSize"]): Readonly<{ width: number; height: number }> {
  if (![size.width, size.height].every((value) => Number.isFinite(value) && value > 0)) throw new RangeError("Menu surface dimensions must be positive and finite");
  const scale = 1536 / Math.max(size.width, size.height);
  return Object.freeze({ width: Math.max(1, Math.round(size.width * scale)), height: Math.max(1, Math.round(size.height * scale)) });
}

export function measureMenuLayout(context: MenuDrawingContext, request: MenuLayoutRequest, canvasWidth: number, canvasHeight: number): MenuMeasurementResult {
  if (![canvasWidth, canvasHeight].every((value) => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const metersPerPixel = request.surfaceSize.height / canvasHeight;
  const horizontalMetersPerPixel = request.surfaceSize.width / canvasWidth;
  context.save();
  try {
    let activeIndex = -1;
    const reply = JSON.parse(menu_measure_text_json(JSON.stringify(request.texts), (index, value) => {
      const text = request.texts[index];
      if (text === undefined) throw new Error("Shared Menu font query lost its request");
      if (activeIndex !== index) {
        configureText(context, text, metersPerPixel);
        activeIndex = index;
      }
      const metrics = context.measureText(value);
      return Float64Array.of(metrics.width * horizontalMetersPerPixel, metrics.left * horizontalMetersPerPixel,
        metrics.right * horizontalMetersPerPixel, metrics.ascent * metersPerPixel, metrics.descent * metersPerPixel);
    })) as MenuLayoutReply;
    if (reply.kind === "unavailable" && (reply.reason === "invalid-text-metrics" || reply.reason === "measurement-unavailable")) return { kind: "unavailable", reason: reply.reason };
    if (reply.kind !== "measured") throw new Error("Shared Menu measurement returned an unexpected phase");
    return { kind: "ready", measurements: reply.measurements };
  } catch {
    return Object.freeze({ kind: "unavailable", reason: "measurement-unavailable" });
  } finally {
    context.restore();
  }
}

function configureText(context: MenuDrawingContext, text: MenuTextRequest, metersPerPixel: number): void {
  context.setLocale(text.locale);
  context.setFont(`${text.font.style} ${String(text.font.weight)} ${String(text.fontMeters / metersPerPixel)}px ${text.font.family}`);
  context.setTextBaseline(text.chartText == null ? "middle" : "alphabetic");
  context.setTextAlign("left");
}

type PendingInk = Readonly<{ ink: Omit<MenuTextInk, "visible">; documentRect: MenuRect | null }>;

function paintedText(viewport: MenuViewport, text: MenuTextLayout, surfaceFixed: boolean): readonly PendingInk[] {
  const request = viewport.document.request;
  const left = text.bounds.x + (surfaceFixed ? 0 : viewport.contentClip.x);
  const top = text.bounds.y + (surfaceFixed ? 0 : viewport.contentClip.y - viewport.offsetMeters);
  return Object.freeze(text.lines.map((line, index) => {
    const projection = text.chartProjection;
    if (projection !== undefined) {
      const geometry = text.lineGeometry?.[index];
      if (geometry === undefined) throw new Error("Shared Menu chart line lost its ink geometry");
      const { anchor, bounds: documentRect } = geometry;
      return { ink: { identity: text.request.identity, value: line.value,
        anchorX: anchor.x + viewport.contentClip.x, baselineY: anchor.y + viewport.contentClip.y - viewport.offsetMeters,
        rotationDegrees: projection.rotationDegrees,
        fullRect: { ...documentRect, x: documentRect.x + viewport.contentClip.x, y: documentRect.y + viewport.contentClip.y - viewport.offsetMeters } }, documentRect };
    }
    const anchorX = left + Math.max(0, line.leftMeters);
    const baselineY = top + index * request.lineHeightMeters + request.lineHeightMeters / 2 + (line.ascentMeters - line.descentMeters) / 2;
    const fullRect = Object.freeze({ x: anchorX - line.leftMeters, y: baselineY - line.ascentMeters,
      width: line.leftMeters + line.rightMeters, height: line.ascentMeters + line.descentMeters });
    const documentRect = { ...fullRect, x: fullRect.x - viewport.contentClip.x,
      y: fullRect.y - viewport.contentClip.y + viewport.offsetMeters };
    return { ink: { identity: text.request.identity, value: line.value, anchorX, baselineY, rotationDegrees: 0, fullRect },
      documentRect: surfaceFixed ? null : documentRect };
  }));
}

function pageText(viewport: MenuViewport, direction: "previous" | "next"): MenuTextLayout {
  const document = viewport.document;
  const measurement = direction === "previous" ? document.previous : document.next;
  const request = document.request.texts.find((text) => text.identity === measurement.identity);
  if (request === undefined) throw new Error("Menu page measurement lost its request");
  const bounds = direction === "previous" ? viewport.previousBounds : viewport.nextBounds;
  return Object.freeze({ request, lines: measurement.lines, bounds: Object.freeze({ x: bounds.x + document.request.paddingMeters,
    y: bounds.y + document.request.paddingMeters, width: request.widthMeters, height: measurement.lines.length * document.request.lineHeightMeters }) });
}

export function menuPaintedTextInk(viewport: MenuViewport): readonly MenuTextInk[] {
  const pending = [viewport.document.title, viewport.document.caption, ...viewport.document.sectionHeadings, ...viewport.document.controls.flatMap((control) => control.texts)]
    .flatMap((text) => paintedText(viewport, text, false))
    .concat(viewport.maximumOffsetMeters > 0 ? [...paintedText(viewport, pageText(viewport, "previous"), true), ...paintedText(viewport, pageText(viewport, "next"), true)] : []);
  const rectangles = pending.flatMap(item => item.documentRect === null ? [] : [item.documentRect]);
  const projected = projectMenuViewport(viewport, rectangles);
  let index = 0;
  return pending.map(({ ink, documentRect }): MenuTextInk => {
    if (documentRect === null) return { ...ink, visible: { kind: "visible", rect: ink.fullRect } };
    const geometry = projected.rectangles[index++];
    if (geometry === undefined) throw new Error("Menu text lost its viewport projection");
    return { ...ink, fullRect: geometry.full, visible: geometry.visible === null
      ? { kind: "outside" } : { kind: "visible", rect: geometry.visible } };
  });
}

export function drawMeasuredMenu(context: MenuDrawingContext, viewport: MenuViewport, canvasWidth: number, canvasHeight: number): void {
  if (![canvasWidth, canvasHeight].every((value) => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const request = viewport.document.request;
  const theme = request.theme;
  const inkByIdentity = new Map<string, MenuTextInk[]>();
  for (const ink of menuPaintedTextInk(viewport)) {
    const lines = inkByIdentity.get(ink.identity) ?? [];
    lines.push(ink);
    inkByIdentity.set(ink.identity, lines);
  }
  const rectangles = viewport.document.controls.flatMap(layout => layout.kind === "chart" ? [layout.bounds, layout.plot] : [layout.bounds]);
  const projected = projectMenuViewport(viewport, rectangles);
  const visibleRects = new Map(rectangles.map((rectangle, index) => [rectangle, projected.rectangles[index]?.visible ?? null]));
  const scaleX = canvasWidth / request.surfaceSize.width;
  const scaleY = canvasHeight / request.surfaceSize.height;
  const drawRect = (rectangle: MenuRect): void => { context.fillRect(rectangle.x * scaleX, rectangle.y * scaleY, rectangle.width * scaleX, rectangle.height * scaleY); };
  const text = (layout: MenuTextLayout, color = theme.text): void => {
    configureText(context, layout.request, 1 / scaleY);
    context.setFillStyle(layout.request.chartText?.color ?? color);
    for (const ink of inkByIdentity.get(layout.request.identity) ?? []) {
      if (ink.visible.kind !== "visible") continue;
      if (layout.chartProjection === undefined) context.fillText(ink.value, ink.anchorX * scaleX, ink.baselineY * scaleY);
      else {
        context.save();
        try {
          context.translate(ink.anchorX * scaleX, ink.baselineY * scaleY);
          context.rotate(ink.rotationDegrees * Math.PI / 180);
          context.fillText(ink.value, 0, 0);
        } finally { context.restore(); }
      }
    }
  };
  context.clearRect(0, 0, canvasWidth, canvasHeight);
  context.save();
  try {
    context.setGlobalAlpha(theme.panel_opacity);
    context.setFillStyle(theme.panel_background);
    context.fillRect(0, 0, canvasWidth, canvasHeight);
    context.setGlobalAlpha(1);
    context.save();
    try {
      context.beginPath();
      context.rect(viewport.contentClip.x * scaleX, viewport.contentClip.y * scaleY,
        viewport.contentClip.width * scaleX, viewport.contentClip.height * scaleY);
      context.clip();
      text(viewport.document.title);
      text(viewport.document.caption, theme.muted_text);
      for (const heading of viewport.document.sectionHeadings) text(heading);
      for (const control of viewport.document.controls) {
        const clipped = visibleRects.get(control.bounds);
        if (clipped == null) continue;
        context.setFillStyle(uiControlBackground(control.control, theme));
        drawRect(clipped);
        if (control.kind === "chart") drawChart(context, control.control, control.plot, viewport, visibleRects.get(control.plot) ?? null, scaleX, scaleY);
        for (const layout of control.texts) {
          const legend = layout.request.legend;
          if (legend != null) {
            const left = (layout.bounds.x + viewport.contentClip.x - request.fontMeters * 2) * scaleX;
            const top = (layout.bounds.y + viewport.contentClip.y - viewport.offsetMeters + request.lineHeightMeters * 0.35) * scaleY;
            const width = request.fontMeters * 1.5 * scaleX;
            const height = request.lineHeightMeters * 0.3 * scaleY;
            if (legend.segment_gradient === null) {
              context.setFillStyle(legend.color);
              context.fillRect(left, top, width, height);
            } else context.fillGradientRect(left, top, width, height, legend.segment_gradient);
          }
          text(layout, control.control.enabled || control.control.kind === "status" || control.control.kind === "chart" ? theme.text : theme.muted_text);
        }
      }
    } finally { context.restore(); }
    if (viewport.maximumOffsetMeters === 0) return;
    for (const direction of ["previous", "next"] as const) {
      const enabled = direction === "previous" ? viewport.offsetMeters > 0 : viewport.offsetMeters < viewport.maximumOffsetMeters;
      const bounds = direction === "previous" ? viewport.previousBounds : viewport.nextBounds;
      context.setGlobalAlpha(1);
      context.setFillStyle(theme.normal_background);
      drawRect(bounds);
      text(pageText(viewport, direction), enabled ? theme.text : theme.muted_text);
    }
  } finally { context.restore(); }
}

function drawChart(context: HeadHudDrawingContext, chart: UiChart, plot: MenuRect, viewport: MenuViewport, clip: MenuRect | null, scaleX: number, scaleY: number): void {
  if (clip === null) return;
  const transform = (point: Readonly<{ x: number; y: number }>) => ({
    x: (plot.x + (point.x - chart.x_minimum) / (chart.x_maximum - chart.x_minimum) * plot.width + viewport.contentClip.x) * scaleX,
    y: (plot.y + (chart.y_maximum - point.y) / (chart.y_maximum - chart.y_minimum) * plot.height + viewport.contentClip.y - viewport.offsetMeters) * scaleY
  });
  const segment = (start: Readonly<{ x: number; y: number }>, end: Readonly<{ x: number; y: number }>, color: string, lineWidth: number): void => {
    const first = transform(start);
    const second = transform(end);
    context.setStrokeStyle(color);
    context.setLineWidth(lineWidth);
    context.beginPath();
    context.moveTo(first.x, first.y);
    context.lineTo(second.x, second.y);
    context.stroke();
  };
  context.save();
  try {
    context.beginPath();
    context.rect(clip.x * scaleX, clip.y * scaleY, clip.width * scaleX, clip.height * scaleY);
    context.clip();
    const [left, top, right, bottom] = chart.presentation.plot;
    const descriptorPoint = (point: Readonly<{ x: number; y: number }>) => ({
      x: chart.x_minimum + (point.x - left) / (right - left) * (chart.x_maximum - chart.x_minimum),
      y: chart.y_maximum - (point.y - top) / (bottom - top) * (chart.y_maximum - chart.y_minimum)
    });
    context.setGlobalAlpha(chart.presentation.grid_opacity);
    for (const [start, end] of chart.presentation.grid_lines) segment(descriptorPoint(start), descriptorPoint(end), chart.presentation.grid_color, 1);
    context.setGlobalAlpha(1);
    segment({ x: chart.x_minimum, y: chart.y_maximum }, { x: chart.x_minimum, y: chart.y_minimum }, chart.presentation.axes_color, 1.5);
    segment({ x: chart.x_minimum, y: chart.y_minimum }, { x: chart.x_maximum, y: chart.y_minimum }, chart.presentation.axes_color, 1.5);
    for (const reference of chart.reference_lines) segment({ x: chart.x_minimum, y: reference.value }, { x: chart.x_maximum, y: reference.value }, reference.color, 2);
    const guides = chart.presentation.map_guides;
    if (guides !== null) {
      for (const [start, end] of [...guides.scale_segments, ...guides.north_segments]) {
        segment(descriptorPoint(start), descriptorPoint(end), "#f3f4e8", 1.5);
      }
    }
    for (const series of chart.series) for (let index = 1; index < series.points.length; index++) {
      const first = series.points[index - 1];
      const second = series.points[index];
      if (first !== undefined && second !== undefined) segment(first, second, series.segment_colors[index - 1] ?? series.color, 3);
    }
    for (const vector of chart.vectors) if (vector.segment !== null) {
      const [start, end] = vector.segment;
      segment(start, end, vector.color, 2);
      const deltaX = end.x - start.x;
      const deltaY = end.y - start.y;
      segment(end, { x: end.x - deltaX * 0.25 - deltaY * 0.15, y: end.y - deltaY * 0.25 + deltaX * 0.15 }, vector.color, 2);
      segment(end, { x: end.x - deltaX * 0.25 + deltaY * 0.15, y: end.y - deltaY * 0.25 - deltaX * 0.15 }, vector.color, 2);
    }
    for (const marker of [...chart.markers, ...chart.time_markers]) {
      const point = transform(marker.point);
      context.setFillStyle(marker.color);
      context.fillRect(point.x - 3, point.y - 3, 6, 6);
    }
    if (chart.cursor_x !== null) segment({ x: chart.cursor_x, y: chart.y_minimum }, { x: chart.cursor_x, y: chart.y_maximum }, "#ffd45c", 2);
    for (const cursor of chart.cursor_points) {
      const point = transform(cursor.point);
      context.setFillStyle(cursor.color);
      context.fillRect(point.x - 4, point.y - 4, 8, 8);
    }
  } finally { context.restore(); }
}
