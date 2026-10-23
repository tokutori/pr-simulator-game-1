import type { MenuRect, SurfacePaintItem } from "../generated/surface-paint-item.js";
import type { SurfaceDrawingContext } from "./browser-canvas.js";

export function paintSurface(context: SurfaceDrawingContext, items: readonly SurfacePaintItem[], scaleX: number, scaleY: number): void {
  const rectangle = (bounds: MenuRect): void => { context.rect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY); };
  const paint = (items: readonly SurfacePaintItem[]): void => {
    for (const item of items) {
      switch (item.kind) {
        case "rectangle": {
          context.setGlobalAlpha(item.opacity);
          const bounds = item.bounds;
          if (item.gradient === null) {
            context.setFillStyle(item.color);
            context.fillRect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY);
          } else context.fillGradientRect(bounds.x * scaleX, bounds.y * scaleY, bounds.width * scaleX, bounds.height * scaleY, item.gradient);
          context.setGlobalAlpha(1);
          break;
        }
        case "text":
          context.setGlobalAlpha(item.opacity);
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
          context.setGlobalAlpha(1);
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
  paint(items);
}
