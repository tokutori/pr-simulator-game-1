import { inversePose, transformPoint, vec3 } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import { MENU_MINIMUM_INK_HEIGHT_DEGREES } from "../render/contracts/menu-layout.js";
import type { MenuClippedRect, MenuContentPoint, MenuControlLayout, MenuControlTextRole, MenuDocument, MenuDocumentResult,
  MenuFixedTextRole, MenuFontIdentity, MenuHit, MenuLayoutRequest,
  MenuLayoutRow, MenuMeasuredLine, MenuOpeningReadability, MenuPoint, MenuRect, MenuTextLayout, MenuTextMeasurement, MenuTextRequest,
  MenuRangeResult, MenuTextSlot, MenuViewport } from "../render/contracts/menu-layout.js";
import { chartScaleBarDistance, formatChartTick } from "../render/contracts/ui.js";
import type { PanelSize, UiControl, UiPanel } from "../render/contracts/ui.js";
import { projectHeadPoint } from "../render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";

export function requestMenuLayout(panel: UiPanel, surfaceSize: PanelSize, style: Readonly<{
  fontMeters: number; font: MenuFontIdentity; locale: string; previousLabel: string; nextLabel: string; caption: string;
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
  const rows = menuRows(panel.controls, surfaceSize.width < surfaceSize.height, contentWidth, style.fontMeters);
  const font = Object.freeze({ ...style.font });
  const texts: MenuTextRequest[] = [];
  const addText = (slot: MenuTextSlot, value: string, widthMeters: number): void => {
    const identity = JSON.stringify([panel.id, surfaceSize.width, surfaceSize.height, style.locale, style.fontMeters,
      font.family, font.weight, font.style, font.generation, slot, value, widthMeters]);
    texts.push(Object.freeze({ identity, slot: Object.freeze(slot), value, widthMeters, fontMeters: style.fontMeters, font, locale: style.locale }));
  };
  addText({ kind: "fixed", role: "title" }, panel.title, contentWidth);
  addText({ kind: "fixed", role: "caption" }, style.caption, contentWidth);
  const pageWidth = (contentWidth - gapMeters) / 2 - paddingMeters * 2;
  if (pageWidth <= 0) throw new RangeError("Menu surface has no space for page controls");
  addText({ kind: "fixed", role: "previous" }, style.previousLabel, pageWidth);
  addText({ kind: "fixed", role: "next" }, style.nextLabel, pageWidth);
  for (const row of rows) for (const item of row.controls) {
    const control = item.control;
    const width = item.width - paddingMeters * 2;
    const addControlText = (role: MenuControlTextRole, index: number, value: string, textWidth = width): void => {
      addText({ kind: "control", controlId: control.id, role, index }, value, textWidth);
    };
    addControlText("label", 0, control.label);
    switch (control.kind) {
      case "button": break;
      case "toggle": addControlText("value", 0, control.value ? "On" : "Off"); break;
      case "range": addControlText("value", 0, String(control.value)); break;
      case "status": addControlText("value", 0, control.value); break;
      case "chart": {
        addControlText("x-axis", 0, control.xAxisLabel);
        addControlText("y-axis", 0, control.yAxisLabel);
        if (control.equalAxisScale) {
          addControlText("north", 0, "N ↑");
          addControlText("scale", 0, `${formatChartTick(chartScaleBarDistance(control.xMaximum - control.xMinimum))} m`);
        }
        for (let tickIndex = 0; tickIndex <= 4; tickIndex++) {
          addControlText("x-tick", tickIndex, formatChartTick(control.xMinimum + (control.xMaximum - control.xMinimum) * tickIndex / 4), width / 5);
          addControlText("y-tick", tickIndex, formatChartTick(control.yMinimum + (control.yMaximum - control.yMinimum) * tickIndex / 4), width / 5);
        }
        control.series.forEach((series, index) => { addControlText("legend", index, series.label); });
        [...control.vectors, ...control.markers, ...control.timeMarkers, ...control.referenceLines]
          .forEach((annotation, index) => { addControlText("annotation", index, annotation.label); });
        break;
      }
    }
  }
  return Object.freeze({ panel, surfaceSize: Object.freeze({ ...surfaceSize }), fontMeters: style.fontMeters, font,
    lineHeightMeters: style.fontMeters * 1.35, paddingMeters, gapMeters, rows, texts: Object.freeze(texts) });
}

function menuRows(controls: readonly UiControl[], portrait: boolean, width: number, fontMeters: number): readonly MenuLayoutRow[] {
  const ordered = controls.map((control, index) => ({ control, index }))
    .sort((left, right) => left.control.rect.y - right.control.rect.y || left.control.rect.x - right.control.rect.x || left.index - right.index);
  const groups: UiControl[][] = [];
  for (const item of ordered) {
    const previous = groups[groups.length - 1];
    const first = previous?.[0];
    if (!portrait && previous !== undefined && first !== undefined && item.control.kind !== "chart" &&
        previous.every((control) => control.kind !== "chart") && Math.abs(item.control.rect.y - first.rect.y) < 1e-8 &&
        previous.every((control) => item.control.rect.x >= control.rect.x + control.rect.width)) previous.push(item.control);
    else groups.push([item.control]);
  }
  return Object.freeze(groups.flatMap((group) => {
    const left = Math.min(...group.map((control) => control.rect.x));
    const right = Math.max(...group.map((control) => control.rect.x + control.rect.width));
    const items = group.map((control) => Object.freeze({ control,
      left: (control.rect.x - left) / (right - left) * width, width: control.rect.width / (right - left) * width }));
    return items.some((item) => item.width < fontMeters * 8)
      ? group.map((control) => Object.freeze({ controls: Object.freeze([Object.freeze({ control, left: 0, width })]) }))
      : [Object.freeze({ controls: Object.freeze(items) })];
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
  for (const row of request.rows) {
    const layouts = row.controls.map((item): MenuControlLayout => {
      const specifications = request.texts.filter((text) => text.slot.kind === "control" && text.slot.controlId === item.control.id);
      const texts: MenuTextLayout[] = [];
      let textTop = top + request.paddingMeters;
      const placeText = (specification: MenuTextRequest): void => {
        const text = textLayout(specification, item.left + request.paddingMeters, textTop);
        texts.push(text);
        textTop += text.bounds.height;
      };
      specifications.filter((text) => text.slot.role === "label" || text.slot.role === "value" || text.slot.role === "y-axis" || text.slot.role === "north" || text.slot.role === "scale").forEach(placeText);
      if (item.control.kind !== "chart") return Object.freeze({ kind: "control", control: item.control,
        bounds: Object.freeze({ x: item.left, y: top, width: item.width, height: textTop - top + request.paddingMeters }), texts: Object.freeze(texts) });
      const ticks = specifications.filter((text) => text.slot.role === "x-tick" || text.slot.role === "y-tick")
        .map((text) => textLayout(text, 0, 0));
      const tickHeight = Math.max(request.lineHeightMeters, ...ticks.map((text) => text.bounds.height));
      const tickWidth = (item.width - request.paddingMeters * 2) / 5;
      const plotWidth = item.width - request.paddingMeters * 2 - tickWidth * 1.5 - request.gapMeters;
      const plotHeight = item.control.equalAxisScale ? plotWidth : Math.max(request.lineHeightMeters * 5, plotWidth * 0.55);
      const plot = Object.freeze({ x: item.left + request.paddingMeters + tickWidth + request.gapMeters,
        y: textTop + tickHeight / 2, width: plotWidth, height: plotHeight });
      for (const tick of ticks) {
        if (tick.request.slot.kind !== "control") throw new Error("Chart tick must belong to a control");
        const horizontal = tick.request.slot.role === "x-tick";
        const position = tick.request.slot.index / 4;
        const left = horizontal ? plot.x + position * plot.width - tickWidth / 2 : item.left + request.paddingMeters;
        const tickTop = horizontal ? plot.y + plot.height + request.gapMeters : plot.y + (1 - position) * plot.height - tick.bounds.height / 2;
        texts.push(Object.freeze({ ...tick, bounds: Object.freeze({ ...tick.bounds, x: left, y: tickTop }) }));
      }
      textTop = plot.y + plot.height + request.gapMeters + tickHeight;
      specifications.filter((text) => text.slot.role === "x-axis" || text.slot.role === "legend" || text.slot.role === "annotation").forEach(placeText);
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
  return Object.freeze({ kind: "ready", document: Object.freeze({ request, title, caption, controls: Object.freeze(controls),
    contentHeightMeters, previous, next, footerHeightMeters }) });
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

export function menuRangeAction(viewport: MenuViewport, point: MenuPoint): MenuRangeResult {
  const hit = hitMenuViewport(viewport, point);
  if (hit.kind !== "control" || hit.layout.control.kind !== "range") return Object.freeze({ kind: "absent" });
  const control = hit.layout.control;
  const ratio = Math.min(1, Math.max(0, (hit.documentPoint.x - hit.layout.bounds.x) / hit.layout.bounds.width));
  const raw = control.minimum + ratio * (control.maximum - control.minimum);
  const value = Math.min(control.maximum, Math.max(control.minimum, control.minimum + Math.round((raw - control.minimum) / control.step) * control.step));
  return Object.freeze({ kind: "action", controlId: control.id, value });
}

function contains(rectangle: MenuRect, point: MenuPoint): boolean {
  return point.x >= rectangle.x && point.x <= rectangle.x + rectangle.width && point.y >= rectangle.y && point.y <= rectangle.y + rectangle.height;
}

export function validateMenuOpeningInk(document: MenuDocument, viewer: ViewerFrame, headFromSurface: Pose): MenuOpeningReadability {
  if (viewer.source === "unavailable") return Object.freeze({ kind: "unavailable", reason: viewer.reason });
  const size = document.request.surfaceSize;
  const lines: readonly MenuMeasuredLine[] = [...document.title.lines, ...document.caption.lines,
    ...document.controls.flatMap((control) => control.texts.flatMap((text) => text.lines)), ...document.previous.lines, ...document.next.lines];
  try {
    let minimumInkAngleDegrees = Number.POSITIVE_INFINITY;
    for (const line of lines.filter((value) => value.value.trim() !== "")) {
      const inkHeight = line.ascentMeters + line.descentMeters;
      if (inkHeight <= 0 || inkHeight > size.height) return Object.freeze({ kind: "unavailable", reason: "insufficient-ink-angle" });
      for (const eye of viewer.eyes) for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
        const edge = vertical * size.height / 2;
        const first = transformPoint(headFromSurface, vec3(horizontal * size.width / 2, edge, 0));
        const second = transformPoint(headFromSurface, vec3(horizontal * size.width / 2, edge - vertical * inkHeight, 0));
        if ([first, second].some((point) => {
          const projected = projectHeadPoint(eye, point);
          return projected === null || Math.abs(projected.x) > 1 || Math.abs(projected.y) > 1 || projected.z < -1 || projected.z > 1;
        })) return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
        const eyeFromHead = inversePose(eye.headFromEye);
        const eyeFirst = transformPoint(eyeFromHead, first);
        const eyeSecond = transformPoint(eyeFromHead, second);
        const firstLength = Math.hypot(eyeFirst.x, eyeFirst.y, eyeFirst.z);
        const secondLength = Math.hypot(eyeSecond.x, eyeSecond.y, eyeSecond.z);
        const firstDirection = { x: eyeFirst.x / firstLength, y: eyeFirst.y / firstLength, z: eyeFirst.z / firstLength };
        const secondDirection = { x: eyeSecond.x / secondLength, y: eyeSecond.y / secondLength, z: eyeSecond.z / secondLength };
        const cross = Math.hypot(firstDirection.y * secondDirection.z - firstDirection.z * secondDirection.y,
          firstDirection.z * secondDirection.x - firstDirection.x * secondDirection.z,
          firstDirection.x * secondDirection.y - firstDirection.y * secondDirection.x);
        const dot = firstDirection.x * secondDirection.x + firstDirection.y * secondDirection.y + firstDirection.z * secondDirection.z;
        const angle = Math.atan2(cross, dot) * 180 / Math.PI;
        if (!Number.isFinite(angle)) return Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" });
        minimumInkAngleDegrees = Math.min(minimumInkAngleDegrees, angle);
      }
    }
    return minimumInkAngleDegrees < MENU_MINIMUM_INK_HEIGHT_DEGREES || !Number.isFinite(minimumInkAngleDegrees)
      ? Object.freeze({ kind: "unavailable", reason: "insufficient-ink-angle" })
      : Object.freeze({ kind: "ready", minimumInkAngleDegrees });
  } catch (error) {
    if (error instanceof RangeError) return Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" });
    throw error;
  }
}
