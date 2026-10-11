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

export function browserPanelContext(context: CanvasRenderingContext2D): PanelDrawingContext {
  return {
    clearRect: (x, y, width, height) => { context.clearRect(x, y, width, height); },
    fillRect: (x, y, width, height) => { context.fillRect(x, y, width, height); },
    fillText: (text, x, y, maxWidth) => { context.fillText(text, x, y, maxWidth); },
    strokeRect: (x, y, width, height) => { context.strokeRect(x, y, width, height); },
    beginPath: () => { context.beginPath(); },
    closePath: () => { context.closePath(); },
    rect: (x, y, width, height) => { context.rect(x, y, width, height); },
    clip: () => { context.clip(); },
    save: () => { context.save(); },
    restore: () => { context.restore(); },
    fill: () => { context.fill(); },
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
