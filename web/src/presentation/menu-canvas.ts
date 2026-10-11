import type { MenuLayoutRequest, MenuMeasuredLine, MenuRect, MenuTextLayout, MenuTextMeasurement,
  MenuTextRequest, MenuViewport } from "../render/contracts/menu-layout.js";
import { uiControlBackground } from "../render/contracts/ui.js";
import type { UiChart } from "../render/contracts/ui.js";
import { browserHeadHudContext } from "./head-hud-canvas.js";
import type { HeadHudDrawingContext } from "./head-hud-canvas.js";
import { menuChartTextLineGeometry, menuRectInViewport } from "./menu-layout.js";

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
  readonly visible: ReturnType<typeof menuRectInViewport>;
}

export interface MenuDrawingContext extends HeadHudDrawingContext {
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
    const measurements: MenuTextMeasurement[] = [];
    for (const text of request.texts) {
      configureText(context, text, metersPerPixel);
      const lines: MenuMeasuredLine[] = [];
      const measure = (value: string, paragraphIndex: number): MenuMeasuredLine => {
        const metrics = context.measureText(value);
        return Object.freeze({ value, paragraphIndex, advanceMeters: metrics.width * horizontalMetersPerPixel,
          leftMeters: metrics.left * horizontalMetersPerPixel, rightMeters: metrics.right * horizontalMetersPerPixel,
          ascentMeters: metrics.ascent * metersPerPixel, descentMeters: metrics.descent * metersPerPixel });
      };
      const valid = (line: MenuMeasuredLine): boolean => [line.advanceMeters, line.leftMeters, line.rightMeters,
        line.ascentMeters, line.descentMeters].every(Number.isFinite) && line.advanceMeters >= 0 &&
        line.leftMeters + line.rightMeters >= 0 && line.ascentMeters + line.descentMeters >= 0;
      const fits = (line: MenuMeasuredLine): boolean => Math.max(line.advanceMeters, Math.max(0, line.leftMeters) + line.rightMeters) <= text.widthMeters + 1e-10;
      for (const [paragraphIndex, paragraph] of text.value.split("\n").entries()) {
        let current = measure("", paragraphIndex);
        if (!valid(current) || !fits(current)) return Object.freeze({ kind: "unavailable", reason: "invalid-text-metrics" });
        for (const character of Array.from(paragraph)) {
          const candidate = measure(current.value + character, paragraphIndex);
          if (!valid(candidate)) return Object.freeze({ kind: "unavailable", reason: "invalid-text-metrics" });
          if (fits(candidate)) current = candidate;
          else {
            if (current.value === "") return Object.freeze({ kind: "unavailable", reason: "invalid-text-metrics" });
            lines.push(current);
            current = measure(character, paragraphIndex);
            if (!valid(current) || !fits(current)) return Object.freeze({ kind: "unavailable", reason: "invalid-text-metrics" });
          }
        }
        lines.push(current);
      }
      measurements.push(Object.freeze({ identity: text.identity, lines: Object.freeze(lines) }));
    }
    return Object.freeze({ kind: "ready", measurements: Object.freeze(measurements) });
  } catch {
    return Object.freeze({ kind: "unavailable", reason: "measurement-unavailable" });
  } finally {
    context.restore();
  }
}

function configureText(context: MenuDrawingContext, text: MenuTextRequest, metersPerPixel: number): void {
  context.setLocale(text.locale);
  context.setFont(`${text.font.style} ${String(text.font.weight)} ${String(text.fontMeters / metersPerPixel)}px ${text.font.family}`);
  context.setTextBaseline(text.chartText === undefined ? "middle" : "alphabetic");
  context.setTextAlign("left");
}

