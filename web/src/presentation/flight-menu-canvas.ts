import type { UiPanel } from "../render/contracts/ui.js";
import { prepareHudTextLines } from "./head-hud-canvas.js";
import type { HeadHudDrawingContext, PaintedLine } from "./head-hud-canvas.js";

interface MenuText {
  readonly fontSize: number;
  readonly lines: readonly PaintedLine[];
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly enabled: boolean;
  readonly button: boolean;
}

export type FlightMenuPaintPreparation = Readonly<{ width: number; height: number }> & (
  | { readonly kind: "unavailable"; readonly reason: "text-overflow" }
  | { readonly kind: "ready"; readonly texts: readonly MenuText[] }
);

export function flightMenuCanvasSize(panel: UiPanel): Readonly<{ width: number; height: number }> {
  const scale = 1024 / Math.max(panel.size.width, panel.size.height);
  return Object.freeze({ width: Math.max(1, Math.round(panel.size.width * scale)), height: Math.max(1, Math.round(panel.size.height * scale)) });
}

export function prepareFlightMenuPaint(context: HeadHudDrawingContext, panel: UiPanel, width: number, height: number): FlightMenuPaintPreparation {
  if (![width, height].every((value) => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const texts: MenuText[] = [];
  const specifications = [
    { value: panel.title, fontMeters: 0.024, left: 0.06, top: 0.04, width: 0.88, height: 0.14, enabled: true, button: false },
    ...panel.controls.map((control) => ({ value: control.kind === "status" ? control.value : control.label,
      fontMeters: control.kind === "button" ? 0.034 : 0.024,
      left: control.rect.x, top: control.rect.y, width: control.rect.width, height: control.rect.height,
      enabled: control.kind === "status" || control.enabled, button: control.kind === "button" }))
  ];
  context.save();
  context.setTextBaseline("middle");
  context.setTextAlign("left");
  for (const specification of specifications) {
    const fontSize = specification.fontMeters * height / panel.size.height;
    const lineHeight = fontSize * 1.35;
    const padding = specification.button ? fontSize * 0.5 : 0;
    context.setFont(`600 ${String(fontSize)}px monospace`);
    const lines = prepareHudTextLines(context, specification.value, specification.width * width - padding * 2, lineHeight);
    if (lines === null || lines.length * lineHeight + padding * 2 > specification.height * height + 1e-6) {
      context.restore();
      return Object.freeze({ kind: "unavailable", reason: "text-overflow", width, height });
    }
    texts.push(Object.freeze({ fontSize, lines: Object.freeze(lines), left: specification.left * width,
      top: specification.top * height, width: specification.width * width, height: specification.height * height,
      enabled: specification.enabled, button: specification.button }));
  }
  context.restore();
  return Object.freeze({ kind: "ready", width, height, texts: Object.freeze(texts) });
}

export function drawFlightMenu(context: HeadHudDrawingContext, preparation: FlightMenuPaintPreparation): void {
  context.clearRect(0, 0, preparation.width, preparation.height);
  if (preparation.kind !== "ready") return;
  context.save();
  context.setTextBaseline("middle");
  context.setTextAlign("left");
  context.setGlobalAlpha(0.82);
  context.setFillStyle("#10242d");
  context.fillRect(0, 0, preparation.width, preparation.height);
  for (const text of preparation.texts) {
    context.setGlobalAlpha(1);
    context.setFont(`600 ${String(text.fontSize)}px monospace`);
    if (text.button) {
      context.setFillStyle(text.enabled ? "#294853" : "#24343a");
      context.fillRect(text.left, text.top, text.width, text.height);
    }
    context.setFillStyle(text.enabled ? "#f3fff9" : "#899ba0");
    const lineHeight = text.fontSize * 1.35;
    let lineTop = text.top + (text.height - text.lines.length * lineHeight) / 2;
    for (const line of text.lines) {
      const inkWidth = Math.max(0, line.metrics.left) + line.metrics.right;
      context.fillText(line.value, text.left + (text.width - inkWidth) / 2 + Math.max(0, line.metrics.left),
        lineTop + lineHeight / 2 + (line.metrics.ascent - line.metrics.descent) / 2);
      lineTop += lineHeight;
    }
  }
  context.restore();
}
