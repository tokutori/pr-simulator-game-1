import { validateHeadHudLayer } from "../render/contracts/head-hud.js";
import type { HeadHudElement, VisibleHeadHud } from "../render/contracts/head-hud.js";
import { browserPanelContext } from "./browser-canvas.js";
import { HEAD_HUD_CARD_PADDING, HEAD_HUD_LINE_HEIGHT, headHudElementValue, headHudInstrumentHeight } from "./head-hud-view.js";
import type { HeadHudUnavailableReason, HeadHudView } from "./head-hud-view.js";
import type { PanelDrawingContext } from "./browser-canvas.js";
import { inversePose, transformPoint, vec3 } from "../render/contracts/math.js";
import { projectHeadPoint } from "../render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";

export const HEAD_HUD_MINIMUM_INK_HEIGHT_DEGREES = 0.35;

export interface HeadHudTextInk {
  readonly value: string;
  readonly x: number;
  readonly baseline: number;
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface HeadHudTextMetrics {
  readonly width: number;
  readonly left: number;
  readonly right: number;
  readonly ascent: number;
  readonly descent: number;
}

export interface HeadHudDrawingContext extends Omit<PanelDrawingContext, "fillText"> {
  setLocale(locale: string): void;
  fillText(text: string, x: number, y: number): void;
  measureText(text: string): HeadHudTextMetrics;
  setTextAlign(value: "left"): void;
}

export type HeadHudPaintPreparation = Readonly<{ width: number; height: number }> & (
  | { readonly kind: "absent" }
  | { readonly kind: "unavailable"; readonly reason: HeadHudUnavailableReason }
  | { readonly kind: "ready"; readonly view: Extract<HeadHudView, { kind: "visible" }>; readonly fontSize: number; readonly cards: readonly PreparedCard[] }
);

export function headHudCanvasSize(layer: VisibleHeadHud): Readonly<{ width: number; height: number }> {
  const scale = 2048 / Math.max(layer.size.width, layer.size.height);
  return Object.freeze({ width: Math.max(1, Math.round(layer.size.width * scale)), height: Math.max(1, Math.round(layer.size.height * scale)) });
}

export function browserHeadHudContext(context: CanvasRenderingContext2D): HeadHudDrawingContext {
  return Object.freeze({
    ...browserPanelContext(context),
    setLocale: (locale: string): void => {
      context.canvas.lang = locale;
      if ("lang" in context && typeof context.lang === "string") context.lang = locale;
    },
    setTextAlign: (value: "left"): void => { context.textAlign = value; },
    fillText: (value: string, x: number, y: number): void => { context.fillText(value, x, y); },
    measureText: (value: string): HeadHudTextMetrics => {
      const measured = context.measureText(value);
      return Object.freeze({ width: measured.width, left: measured.actualBoundingBoxLeft, right: measured.actualBoundingBoxRight,
        ascent: measured.actualBoundingBoxAscent, descent: measured.actualBoundingBoxDescent });
    }
  });
}

export interface PaintedLine {
  readonly value: string;
  readonly metrics: HeadHudTextMetrics;
}

interface PreparedCard {
  readonly element: HeadHudElement;
  readonly value: readonly PaintedLine[];
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly padding: number;
  readonly instrumentHeight: number;
  readonly instrumentText: readonly HeadHudTextInk[];
}

export function prepareHeadHudPaint(context: HeadHudDrawingContext, view: HeadHudView, width: number, height: number): HeadHudPaintPreparation {
  if (![width, height].every((value) => Number.isInteger(value) && value > 0)) throw new RangeError("Head HUD canvas dimensions must be positive integers");
  if (view.kind === "absent") return Object.freeze({ kind: "absent", width, height });
  if (view.kind === "unavailable") return Object.freeze({ kind: "unavailable", reason: view.reason, width, height });
  const layer = view.layer;
  validateHeadHudLayer(layer);
  const fontSize = view.textHeightMeters * height / layer.size.height;
  const lineHeight = fontSize * HEAD_HUD_LINE_HEIGHT;
  context.save();
  configureText(context, fontSize, layer.locale);
  const cards: PreparedCard[] = [];
  for (const element of layer.elements) {
    const cardWidth = element.bounds.width * width;
    const cardHeight = element.bounds.height * height;
    const padding = element.content.kind === "text" ? 0 : fontSize * HEAD_HUD_CARD_PADDING;
    const value = prepareHudTextLines(context, element.label !== "" ? `${element.label} ${headHudElementValue(element)}` : headHudElementValue(element), cardWidth - padding * 2, lineHeight);
    const instrumentHeight = headHudInstrumentHeight(element, fontSize);
    if (value === null || value.length * lineHeight + instrumentHeight + padding * 2 > cardHeight + 1e-6) {
      context.restore();
      return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
    }
    const left = element.bounds.left * width;
    const top = element.bounds.top * height;
    const instrumentText: HeadHudTextInk[] = [];
    const content = element.content;
    if (content.kind === "attitude" && content.geometry.kind === "attitude") {
      const horizon = content.geometry.horizon;
      const roll = horizon.rotation_degrees * Math.PI / 180;
      const instrumentTop = top + padding + value.length * lineHeight;
      const instrumentWidth = cardWidth - 2 * padding;
      for (const tick of horizon.pitch_ticks) {
        if (tick.label_degrees === null) continue;
        const metrics = measuredLine(context, String(tick.label_degrees), lineHeight);
        if (metrics === null) {
          context.restore();
          return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
        }
        const across = tick.half_width_ratio * instrumentWidth + fontSize * 0.7;
        const down = (horizon.pitch_offset_ratio + tick.offset_ratio) * instrumentHeight;
        const centerX = left + cardWidth / 2 + Math.cos(roll) * across - Math.sin(roll) * down;
        const centerY = instrumentTop + instrumentHeight / 2 + Math.sin(roll) * across + Math.cos(roll) * down;
        const x = centerX - metrics.width / 2;
        const baseline = centerY + (metrics.ascent - metrics.descent) / 2;
        const ink = { value: String(tick.label_degrees), x, baseline,
          left: x - metrics.left, top: baseline - metrics.ascent, width: metrics.left + metrics.right, height: metrics.ascent + metrics.descent };
        if (ink.left >= left + padding && ink.left + ink.width <= left + cardWidth - padding
            && ink.top >= instrumentTop && ink.top + ink.height <= instrumentTop + instrumentHeight) instrumentText.push(Object.freeze(ink));
      }
    }
    if (content.kind === "heading_tape" || content.kind === "linear_gauge") {
      const textTop = top + padding + value.length * lineHeight;
      const labels = content.kind === "heading_tape"
        ? content.ticks.filter((tick) => tick.label !== "").map((tick) => ({ value: tick.label, ratio: tick.position_ratio }))
        : [{ value: content.minimum_label, ratio: 0 }, { value: content.maximum_label, ratio: 1 }];
      for (const tick of labels) {
        const metrics = measuredLine(context, tick.value, lineHeight);
        if (metrics === null) {
          context.restore();
          return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
        }
        const x = Math.max(left + padding + Math.max(0, metrics.left), Math.min(left + cardWidth - padding - metrics.right,
          left + padding + tick.ratio * (cardWidth - padding * 2) - metrics.width / 2));
        const baseline = textTop + lineHeight / 2 + (metrics.ascent - metrics.descent) / 2;
        const ink = Object.freeze({ value: tick.value, x, baseline, left: x - metrics.left, top: baseline - metrics.ascent,
          width: metrics.left + metrics.right, height: metrics.ascent + metrics.descent });
        if (ink.left < left + padding || ink.left + ink.width > left + cardWidth - padding ||
            ink.top < textTop || ink.top + ink.height > textTop + instrumentHeight) {
          context.restore();
          return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
        }
        instrumentText.push(ink);
      }
    }
    cards.push(Object.freeze({ element, value: Object.freeze(value), left, top,
      width: cardWidth, height: cardHeight, padding, instrumentHeight, instrumentText: Object.freeze(instrumentText) }));
  }
  context.restore();
  return Object.freeze({ kind: "ready", view, width, height, fontSize, cards: Object.freeze(cards) });
}

export function drawHeadHud(context: HeadHudDrawingContext, preparation: HeadHudPaintPreparation): void {
  context.clearRect(0, 0, preparation.width, preparation.height);
  if (preparation.kind !== "ready") return;
  const layer = preparation.view.layer;
  const fontSize = preparation.fontSize;
  const lineHeight = fontSize * HEAD_HUD_LINE_HEIGHT;
  context.save();
  configureText(context, fontSize, layer.locale);
  for (const card of preparation.cards) {
    context.save();
    context.beginPath();
    context.rect(card.left, card.top, card.width, card.height);
    context.clip();
    context.setGlobalAlpha(layer.backgroundAlpha);
    context.setFillStyle(layer.theme.panel_background);
    context.fillRect(card.left, card.top, card.width, card.height);
    context.setGlobalAlpha(layer.foregroundAlpha);
    context.setFillStyle(card.element.content.kind === "text" && card.element.content.tone === "warning" ? layer.theme.focus_outline
      : card.element.content.kind === "text" && card.element.content.tone === "muted" ? layer.theme.muted_text : layer.theme.text);
    for (const text of paintedCardText(card, lineHeight)) context.fillText(text.value, text.x, text.baseline);
    const textTop = card.top + card.padding + card.value.length * lineHeight;
    drawInstrument(context, card.element, card.left + card.padding, textTop,
      card.width - card.padding * 2, card.instrumentHeight, fontSize, layer.theme);
    context.restore();
  }
  context.restore();
}

export function headHudPaintedTextInk(preparation: HeadHudPaintPreparation): readonly HeadHudTextInk[] {
  return preparation.kind !== "ready" ? Object.freeze([]) : Object.freeze(preparation.cards.flatMap((card) =>
    paintedCardText(card, preparation.fontSize * HEAD_HUD_LINE_HEIGHT)));
}

function paintedCardText(card: PreparedCard, lineHeight: number): readonly HeadHudTextInk[] {
  return [...card.value.map((text, index) => {
    const x = card.left + card.padding + Math.max(0, text.metrics.left);
    const baseline = card.top + card.padding + index * lineHeight + lineHeight / 2 + (text.metrics.ascent - text.metrics.descent) / 2;
    return Object.freeze({ value: text.value, x, baseline, left: x - text.metrics.left, top: baseline - text.metrics.ascent,
      width: text.metrics.left + text.metrics.right, height: text.metrics.ascent + text.metrics.descent });
  }), ...card.instrumentText];
}

export function validateHeadHudPaint(preparation: HeadHudPaintPreparation, viewer: ViewerFrame): HeadHudPaintPreparation {
  if (preparation.kind !== "ready") return preparation;
  if (viewer.source === "unavailable") return Object.freeze({ kind: "unavailable", reason: viewer.reason, width: preparation.width, height: preparation.height });
  const layer = preparation.view.layer;
  const toHeadPoint = (horizontal: number, vertical: number) => transformPoint(layer.localPose, vec3(
    (horizontal / preparation.width - 0.5) * layer.size.width,
    (0.5 - vertical / preparation.height) * layer.size.height, 0));
  for (const glyph of headHudPaintedTextInk(preparation)) {
    if (glyph.value.trim() === "") continue;
    for (const eye of viewer.eyes) for (const horizontal of [glyph.left, glyph.left + glyph.width]) {
      const top = toHeadPoint(horizontal, glyph.top);
      const bottom = toHeadPoint(horizontal, glyph.top + glyph.height);
      const topProjection = projectHeadPoint(eye, top);
      const bottomProjection = projectHeadPoint(eye, bottom);
      const eyeFromHead = inversePose(eye.headFromEye);
      const eyeTop = transformPoint(eyeFromHead, top);
      const eyeBottom = transformPoint(eyeFromHead, bottom);
      const dot = eyeTop.x * eyeBottom.x + eyeTop.y * eyeBottom.y + eyeTop.z * eyeBottom.z;
      const angle = Math.acos(Math.max(-1, Math.min(1, dot / (Math.hypot(eyeTop.x, eyeTop.y, eyeTop.z) * Math.hypot(eyeBottom.x, eyeBottom.y, eyeBottom.z))))) * 180 / Math.PI;
      if (topProjection === null || bottomProjection === null || [topProjection, bottomProjection].some((point) =>
        Math.abs(point.x) > 1 || Math.abs(point.y) > 1 || point.z < -1 || point.z > 1) ||
        !Number.isFinite(angle) || angle < HEAD_HUD_MINIMUM_INK_HEIGHT_DEGREES) {
        return Object.freeze({ kind: "unavailable", reason: "text-overflow", width: preparation.width, height: preparation.height });
      }
    }
  }
  return preparation;
}

export function prepareHudTextLines(context: HeadHudDrawingContext, text: string, width: number, lineHeight: number): readonly PaintedLine[] | null {
  const lines: PaintedLine[] = [];
  for (const paragraph of text.split("\n")) {
    let current = "";
    let currentMetrics = measuredLine(context, "", lineHeight);
    if (currentMetrics === null) return null;
    for (const character of Array.from(paragraph)) {
      const candidate = current + character;
      const metrics = measuredLine(context, candidate, lineHeight);
      if (metrics === null) return null;
      if (Math.max(metrics.width, Math.max(0, metrics.left) + metrics.right) > width) {
        if (current === "") return null;
        lines.push(Object.freeze({ value: current, metrics: currentMetrics }));
        current = character;
        const single = measuredLine(context, current, lineHeight);
        if (single === null || Math.max(single.width, Math.max(0, single.left) + single.right) > width) return null;
        currentMetrics = single;
      } else {
        current = candidate;
        currentMetrics = metrics;
      }
    }
    lines.push(Object.freeze({ value: current, metrics: currentMetrics }));
  }
  return lines;
}

function measuredLine(context: HeadHudDrawingContext, value: string, lineHeight: number): HeadHudTextMetrics | null {
  const metrics = context.measureText(value);
  return [metrics.width, metrics.left, metrics.right, metrics.ascent, metrics.descent].every(Number.isFinite) &&
    metrics.width >= 0 && metrics.left + metrics.right >= 0 && metrics.ascent + metrics.descent >= 0 && metrics.ascent + metrics.descent <= lineHeight
    ? Object.freeze({ ...metrics }) : null;
}

function configureText(context: HeadHudDrawingContext, fontSize: number, locale: string): void {
  context.setLocale(locale);
  context.setFont(`600 ${String(fontSize)}px ui-monospace, monospace`);
  context.setTextBaseline("middle");
  context.setTextAlign("left");
}

function drawInstrument(context: HeadHudDrawingContext, element: HeadHudElement, left: number, top: number, width: number, height: number, fontSize: number,
  theme: VisibleHeadHud["theme"]): void {
  const centerX = left + width / 2;
  const centerY = top + height / 2;
  const content = element.content;
  switch (content.kind) {
    case "text": return;
    case "attitude": {
      if (content.geometry.kind === "flight_path_only") {
        drawFlightPathMarker(context, centerX, top + content.geometry.y_ratio * height, fontSize, "#7df4c5");
        return;
      }
      const roll = content.geometry.horizon.rotation_degrees * Math.PI / 180;
      const pitchOffset = content.geometry.horizon.pitch_offset_ratio * height;
      const length = Math.min(width * 0.43, height * 0.5);
      const directionX = Math.cos(roll);
      const directionY = Math.sin(roll);
      const perpendicularX = -directionY;
      const perpendicularY = directionX;
      for (const tick of content.geometry.horizon.pitch_ticks) {
        const tickOffset = pitchOffset + tick.offset_ratio * height;
        const horizonX = centerX + perpendicularX * tickOffset;
        const horizonY = centerY + perpendicularY * tickOffset;
        const halfWidth = tick.half_width_ratio * width;
        line(context, horizonX - directionX * halfWidth, horizonY - directionY * halfWidth,
          horizonX + directionX * halfWidth, horizonY + directionY * halfWidth, theme.text, fontSize);
      }
      line(context, centerX - length * 0.45, centerY, centerX + length * 0.45, centerY, theme.focus_outline, fontSize);
      line(context, centerX, centerY - fontSize * 0.25, centerX, centerY + fontSize * 0.25, theme.focus_outline, fontSize);
      if (content.geometry.flight_path_y_ratio !== null) drawFlightPathMarker(context, centerX,
        top + content.geometry.flight_path_y_ratio * height, fontSize, "#7df4c5");
      return;
    }
    case "heading_tape":
      for (const tick of content.ticks) {
        const tickX = left + tick.position_ratio * width;
        line(context, tickX, top + height * (tick.major ? 0.5 : 0.75), tickX, top + height, tick.major ? theme.text : theme.muted_text, fontSize);
      }
      line(context, centerX - fontSize * 0.2, top + height * 0.9, centerX + fontSize * 0.2, top + height * 0.9, theme.focus_outline, fontSize);
      return;
    case "direction": {
      if (content.rotation_degrees === null) return;
      const radians = content.rotation_degrees * Math.PI / 180;
      const radius = Math.min(width * 0.3, height * 0.4);
      const endX = centerX + Math.sin(radians) * radius;
      const endY = centerY - Math.cos(radians) * radius;
      line(context, centerX, centerY, endX, endY, theme.focus_outline, fontSize);
      line(context, endX, endY, endX - Math.sin(radians - 0.5) * radius * 0.35, endY + Math.cos(radians - 0.5) * radius * 0.35, theme.focus_outline, fontSize);
      line(context, endX, endY, endX - Math.sin(radians + 0.5) * radius * 0.35, endY + Math.cos(radians + 0.5) * radius * 0.35, theme.focus_outline, fontSize);
      return;
    }
    case "linear_gauge": {
      const innerWidth = width * 0.9;
      const innerLeft = left + width * 0.05;
      line(context, innerLeft, centerY, innerLeft + innerWidth, centerY, theme.text, fontSize);
      line(context, centerX, top + height * 0.3, centerX, top + height * 0.7, theme.muted_text, fontSize);
      if (content.position_ratio !== null) line(context, innerLeft + content.position_ratio * innerWidth, top + height * 0.35,
        innerLeft + content.position_ratio * innerWidth, top + height * 0.85, theme.focus_outline, fontSize);
      return;
    }
  }
}

function drawFlightPathMarker(context: HeadHudDrawingContext, centerX: number, markerY: number, fontSize: number, color: string): void {
  const radius = fontSize * 0.35;
  line(context, centerX - radius, markerY, centerX, markerY - radius, color, fontSize);
  line(context, centerX, markerY - radius, centerX + radius, markerY, color, fontSize);
}

function line(context: HeadHudDrawingContext, startX: number, startY: number, endX: number, endY: number, color: string, fontSize: number): void {
  context.beginPath();
  context.setStrokeStyle(color);
  context.setLineWidth(Math.max(1, fontSize * 0.1));
  context.moveTo(startX, startY);
  context.lineTo(endX, endY);
  context.stroke();
}
