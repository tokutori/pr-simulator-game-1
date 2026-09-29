import { APP_BUILD_LABEL } from "./build-info.js";
import { chartScaleBarDistance, fitPlotRectToEqualScale, formatChartTick } from "../render/contracts/ui.js";
import type { UiChart, UiControl, UiPanel } from "../render/contracts/ui.js";
import type { FlightHudModel } from "./flight-hud-model.js";

export const VR_PANEL_PIXELS = Object.freeze({ width: 1024, height: 768 });

export interface PanelDrawingContext {
  clearRect(x: number, y: number, width: number, height: number): void;
  fillRect(x: number, y: number, width: number, height: number): void;
  fillText(text: string, x: number, y: number, maxWidth: number): void;
  strokeRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  closePath(): void;
  rect(x: number, y: number, width: number, height: number): void;
  clip(): void;
  save(): void;
  restore(): void;
  fill(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  stroke(): void;
  setFillStyle(value: string): void;
  setStrokeStyle(value: string): void;
  setFont(value: string): void;
  setTextBaseline(value: "middle"): void;
  setLineWidth(value: number): void;
  setGlobalAlpha(value: number): void;
}

export function drawVrFlightInstruments(context: PanelDrawingContext, model: FlightHudModel): void {
  context.setGlobalAlpha(1);
  context.setFillStyle("#d5e0dc");
  context.setFont("500 22px system-ui, sans-serif");
  context.setTextBaseline("middle");
  context.fillText(model.status, 52, 151, 920);
  if (model.warning !== null) {
    context.setFillStyle("#ff9b73");
    context.setFont("700 18px system-ui, sans-serif");
    context.fillText(model.warning, 52, 168, 920);
  }

  const readoutLines = model.readouts.split("\n");
  if (model.attitude !== null) {
    drawInstrumentCard(context, 42, 174, 474, 326, "ADI · PITCH / ROLL");
    if (readoutLines[0] !== undefined) {
      context.setFillStyle("#f3f4e8");
      context.setFont("500 19px system-ui, sans-serif");
      context.fillText(readoutLines[0], 58, 205, 442);
    }
    drawVrAdi(context, model, { x: 58, y: 222, width: 442, height: 260 });
  } else {
    drawInstrumentCard(context, 42, 174, 940, 326, "FLIGHT DATA");
    context.setFillStyle("#f3f4e8");
    context.setFont("500 24px system-ui, sans-serif");
    readoutLines.forEach((line, index) => {
      context.fillText(line, 74, 258 + index * 56, 876);
    });
  }

  if (model.headingDegrees !== null) {
    drawInstrumentCard(context, 532, 174, 450, 102, "HDG");
    drawHeadingTape(context, model.headingDegrees, 548, 209, 418, 54);
  }
  if (model.pilotPositionRatio !== null) {
    drawInstrumentCard(context, 532, 290, 450, 88, "PILOT CG · FORWARD / AFT");
    drawPilotPosition(context, model.pilotPositionRatio, 555, 337, 402);
  }
  if (model.windDirectionDegrees !== null && model.wind !== null) {
    drawInstrumentCard(context, 532, 392, 214, 132, "WIND VECTOR");
    drawWindVector(context, model.windDirectionDegrees, 602, 455, 43);
    context.setFillStyle("#f3f4e8");
    context.setFont("400 14px system-ui, sans-serif");
    context.fillText(model.wind, 545, 507, 188);
  }
  if (model.angleOfAttackDegrees !== null) {
    drawInstrumentCard(context, 762, 392, 220, 132, "ANGLE OF ATTACK");
    context.setFillStyle("#f3f4e8");
    context.setFont("600 25px system-ui, sans-serif");
    context.fillText(model.angleOfAttack ?? "—", 785, 434, 175);
    drawAngleOfAttack(context, model.angleOfAttackDegrees, 780, 476, 184);
  }

  if (model.telemetry !== "") {
    context.setFillStyle("#f3f4e8");
    context.setFont("400 17px system-ui, sans-serif");
    context.fillText(model.telemetry, 52, 548, 920);
  }
}

function drawInstrumentCard(
  context: PanelDrawingContext,
  x: number,
  y: number,
  width: number,
  height: number,
  label: string
): void {
  context.setGlobalAlpha(1);
  context.setFillStyle("#183139");
  context.fillRect(x, y, width, height);
  context.setStrokeStyle("#91b4b3");
  context.setLineWidth(2);
  context.strokeRect(x, y, width, height);
  context.setFillStyle("#b9c9c2");
  context.setFont("600 15px system-ui, sans-serif");
  context.setTextBaseline("middle");
  context.fillText(label, x + 12, y + 15, width - 24);
}

function drawVrAdi(
  context: PanelDrawingContext,
  model: FlightHudModel,
  bounds: Readonly<{ x: number; y: number; width: number; height: number }>
): void {
  if (model.attitude === null) {
    context.setFillStyle("#d5e0dc");
    context.setFont("500 20px system-ui, sans-serif");
    context.fillText("姿勢計器 unavailable", bounds.x + 12, bounds.y + bounds.height / 2, bounds.width - 24);
    return;
  }
  const centerX = bounds.x + bounds.width / 2;
  const centerY = bounds.y + bounds.height / 2;
  const rollRadians = -model.attitude.rollDegrees * Math.PI / 180;
  const direction = { x: Math.cos(rollRadians), y: Math.sin(rollRadians) };
  const normal = { x: -direction.y, y: direction.x };
  const pitchShift = Math.max(-55, Math.min(55, model.attitude.pitchDegrees)) * 2.2;
  const horizonY = centerY + pitchShift;
  const diagonal = Math.hypot(bounds.width, bounds.height) * 2;
  const firstHorizonPoint = { x: centerX - direction.x * diagonal, y: horizonY - direction.y * diagonal };
  const secondHorizonPoint = { x: centerX + direction.x * diagonal, y: horizonY + direction.y * diagonal };
  context.save();
  context.beginPath();
  context.rect(bounds.x, bounds.y, bounds.width, bounds.height);
  context.clip();
  context.setFillStyle("#397d9a");
  context.fillRect(bounds.x, bounds.y, bounds.width, bounds.height);
  const normalReach = diagonal * 2;
  context.beginPath();
  context.moveTo(firstHorizonPoint.x, firstHorizonPoint.y);
  context.lineTo(secondHorizonPoint.x, secondHorizonPoint.y);
  context.lineTo(secondHorizonPoint.x + normal.x * normalReach, secondHorizonPoint.y + normal.y * normalReach);
  context.lineTo(firstHorizonPoint.x + normal.x * normalReach, firstHorizonPoint.y + normal.y * normalReach);
  context.closePath();
  context.setFillStyle("#9a7047");
  context.fill();
  drawCanvasLine(context, firstHorizonPoint.x, firstHorizonPoint.y,
    secondHorizonPoint.x, secondHorizonPoint.y, "#f3f4e8", 4);

  for (const pitchMark of [-30, -20, -10, 10, 20, 30]) {
    const offset = -pitchMark * 2.2;
    const lineCenterX = centerX + normal.x * offset;
    const lineCenterY = horizonY + normal.y * offset;
    const halfLength = Math.abs(pitchMark) % 20 === 0 ? 48 : 30;
    const first = { x: lineCenterX - direction.x * halfLength, y: lineCenterY - direction.y * halfLength };
    const second = { x: lineCenterX + direction.x * halfLength, y: lineCenterY + direction.y * halfLength };
    if (![first.x, first.y, second.x, second.y].every(Number.isFinite)) continue;
    drawCanvasLine(context, first.x, first.y, second.x, second.y, "#d5e0dc", 2);
  }
  drawCanvasLine(context, centerX - 58, centerY, centerX - 18, centerY, "#ffd45c", 5);
  drawCanvasLine(context, centerX - 18, centerY, centerX - 10, centerY - 10, "#ffd45c", 5);
  drawCanvasLine(context, centerX - 10, centerY - 10, centerX + 10, centerY - 10, "#ffd45c", 5);
  drawCanvasLine(context, centerX + 10, centerY - 10, centerX + 18, centerY, "#ffd45c", 5);
  drawCanvasLine(context, centerX + 18, centerY, centerX + 58, centerY, "#ffd45c", 5);
  drawCanvasLine(context, centerX, centerY - 10, centerX, centerY + 8, "#ffd45c", 5);
  if (model.flightPathAngleDegrees !== null) {
    const markerY = centerY - Math.max(-30, Math.min(30, model.flightPathAngleDegrees)) * 2.2;
    drawCanvasLine(context, centerX - 7, markerY, centerX, markerY - 7, "#7df4c5", 3);
    drawCanvasLine(context, centerX, markerY - 7, centerX + 7, markerY, "#7df4c5", 3);
    drawCanvasLine(context, centerX + 7, markerY, centerX, markerY + 7, "#7df4c5", 3);
    drawCanvasLine(context, centerX, markerY + 7, centerX - 7, markerY, "#7df4c5", 3);
  }
  context.restore();
  context.setStrokeStyle("#e8eee5");
  context.setLineWidth(3);
  context.strokeRect(bounds.x, bounds.y, bounds.width, bounds.height);
  context.setFillStyle("#f3f4e8");
  context.setFont("400 15px system-ui, sans-serif");
  context.fillText(`PITCH ${model.attitude.pitchDegrees.toFixed(0)}° · ROLL ${model.attitude.rollDegrees.toFixed(0)}°`, bounds.x + 8, bounds.y + bounds.height - 4, bounds.width - 16);
}

function drawHeadingTape(context: PanelDrawingContext, headingDegrees: number, x: number, y: number, width: number, height: number): void {
  const centerX = x + width / 2;
  context.setFillStyle("#f3f4e8");
  context.setFont("600 26px system-ui, sans-serif");
  context.fillText(`${headingDegrees.toFixed(0)}°`, centerX - 36, y + 12, 72);
  for (let offset = -60; offset <= 60; offset += 10) {
    const tickX = centerX + offset * 3.2;
    drawCanvasLine(context, tickX, y + 34, tickX, y + (offset % 30 === 0 ? height - 3 : height - 15),
      offset % 30 === 0 ? "#f3f4e8" : "#91b4b3", offset % 30 === 0 ? 2 : 1);
    if (offset % 30 === 0) {
      context.setFillStyle("#d5e0dc");
      context.setFont("400 13px system-ui, sans-serif");
      context.fillText(String(normalizeBearing(headingDegrees + offset)), tickX - 16, y + 27, 32);
    }
  }
  context.setFillStyle("#ffd45c");
  context.fillRect(centerX - 2, y + 32, 4, height - 30);
}

function drawPilotPosition(context: PanelDrawingContext, ratio: number, x: number, y: number, width: number): void {
  drawCanvasLine(context, x, y, x + width, y, "#b9c9c2", 3);
  for (let index = 0; index <= 8; index += 1) {
    const tickX = x + width * index / 8;
    drawCanvasLine(context, tickX, y - 9, tickX, y + 9, "#b9c9c2", 1);
  }
  const indicatorX = x + (Math.max(-1, Math.min(1, ratio)) + 1) * width / 2;
  context.setFillStyle("#ffd45c");
  context.fillRect(indicatorX - 5, y - 17, 10, 18);
  context.setFillStyle("#f3f4e8");
  context.setFont("500 17px system-ui, sans-serif");
  context.fillText("AFT", x, y + 27, 40);
  context.fillText("FWD", x + width - 40, y + 27, 40);
}

function drawWindVector(context: PanelDrawingContext, directionDegrees: number, centerX: number, centerY: number, radius: number): void {
  context.setStrokeStyle("#526c70");
  context.setLineWidth(1);
  context.beginPath();
  context.moveTo(centerX - radius, centerY);
  context.lineTo(centerX + radius, centerY);
  context.moveTo(centerX, centerY - radius);
  context.lineTo(centerX, centerY + radius);
  context.stroke();
  context.setFillStyle("#f3f4e8");
  context.setFont("400 13px system-ui, sans-serif");
  context.fillText("N", centerX - 5, centerY - radius - 8, 18);
  const angle = directionDegrees * Math.PI / 180;
  const endX = centerX + Math.sin(angle) * (radius - 8);
  const endY = centerY - Math.cos(angle) * (radius - 8);
  drawCanvasLine(context, centerX, centerY, endX, endY, "#ffd45c", 4);
  const headAngle = Math.atan2(endY - centerY, endX - centerX);
  drawCanvasLine(context, endX, endY, endX - 13 * Math.cos(headAngle - Math.PI / 6), endY - 13 * Math.sin(headAngle - Math.PI / 6), "#ffd45c", 4);
  drawCanvasLine(context, endX, endY, endX - 13 * Math.cos(headAngle + Math.PI / 6), endY - 13 * Math.sin(headAngle + Math.PI / 6), "#ffd45c", 4);
}

function drawAngleOfAttack(context: PanelDrawingContext, angleDegrees: number, x: number, y: number, width: number): void {
  drawCanvasLine(context, x, y, x + width, y, "#b9c9c2", 3);
  for (let index = 0; index <= 6; index += 1) {
    const tickX = x + width * index / 6;
    drawCanvasLine(context, tickX, y - 8, tickX, y + 8, "#b9c9c2", 1);
  }
  const ratio = (Math.max(-10, Math.min(20, angleDegrees)) + 10) / 30;
  context.setFillStyle("#ffd45c");
  context.fillRect(x + width * ratio - 4, y - 13, 8, 16);
}

function drawCanvasLine(context: PanelDrawingContext, x1: number, y1: number, x2: number, y2: number, color: string, width: number): void {
  context.setStrokeStyle(color);
  context.setLineWidth(width);
  context.beginPath();
  context.moveTo(x1, y1);
  context.lineTo(x2, y2);
  context.stroke();
}

function normalizeBearing(degrees: number): number {
  return ((Math.round(degrees) % 360) + 360) % 360;
}

export function drawVrPanel(
  context: PanelDrawingContext,
  panel: UiPanel,
  width: number,
  height: number
): void {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    throw new RangeError("Panel drawing dimensions must be positive and finite");
  }
  context.clearRect(0, 0, width, height);
  context.setFillStyle("#10242b");
  context.fillRect(0, 0, width, height);
  context.setFillStyle("#f3f4e8");
  context.setFont("600 36px system-ui, sans-serif");
  context.setTextBaseline("middle");
  context.fillText(panel.title, width * 0.08, height * 0.09, width * 0.84);
  context.setStrokeStyle("#6c898d");
  context.setLineWidth(2);
  context.beginPath();
  context.moveTo(width * 0.08, height * 0.16);
  context.lineTo(width * 0.92, height * 0.16);
  context.stroke();
  for (const control of panel.controls) drawControl(context, control, width, height);
  context.setFillStyle("#b9c9c2");
  context.setFont("400 16px system-ui, sans-serif");
  context.fillText(APP_BUILD_LABEL, width * 0.08, height * 0.96, width * 0.84);
}

