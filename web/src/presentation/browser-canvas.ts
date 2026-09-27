import type { PanelDrawingContext } from "./vr-panel-canvas.js";

export function browserPanelContext(context: CanvasRenderingContext2D): PanelDrawingContext {
  return {
    clearRect: (x, y, width, height) => { context.clearRect(x, y, width, height); },
    fillRect: (x, y, width, height) => { context.fillRect(x, y, width, height); },
    fillText: (text, x, y, maxWidth) => { context.fillText(text, x, y, maxWidth); },
    strokeRect: (x, y, width, height) => { context.strokeRect(x, y, width, height); },
    beginPath: () => { context.beginPath(); },
    moveTo: (x, y) => { context.moveTo(x, y); },
    lineTo: (x, y) => { context.lineTo(x, y); },
    stroke: () => { context.stroke(); },
    setFillStyle: (value) => { context.fillStyle = value; },
    setStrokeStyle: (value) => { context.strokeStyle = value; },
    setFont: (value) => { context.font = value; },
    setTextBaseline: (value) => { context.textBaseline = value; },
    setLineWidth: (value) => { context.lineWidth = value; },
    setGlobalAlpha: (value) => { context.globalAlpha = value; }
  };
}
