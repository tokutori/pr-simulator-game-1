import { validateHeadHudLayer } from "../render/contracts/head-hud.js";
import type { HeadHudElement, VisibleHeadHud } from "../render/contracts/head-hud.js";
import { browserPanelContext } from "./browser-canvas.js";
import { HEAD_HUD_CARD_PADDING, HEAD_HUD_LINE_HEIGHT, headHudElementValue, headHudInstrumentHeight } from "./head-hud-view.js";
import type { HeadHudUnavailableReason, HeadHudView } from "./head-hud-view.js";
import type { PanelDrawingContext } from "./vr-panel-canvas.js";
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
  readonly label: readonly PaintedLine[];
  readonly value: readonly PaintedLine[];
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly padding: number;
  readonly instrumentHeight: number;
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
    const padding = fontSize * HEAD_HUD_CARD_PADDING;
    const label = element.label === "" ? [] : prepareHudTextLines(context, element.label, cardWidth - padding * 2, lineHeight);
    const value = prepareHudTextLines(context, headHudElementValue(element), cardWidth - padding * 2, lineHeight);
    const instrumentHeight = headHudInstrumentHeight(element, fontSize);
    if (label === null || value === null || (label.length + value.length) * lineHeight + instrumentHeight + padding * 2 > cardHeight + 1e-6) {
      context.restore();
      return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
    }
    cards.push(Object.freeze({ element, label: Object.freeze(label), value: Object.freeze(value), left: element.bounds.left * width, top: element.bounds.top * height,
      width: cardWidth, height: cardHeight, padding, instrumentHeight }));
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
    context.setFillStyle("#07151d");
    context.fillRect(card.left, card.top, card.width, card.height);
    context.setGlobalAlpha(layer.foregroundAlpha);
    context.setFillStyle(card.element.kind === "text" && card.element.tone === "warning" ? "#ffca87" : "#f3fff9");
    for (const text of paintedCardText(card, lineHeight)) context.fillText(text.value, text.x, text.baseline);
    const textTop = card.top + card.padding + (card.label.length + card.value.length) * lineHeight;
    drawInstrument(context, card.element, card.left + card.padding, textTop,
      card.width - card.padding * 2, card.instrumentHeight, fontSize);
    context.restore();
  }
  context.restore();
}

export function headHudPaintedTextInk(preparation: HeadHudPaintPreparation): readonly HeadHudTextInk[] {
  return preparation.kind !== "ready" ? Object.freeze([]) : Object.freeze(preparation.cards.flatMap((card) =>
    paintedCardText(card, preparation.fontSize * HEAD_HUD_LINE_HEIGHT)));
}