function drawControl(context: PanelDrawingContext, control: UiControl, width: number, height: number): void {
  if (control.kind === "chart") {
    drawChart(context, control, width, height);
    return;
  }
  const x = control.rect.x * width;
  const y = control.rect.y * height;
  const controlWidth = control.rect.width * width;
  const controlHeight = control.rect.height * height;
  context.setGlobalAlpha(control.enabled ? 1 : 0.45);
  context.setFillStyle(control.kind === "button" ? "#31555a" : "#233c42");
  context.fillRect(x, y, controlWidth, controlHeight);
  context.setStrokeStyle("#91b4b3");
  context.setLineWidth(2);
  context.strokeRect(x, y, controlWidth, controlHeight);
  context.setFillStyle("#f3f4e8");
  context.setTextBaseline("middle");
  const value = controlValue(control);
  const lines = (value === "" ? control.label : `${control.label}  ${value}`).split("\n");
  context.setFont(lines.length > 1 ? "500 17px system-ui, sans-serif" : "500 24px system-ui, sans-serif");
  const lineSpacing = lines.length > 1 ? 22 : 0;
  const firstLineY = y + controlHeight / 2 - (lines.length - 1) * lineSpacing / 2;
  lines.forEach((line, index) => {
    context.fillText(line, x + 20, firstLineY + index * lineSpacing, controlWidth - 40);
  });
  context.setGlobalAlpha(1);
}

