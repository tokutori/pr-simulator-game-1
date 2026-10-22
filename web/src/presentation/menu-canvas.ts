import type { MenuRect, MenuViewport } from "../render/contracts/menu-layout.js";
import { browserHeadHudContext } from "./head-hud-canvas.js";
import type { HeadHudDrawingContext } from "./head-hud-canvas.js";

export interface MenuDrawingContext extends HeadHudDrawingContext {
  fillGradientRect(left: number, top: number, width: number, height: number, colors: readonly [string, string]): void;
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
    fillGradientRect: (left: number, top: number, width: number, height: number, colors: readonly [string, string]): void => {
      const gradient = context.createLinearGradient(left, top, left + width, top);
      gradient.addColorStop(0, colors[0]);
      gradient.addColorStop(1, colors[1]);
      context.fillStyle = gradient;
      context.fillRect(left, top, width, height);
    },
    setTextBaseline: (value: "middle" | "alphabetic"): void => { context.textBaseline = value; },
    translate: (coordinateX: number, coordinateY: number): void => { context.translate(coordinateX, coordinateY); },
    rotate: (radians: number): void => { context.rotate(radians); }
  });
}

export function menuCanvasSize(size: MenuViewport["size"]): Readonly<{ width: number; height: number }> {
  if (![size.width, size.height].every((value) => Number.isFinite(value) && value > 0)) throw new RangeError("Menu surface dimensions must be positive and finite");
  const scale = 1536 / Math.max(size.width, size.height);
  return Object.freeze({ width: Math.max(1, Math.round(size.width * scale)), height: Math.max(1, Math.round(size.height * scale)) });
}


export function drawMeasuredMenu(context: MenuDrawingContext, viewport: MenuViewport, canvasWidth: number, canvasHeight: number): void {
  if (![canvasWidth, canvasHeight].every(value => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const scaleX = canvasWidth / viewport.size.width;
  const scaleY = canvasHeight / viewport.size.height;
  const rectangle = (bounds: MenuRect): void => { context.rect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY); };
  const paint = (items: MenuViewport["footer"]): void => {
    for (const item of items) {
      switch (item.kind) {
        case "rectangle": {
          const bounds = item.bounds;
          if (item.gradient === null) {
            context.setFillStyle(item.color);
            context.fillRect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY);
          } else context.fillGradientRect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY, item.gradient);
          break;
        }
        case "text":
          context.setLocale(item.locale);
          context.setFont(item.font.style + " " + String(item.font.weight) + " " + String(item.fontMeters * scaleY) + "px " + item.font.family);
          context.setTextBaseline("alphabetic");
          context.setTextAlign("left");
          context.setFillStyle(item.color);
          context.save();
          try {
            context.translate(item.anchor.x * scaleX, item.anchor.y * scaleY);
            context.rotate(item.rotationDegrees * Math.PI / 180);
            context.fillText(item.value, 0, 0);
          } finally { context.restore(); }
          break;
        case "line":
          context.setGlobalAlpha(item.opacity);
          context.setStrokeStyle(item.color);
          context.setLineWidth(item.widthPx);
          context.beginPath();
          context.moveTo(item.start.x * scaleX, item.start.y * scaleY);
          context.lineTo(item.end.x * scaleX, item.end.y * scaleY);
          context.stroke();
          context.setGlobalAlpha(1);
          break;
        case "marker":
          context.setFillStyle(item.color);
          context.fillRect(item.point.x * scaleX - item.sizePx / 2, item.point.y * scaleY - item.sizePx / 2, item.sizePx, item.sizePx);
          break;
        case "plot":
          context.save();
          try { context.beginPath(); rectangle(item.clip); context.clip(); paint(item.items); }
          finally { context.restore(); }
          break;
      }
    }
  };
  context.clearRect(0, 0, canvasWidth, canvasHeight);
  context.save();
  try {
    context.setGlobalAlpha(viewport.opacity);
    context.setFillStyle(viewport.background);
    context.fillRect(0, 0, canvasWidth, canvasHeight);
    context.setGlobalAlpha(1);
    context.save();
    try { context.beginPath(); rectangle(viewport.content.clip); context.clip(); paint(viewport.content.items); }
    finally { context.restore(); }
    paint(viewport.footer);
  } finally { context.restore(); }
}
