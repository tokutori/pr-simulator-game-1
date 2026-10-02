import type { HeadHudDrawingContext, HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";

export class HudCanvasFixture implements HeadHudDrawingContext {
  readonly painted: string[] = [];
  readonly clears: number[][] = [];
  readonly fills: number[][] = [];
  readonly fonts: string[] = [];
  invalidMetrics = false;
  private fontSize = 1;
  private readonly states: number[] = [];
  clearRect(...dimensions: [number, number, number, number]): void { this.clears.push(dimensions); }
  fillRect(...dimensions: [number, number, number, number]): void { this.fills.push(dimensions); }
  fillText(value: string): void { this.painted.push(value); }
  measureText(value: string): HeadHudTextMetrics {
    const width = Array.from(value).reduce((sum, character) => sum + this.fontSize * (character.charCodeAt(0) > 255 ? 1 : 0.65), 0);
    return { width, left: 0, right: width, ascent: this.invalidMetrics ? Number.NaN : this.fontSize * 0.7, descent: this.fontSize * 0.2 };
  }
  save(): void { this.states.push(this.fontSize); }
  restore(): void { this.fontSize = this.states.pop() ?? 1; }
  setFont(value: string): void { this.fonts.push(value); this.fontSize = Number(value.match(/([\d.]+)px/)?.[1] ?? 1); }
  setFillStyle(): void {}
  setStrokeStyle(): void {}
  setTextBaseline(): void {}
  setTextAlign(): void {}
  setGlobalAlpha(): void {}
  setLineWidth(): void {}
  strokeRect(): void {}
  beginPath(): void {}
  closePath(): void {}
  rect(): void {}
  clip(): void {}
  fill(): void {}
  moveTo(): void {}
  lineTo(): void {}
  stroke(): void {}
}

export function convexQuadsOverlap(first: readonly Readonly<{ x: number; y: number }>[], second: readonly Readonly<{ x: number; y: number }>[]): boolean {
  for (const polygon of [first, second]) for (const [index, point] of polygon.entries()) {
    const next = polygon[(index + 1) % polygon.length];
    if (next === undefined) throw new Error("Missing polygon corner");
    const normal = { x: next.y - point.y, y: point.x - next.x };
    const project = (corners: typeof first) => corners.map((corner) => normal.x * corner.x + normal.y * corner.y);
    const firstValues = project(first);
    const secondValues = project(second);
    if (Math.max(...firstValues) <= Math.min(...secondValues) || Math.max(...secondValues) <= Math.min(...firstValues)) return false;
  }
  return true;
}