function drawChart(context: PanelDrawingContext, chart: UiChart, width: number, height: number): void {
  const x = chart.rect.x * width;
  const y = chart.rect.y * height;
  const chartWidth = chart.rect.width * width;
  const chartHeight = chart.rect.height * height;
  const plot = fitPlotRectToEqualScale(
    x + chartWidth * 0.09,
    y + chartHeight * 0.12,
    x + chartWidth * 0.98,
    y + chartHeight * 0.78,
    chart.equalAxisScale
  );
  const { left, right, top, bottom } = plot;
  context.setGlobalAlpha(1);
  context.setFillStyle("#183139");
  context.fillRect(x, y, chartWidth, chartHeight);
  context.setStrokeStyle("#91b4b3");
  context.setLineWidth(2);
  context.beginPath();
  context.moveTo(left, top);
  context.lineTo(left, bottom);
  context.lineTo(right, bottom);
  context.stroke();
  for (let index = 1; index <= 3; index += 1) {
    const gridY = top + ((bottom - top) * index) / 4;
    context.setStrokeStyle("#456168");
    context.setLineWidth(1);
    context.beginPath();
    context.moveTo(left, gridY);
    context.lineTo(right, gridY);
    context.stroke();
  }
  context.setFillStyle("#d5e0dc");
  context.setFont("400 15px system-ui, sans-serif");
  for (let index = 0; index <= 4; index += 1) {
    const ratio = index / 4;
    const tickX = left + ratio * (right - left);
    const tickY = bottom - ratio * (bottom - top);
    context.fillText(
      formatChartTick(chart.xMinimum + ratio * (chart.xMaximum - chart.xMinimum)),
      tickX - 16,
      bottom + 14,
      42
    );
    context.fillText(
      formatChartTick(chart.yMinimum + ratio * (chart.yMaximum - chart.yMinimum)),
      left - 50,
      tickY,
      42
    );
  }
  for (const line of chart.referenceLines) {
    const lineY = bottom - ((line.value - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    context.setStrokeStyle(line.color);
    context.setLineWidth(2);
    context.beginPath();
    context.moveTo(left, lineY);
    context.lineTo(right, lineY);
    context.stroke();
    context.setFillStyle(line.color);
    context.fillText(line.label, left + 8, lineY - 10, chartWidth * 0.4);
  }
  for (const series of chart.series) {
    if (series.segmentColors !== undefined && series.points.length > 1) {
      series.segmentColors.forEach((color, index) => {
        const first = series.points[index];
        const second = series.points[index + 1];
        if (first === undefined || second === undefined) return;
        const firstX = left + ((first.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
        const firstY = bottom - ((first.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
        const secondX = left + ((second.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
        const secondY = bottom - ((second.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
        context.setStrokeStyle(color);
        context.setLineWidth(5);
        context.beginPath();
        context.moveTo(firstX, firstY);
        context.lineTo(secondX, secondY);
        context.stroke();
      });
      continue;
    }
    context.setStrokeStyle(series.color);
    context.setLineWidth(4);
    context.beginPath();
    series.points.forEach((point, index) => {
      const pointX = left + ((point.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
      const pointY = bottom - ((point.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
      if (index === 0) context.moveTo(pointX, pointY);
      else context.lineTo(pointX, pointY);
    });
    context.stroke();
  }
  if (chart.cursorX !== null) {
    const cursorX = left + ((chart.cursorX - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    context.setStrokeStyle("#f3f4e8");
    context.setLineWidth(2);
    context.beginPath();
    context.moveTo(cursorX, top);
    context.lineTo(cursorX, bottom);
    context.stroke();
  }
  chart.cursorPoints.forEach((point, index) => {
    const pointX = left + ((point.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    const pointY = bottom - ((point.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    context.setStrokeStyle(chart.series[index]?.color ?? "#f3f4e8");
    context.setLineWidth(3);
    context.beginPath();
    context.moveTo(pointX - 7, pointY);
    context.lineTo(pointX + 7, pointY);
    context.moveTo(pointX, pointY - 7);
    context.lineTo(pointX, pointY + 7);
    context.stroke();
  });
  for (const marker of chart.markers) {
    const markerX = left + ((marker.point.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    const markerY = bottom - ((marker.point.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    context.setStrokeStyle(marker.color);
    context.setLineWidth(3);
    context.beginPath();
    context.moveTo(markerX - 8, markerY);
    context.lineTo(markerX + 8, markerY);
    context.moveTo(markerX, markerY - 8);
    context.lineTo(markerX, markerY + 8);
    context.stroke();
    context.setFillStyle(marker.color);
    context.fillText(marker.label, markerX + 10, markerY - 10, chartWidth * 0.25);
  }
  for (const marker of chart.timeMarkers) {
    const markerX = left + ((marker.point.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    const markerY = bottom - ((marker.point.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    context.setFillStyle(marker.color);
    context.fillRect(markerX - 3, markerY - 3, 6, 6);
    context.fillText(marker.label, markerX + 6, markerY + 12, chartWidth * 0.2);
  }
  for (const vector of chart.vectors) {
    if (vector.start === null || vector.end === null) continue;
    const startX = left + ((vector.start.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    const startY = bottom - ((vector.start.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    const endX = left + ((vector.end.x - chart.xMinimum) / (chart.xMaximum - chart.xMinimum)) * (right - left);
    const endY = bottom - ((vector.end.y - chart.yMinimum) / (chart.yMaximum - chart.yMinimum)) * (bottom - top);
    const angle = Math.atan2(endY - startY, endX - startX);
    const arrowLength = 11;
    context.setStrokeStyle(vector.color);
    context.setLineWidth(4);
    context.beginPath();
    context.moveTo(startX, startY);
    context.lineTo(endX, endY);
    context.moveTo(endX, endY);
    context.lineTo(endX - arrowLength * Math.cos(angle - Math.PI / 6), endY - arrowLength * Math.sin(angle - Math.PI / 6));
    context.moveTo(endX, endY);
    context.lineTo(endX - arrowLength * Math.cos(angle + Math.PI / 6), endY - arrowLength * Math.sin(angle + Math.PI / 6));
    context.stroke();
  }
  if (chart.equalAxisScale) {
    const scaleDistance = chartScaleBarDistance(chart.xMaximum - chart.xMinimum);
    const scaleWidth = scaleDistance / (chart.xMaximum - chart.xMinimum) * (right - left);
    const scaleX = right - scaleWidth - 12;
    const scaleY = bottom - 16;
    context.setStrokeStyle("#f3f4e8");
    context.setLineWidth(3);
    context.beginPath();
    context.moveTo(scaleX, scaleY);
    context.lineTo(scaleX + scaleWidth, scaleY);
    context.moveTo(scaleX, scaleY - 5);
    context.lineTo(scaleX, scaleY + 3);
    context.moveTo(scaleX + scaleWidth, scaleY - 5);
    context.lineTo(scaleX + scaleWidth, scaleY + 3);
    context.stroke();
    context.setFillStyle("#f3f4e8");
    context.fillText(`${formatChartTick(scaleDistance)} m`, scaleX + scaleWidth / 2 - 22, scaleY - 12, 70);
    const northX = right - 22;
    const northTop = top + 18;
    const northBottom = northTop + 34;
    context.beginPath();
    context.moveTo(northX, northBottom);
    context.lineTo(northX, northTop);
    context.moveTo(northX, northTop);
    context.lineTo(northX - 6, northTop + 10);
    context.moveTo(northX, northTop);
    context.lineTo(northX + 6, northTop + 10);
    context.stroke();
    context.fillText("N", northX - 5, northTop - 8, 20);
  }
  context.setFillStyle("#f3f4e8");
  context.setFont("500 18px system-ui, sans-serif");
  context.setTextBaseline("middle");
  context.fillText(chart.label, x + 8, y + chartHeight * 0.05, chartWidth * 0.8);
  context.fillText(chart.yAxisLabel, x + 8, y + chartHeight * 0.15, chartWidth * 0.8);
  context.fillText(chart.xAxisLabel, x + chartWidth * 0.36, y + chartHeight * 0.94, chartWidth * 0.6);
  let legendX = left;
  for (const series of chart.series) {
    const legendY = y + chartHeight * 0.88;
    if (series.segmentColors !== undefined && series.segmentColors.length > 0) {
      const midpoint = legendX + 10;
      context.setLineWidth(4);
      context.setStrokeStyle(series.segmentColors[0] ?? series.color);
      context.beginPath();
      context.moveTo(legendX, legendY);
      context.lineTo(midpoint, legendY);
      context.stroke();
      context.setStrokeStyle(series.segmentColors[series.segmentColors.length - 1] ?? series.color);
      context.beginPath();
      context.moveTo(midpoint, legendY);
      context.lineTo(legendX + 20, legendY);
      context.stroke();
    } else {
      context.setStrokeStyle(series.color);
      context.setLineWidth(4);
      context.beginPath();
      context.moveTo(legendX, legendY);
      context.lineTo(legendX + 20, legendY);
      context.stroke();
    }
    context.setFillStyle("#f3f4e8");
    context.fillText(series.label, legendX + 26, legendY, chartWidth * 0.35);
    legendX += chartWidth * 0.45;
  }
  for (const vector of chart.vectors) {
    if (vector.label.length === 0) continue;
    context.setFillStyle(vector.color);
    context.fillText(vector.label, left, y + chartHeight * 0.97, chartWidth * 0.9);
  }
  for (const marker of chart.markers) {
    context.setFillStyle(marker.color);
    context.fillText(marker.label, left, y + chartHeight * 0.92, chartWidth * 0.2);
  }
}

function controlValue(control: UiControl): string {
  switch (control.kind) {
    case "button":
      return "";
    case "toggle":
      return control.value ? "ON" : "OFF";
    case "range":
      return control.value.toFixed(1);
    case "status":
      return control.value;
    case "chart":
      return "";
  }
}