function paintedText(viewport: MenuViewport, text: MenuTextLayout, surfaceFixed: boolean): readonly MenuTextInk[] {
  const request = viewport.document.request;
  const left = text.bounds.x + (surfaceFixed ? 0 : viewport.contentClip.x);
  const top = text.bounds.y + (surfaceFixed ? 0 : viewport.contentClip.y - viewport.offsetMeters);
  return Object.freeze(text.lines.map((line, index) => {
    const projection = text.chartProjection;
    if (projection !== undefined) {
      const { anchor, bounds: documentRect } = menuChartTextLineGeometry(projection, line, index, request.lineHeightMeters);
      return Object.freeze({ identity: text.request.identity, value: line.value,
        anchorX: anchor.x + viewport.contentClip.x, baselineY: anchor.y + viewport.contentClip.y - viewport.offsetMeters,
        rotationDegrees: projection.rotationDegrees,
        fullRect: Object.freeze({ ...documentRect, x: documentRect.x + viewport.contentClip.x, y: documentRect.y + viewport.contentClip.y - viewport.offsetMeters }),
        visible: menuRectInViewport(viewport, documentRect) });
    }
    const anchorX = left + Math.max(0, line.leftMeters);
    const baselineY = top + index * request.lineHeightMeters + request.lineHeightMeters / 2 + (line.ascentMeters - line.descentMeters) / 2;
    const fullRect = Object.freeze({ x: anchorX - line.leftMeters, y: baselineY - line.ascentMeters,
      width: line.leftMeters + line.rightMeters, height: line.ascentMeters + line.descentMeters });
    const documentRect = { ...fullRect, x: fullRect.x - viewport.contentClip.x,
      y: fullRect.y - viewport.contentClip.y + viewport.offsetMeters };
    return Object.freeze({ identity: text.request.identity, value: line.value, anchorX, baselineY, rotationDegrees: 0, fullRect,
      visible: surfaceFixed ? Object.freeze({ kind: "visible" as const, rect: fullRect }) : menuRectInViewport(viewport, documentRect) });
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
  return Object.freeze([viewport.document.title, viewport.document.caption, ...viewport.document.controls.flatMap((control) => control.texts)]
    .flatMap((text) => paintedText(viewport, text, false))
    .concat(viewport.maximumOffsetMeters > 0 ? [...paintedText(viewport, pageText(viewport, "previous"), true), ...paintedText(viewport, pageText(viewport, "next"), true)] : []));
}

export function drawMeasuredMenu(context: MenuDrawingContext, viewport: MenuViewport, canvasWidth: number, canvasHeight: number): void {
  if (![canvasWidth, canvasHeight].every((value) => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const request = viewport.document.request;
  const theme = request.theme;
  const scaleX = canvasWidth / request.surfaceSize.width;
  const scaleY = canvasHeight / request.surfaceSize.height;
  const drawRect = (rectangle: MenuRect): void => { context.fillRect(rectangle.x * scaleX, rectangle.y * scaleY, rectangle.width * scaleX, rectangle.height * scaleY); };
  const text = (layout: MenuTextLayout, fixed: boolean, color = theme.text): void => {
    configureText(context, layout.request, 1 / scaleY);
    context.setFillStyle(layout.request.chartText?.color ?? color);
    for (const ink of paintedText(viewport, layout, fixed)) {
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
      text(viewport.document.title, false);
      text(viewport.document.caption, false, theme.muted_text);
      for (const control of viewport.document.controls) {
        const clipped = menuRectInViewport(viewport, control.bounds);
        if (clipped.kind === "outside") continue;
        context.setFillStyle(uiControlBackground(control.control, theme));
        drawRect(clipped.rect);
        if (control.kind === "chart") drawChart(context, control.control, control.plot, viewport, scaleX, scaleY);
        for (const layout of control.texts) text(layout, false, control.control.enabled || control.control.kind === "status" || control.control.kind === "chart" ? theme.text : theme.muted_text);
      }
    } finally { context.restore(); }
    if (viewport.maximumOffsetMeters === 0) return;
    for (const direction of ["previous", "next"] as const) {
      const enabled = direction === "previous" ? viewport.offsetMeters > 0 : viewport.offsetMeters < viewport.maximumOffsetMeters;
      const bounds = direction === "previous" ? viewport.previousBounds : viewport.nextBounds;
      context.setGlobalAlpha(1);
      context.setFillStyle(theme.normal_background);
      drawRect(bounds);
      text(pageText(viewport, direction), true, enabled ? theme.text : theme.muted_text);
    }
  } finally { context.restore(); }
}

function drawChart(context: HeadHudDrawingContext, chart: UiChart, plot: MenuRect, viewport: MenuViewport, scaleX: number, scaleY: number): void {
  const clip = menuRectInViewport(viewport, plot);
  if (clip.kind === "outside") return;
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
    context.rect(clip.rect.x * scaleX, clip.rect.y * scaleY, clip.rect.width * scaleX, clip.rect.height * scaleY);
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
