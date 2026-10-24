export interface PanelDrawingContext {
  clearRect(x: number, y: number, width: number, height: number): void;
  fillRect(x: number, y: number, width: number, height: number): void;
  fillText(text: string, x: number, y: number, maxWidth?: number): void;
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
  setTextBaseline(value: "middle" | "alphabetic"): void;
  setLineWidth(value: number): void;
  setGlobalAlpha(value: number): void;
}

export interface SurfaceTextMetrics {
  readonly width: number;
  readonly left: number;
  readonly right: number;
  readonly ascent: number;
  readonly descent: number;
}

export interface SurfaceDrawingContext extends PanelDrawingContext {
  roundRect(left: number, top: number, width: number, height: number, radiusX: number, radiusY: number): void;
  setLocale(locale: string): void;
  measureText(value: string): SurfaceTextMetrics;
  setTextAlign(value: "left"): void;
  fillGradientRect(left: number, top: number, width: number, height: number, colors: readonly [string, string]): void;
  translate(coordinateX: number, coordinateY: number): void;
  rotate(radians: number): void;
}

export function browserSurfaceContext(context: CanvasRenderingContext2D): SurfaceDrawingContext {
  context.direction = "ltr";
  context.fontKerning = "normal";
  context.fontStretch = "normal";
  context.fontVariantCaps = "normal";
  context.letterSpacing = "0px";
  context.wordSpacing = "0px";
  return {
    ...browserPanelContext(context),
    roundRect: (left, top, width, height, radiusX, radiusY) => { context.roundRect(left, top, width, height, { x: radiusX, y: radiusY }); },
    setLocale: (locale) => {
      context.canvas.lang = locale;
      if ("lang" in context && typeof context.lang === "string") context.lang = locale;
    },
    setTextAlign: (value) => { context.textAlign = value; },
    measureText: (value) => {
      const metric = context.measureText(value);
      return { width: metric.width, left: metric.actualBoundingBoxLeft, right: metric.actualBoundingBoxRight,
        ascent: metric.actualBoundingBoxAscent, descent: metric.actualBoundingBoxDescent };
    },
    fillGradientRect: (left, top, width, height, colors) => {
      const gradient = context.createLinearGradient(left, top, left + width, top);
      gradient.addColorStop(0, colors[0]);
      gradient.addColorStop(1, colors[1]);
      context.fillStyle = gradient;
      context.fillRect(left, top, width, height);
    },
    translate: (coordinateX, coordinateY) => { context.translate(coordinateX, coordinateY); },
    rotate: (radians) => { context.rotate(radians); }
  };
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
