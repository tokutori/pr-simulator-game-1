import type { UiControl, UiPanel } from "../render/contracts/ui.js";

export const VR_PANEL_PIXELS = Object.freeze({ width: 1024, height: 768 });

export interface PanelDrawingContext {
  clearRect(x: number, y: number, width: number, height: number): void;
  fillRect(x: number, y: number, width: number, height: number): void;
  fillText(text: string, x: number, y: number, maxWidth: number): void;
  strokeRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
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
}

function drawControl(context: PanelDrawingContext, control: UiControl, width: number, height: number): void {
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
  context.setFont("500 24px system-ui, sans-serif");
  context.setTextBaseline("middle");
  const value = controlValue(control);
  context.fillText(value === "" ? control.label : `${control.label}  ${value}`, x + 20, y + controlHeight / 2, controlWidth - 40);
  context.setGlobalAlpha(1);
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
  }
}
