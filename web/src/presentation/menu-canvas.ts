import type { MenuRect, MenuViewport } from "../render/contracts/menu-layout.js";
import type { SurfaceDrawingContext } from "./browser-canvas.js";
import { paintSurface } from "./surface-canvas.js";

export function menuCanvasSize(size: MenuViewport["size"]): Readonly<{ width: number; height: number }> {
  if (![size.width, size.height].every((value) => Number.isFinite(value) && value > 0)) throw new RangeError("Menu surface dimensions must be positive and finite");
  const scale = 1536 / Math.max(size.width, size.height);
  return Object.freeze({ width: Math.max(1, Math.round(size.width * scale)), height: Math.max(1, Math.round(size.height * scale)) });
}


export function drawMeasuredMenu(context: SurfaceDrawingContext, viewport: MenuViewport, canvasWidth: number, canvasHeight: number): void {
  if (![canvasWidth, canvasHeight].every(value => Number.isInteger(value) && value > 0)) throw new RangeError("Menu canvas dimensions must be positive integers");
  const scaleX = canvasWidth / viewport.size.width;
  const scaleY = canvasHeight / viewport.size.height;
  const rectangle = (bounds: MenuRect): void => { context.rect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY); };
  context.clearRect(0, 0, canvasWidth, canvasHeight);
  context.save();
  try {
    context.setGlobalAlpha(viewport.opacity);
    context.setFillStyle(viewport.background);
    context.fillRect(0, 0, canvasWidth, canvasHeight);
    context.setGlobalAlpha(1);
    context.save();
    try { context.beginPath(); rectangle(viewport.content.clip); context.clip(); paintSurface(context, viewport.content.items, scaleX, scaleY); }
    finally { context.restore(); }
    paintSurface(context, viewport.footer, scaleX, scaleY);
  } finally { context.restore(); }
}