function paintedCardText(card: PreparedCard, lineHeight: number): readonly HeadHudTextInk[] {
  return [...card.label, ...card.value].map((text, index) => {
    const x = card.left + card.padding + Math.max(0, text.metrics.left);
    const baseline = card.top + card.padding + index * lineHeight + lineHeight / 2 + (text.metrics.ascent - text.metrics.descent) / 2;
    return Object.freeze({ value: text.value, x, baseline, left: x - text.metrics.left, top: baseline - text.metrics.ascent,
      width: text.metrics.left + text.metrics.right, height: text.metrics.ascent + text.metrics.descent });
  });
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

function drawInstrument(context: HeadHudDrawingContext, element: HeadHudElement, left: number, top: number, width: number, height: number, fontSize: number): void {
  const centerX = left + width / 2;
  const centerY = top + height / 2;
  switch (element.kind) {
    case "text": return;
    case "attitude": {
      const roll = -element.rollDegrees * Math.PI / 180;
      const pitchOffset = Math.max(-30, Math.min(30, element.pitchDegrees)) * height / 90;
      const length = Math.min(width * 0.43, height * 0.5);
      const directionX = Math.cos(roll);
      const directionY = Math.sin(roll);
      const perpendicularX = -directionY;
      const perpendicularY = directionX;
      for (const mark of [-20, -10, 0, 10, 20]) {
        const offset = pitchOffset - mark * height / 90;
        const markX = centerX + perpendicularX * offset;
        const markY = centerY + perpendicularY * offset;
        const extent = mark === 0 ? length : length * 0.35;
        line(context, markX - directionX * extent, markY - directionY * extent,
          markX + directionX * extent, markY + directionY * extent, mark === 0 ? "#dffaf3" : "#9dc9cc", fontSize);
      }
      line(context, centerX - length * 0.45, centerY, centerX + length * 0.45, centerY, "#ffde71", fontSize);
      line(context, centerX, centerY - fontSize * 0.25, centerX, centerY + fontSize * 0.25, "#ffde71", fontSize);
      if (element.flightPathAngleDegrees !== null) {
        const markerY = centerY - Math.max(-30, Math.min(30, element.flightPathAngleDegrees)) * height / 90;
        const radius = fontSize * 0.35;
        line(context, centerX - radius, markerY, centerX, markerY - radius, "#7df4c5", fontSize);
        line(context, centerX, markerY - radius, centerX + radius, markerY, "#7df4c5", fontSize);
      }
      return;
    }
    case "heading":
      for (const offset of [-30, -15, 0, 15, 30]) {
        const tickX = centerX + offset * width / 80;
        line(context, tickX, top + height * 0.25, tickX, top + height * (offset === 0 ? 0.9 : 0.65), "#dffaf3", fontSize);
      }
      line(context, centerX - fontSize * 0.2, top + height * 0.1, centerX + fontSize * 0.2, top + height * 0.1, "#ffde71", fontSize);
      return;
    case "pilot-position": {
      const markerX = centerX + element.ratio * width * 0.4;
      line(context, left + width * 0.1, centerY, left + width * 0.9, centerY, "#dffaf3", fontSize);
      line(context, centerX, top + height * 0.3, centerX, top + height * 0.7, "#9dc9cc", fontSize);
      line(context, markerX, top + height * 0.15, markerX, top + height * 0.85, "#ffde71", fontSize);
      return;
    }
    case "wind": {
      const radians = element.degrees * Math.PI / 180;
      const radius = Math.min(width * 0.3, height * 0.4);
      const endX = centerX + Math.sin(radians) * radius;
      const endY = centerY - Math.cos(radians) * radius;
      line(context, centerX, centerY, endX, endY, "#7df4c5", fontSize);
      line(context, endX, endY, endX - Math.sin(radians - 0.5) * radius * 0.35, endY + Math.cos(radians - 0.5) * radius * 0.35, "#7df4c5", fontSize);
      line(context, endX, endY, endX - Math.sin(radians + 0.5) * radius * 0.35, endY + Math.cos(radians + 0.5) * radius * 0.35, "#7df4c5", fontSize);
      return;
    }
    case "angle-of-attack":
    case "flight-path": {
      const ratio = Math.max(0, Math.min(1, (element.degrees + 20) / 40));
      const innerWidth = width * 0.9;
      const innerLeft = left + width * 0.05;
      line(context, innerLeft, centerY, innerLeft + innerWidth, centerY, "#dffaf3", fontSize);
      line(context, centerX, top + height * 0.3, centerX, top + height * 0.7, "#9dc9cc", fontSize);
      line(context, innerLeft + ratio * innerWidth, top + height * 0.15, innerLeft + ratio * innerWidth, top + height * 0.85, "#7df4c5", fontSize);
      return;
    }
  }
}

function line(context: HeadHudDrawingContext, startX: number, startY: number, endX: number, endY: number, color: string, fontSize: number): void {
  context.beginPath();
  context.setStrokeStyle(color);
  context.setLineWidth(Math.max(1, fontSize * 0.1));
  context.moveTo(startX, startY);
  context.lineTo(endX, endY);
  context.stroke();
}
