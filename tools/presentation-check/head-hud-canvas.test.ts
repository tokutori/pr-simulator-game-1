import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel } from "../../web/src/app/app-state.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { createFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { drawHeadHud, headHudCanvasSize, prepareHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import type { HeadHudDrawingContext, HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import { createHeadHudView } from "../../web/src/presentation/head-hud-view.js";
import type { HeadHudView } from "../../web/src/presentation/head-hud-view.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";

class RecordingHeadContext implements HeadHudDrawingContext {
  readonly texts: { value: string; alpha: number; arguments: number }[] = [];
  readonly fills: { alpha: number; left: number; top: number; width: number; height: number }[] = [];
  readonly clears: number[][] = [];
  readonly clips: number[][] = [];
  readonly fonts: string[] = [];
  private alpha = 1;
  private fontSize = 1;
  private readonly states: { alpha: number; fontSize: number }[] = [];
  invalidMetrics = false;
  excessiveInk = false;

  clearRect(...dimensions: [number, number, number, number]): void { this.clears.push(dimensions); }
  fillRect(left: number, top: number, width: number, height: number): void { this.fills.push({ alpha: this.alpha, left, top, width, height }); }
  fillText(...args: [string, number, number]): void { this.texts.push({ value: args[0], alpha: this.alpha, arguments: args.length }); }
  measureText(value: string): HeadHudTextMetrics {
    const width = Array.from(value).reduce((sum, character) => sum + this.fontSize * (character.charCodeAt(0) > 255 ? 1 : 0.65), 0);
    return { width, left: this.excessiveInk ? width * 10 : 0, right: width, ascent: this.invalidMetrics ? Number.NaN : this.fontSize * 0.7, descent: this.fontSize * 0.2 };
  }
  save(): void { this.states.push({ alpha: this.alpha, fontSize: this.fontSize }); }
  restore(): void {
    const state = this.states.pop();
    if (state === undefined) throw new Error("Unbalanced Canvas restore");
    this.alpha = state.alpha;
    this.fontSize = state.fontSize;
  }
  setFont(value: string): void { this.fonts.push(value); this.fontSize = Number(value.match(/([\d.]+)px/)?.[1] ?? 1); }
  setGlobalAlpha(value: number): void { this.alpha = value; }
  rect(...dimensions: [number, number, number, number]): void { this.clips.push(dimensions); }
  setFillStyle(): void {}
  setStrokeStyle(): void {}
  setTextBaseline(): void {}
  setTextAlign(): void {}
  setLineWidth(): void {}
  strokeRect(): void {}
  beginPath(): void {}
  closePath(): void {}
  clip(): void {}
  fill(): void {}
  moveTo(): void {}
  lineTo(): void {}
  stroke(): void {}
}

function flightView(aspect = 1280 / 720): Extract<HeadHudView, { kind: "visible" }> {
  const values = new Array<number>(33).fill(0);
  values[4] = 8;
  values[7] = 1;
  values[19] = -1;
  values[20] = 12;
  values[21] = 8;
  values[22] = 9;
  values[23] = 2;
  values[24] = -1;
  values[31] = 1;
  const camera = new PerspectiveCamera(60, aspect, 0.05, 100);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  const model = createFlightHudModel(parseFlightSnapshot(values), 4, createInitialAppModel().difficulty.hudProfile);
  const view = createHeadHudView(model, captureConfiguredViewerFrame(camera, stereo));
  if (view.kind !== "visible") throw new Error("Expected visible fixture");
  return view;
}

describe("Head HUD Canvas preflight and painting", () => {
  it.each([1280 / 720, 720 / 1280])("preserves physical aspect and glyph height at aspect %s", (aspect) => {
    const view = flightView(aspect);
    const size = headHudCanvasSize(view.layer);
    expect(Math.abs(size.width / size.height - view.layer.size.width / view.layer.size.height)).toBeLessThan(1 / size.height);
    const context = new RecordingHeadContext();
    const preparation = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(preparation.kind).toBe("ready");
    expect(context.clears).toHaveLength(0);
    expect(context.fills).toHaveLength(0);
    expect(context.texts).toHaveLength(0);
    if (preparation.kind !== "ready") throw new Error("Missing ready plan");
    expect(Object.isFrozen(preparation.cards)).toBe(true);
    expect(preparation.fontSize * view.layer.size.height / size.height).toBeCloseTo(view.textHeightMeters, 12);
    drawHeadHud(context, preparation);
    expect(context.clears).toEqual([[0, 0, size.width, size.height]]);
    expect(context.fills).toHaveLength(view.layer.elements.length);
    expect(context.fills.every((fill) => fill.alpha === 0.28)).toBe(true);
    expect(context.texts.every((text) => text.alpha === 1 && text.arguments === 3)).toBe(true);
    expect(context.fonts.at(-1)).toBe(context.fonts[0]);
    for (const fill of context.fills) {
      const clear = view.layer.clearRegion;
      expect(fill.left + fill.width <= clear.left * size.width || fill.left >= (clear.left + clear.width) * size.width ||
        fill.top + fill.height <= clear.top * size.height || fill.top >= (clear.top + clear.height) * size.height).toBe(true);
    }
    const wind = preparation.cards.find((card) => card.element.id === "head-wind");
    if (wind?.element.kind !== "wind") throw new Error("Missing wind card");
    expect(wind.value.map((line) => line.value).join("")).toBe(wind.element.value);
  });

  it.each(["invalidMetrics", "excessiveInk"] as const)("rejects %s before painting any card", (field) => {
    const context = new RecordingHeadContext();
    context[field] = true;
    const view = flightView();
    const size = headHudCanvasSize(view.layer);
    const preparation = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(preparation).toMatchObject({ kind: "unavailable", reason: "text-overflow" });
    drawHeadHud(context, preparation);
    expect(context.clears).toHaveLength(1);
    expect(context.fills).toHaveLength(0);
    expect(context.texts).toHaveLength(0);
  });

  it("clears absent and geometry-unavailable frames without reusing the preceding paint plan", () => {
    const context = new RecordingHeadContext();
    for (const view of [{ kind: "absent" }, { kind: "unavailable", reason: "viewer-unavailable" }] as const) {
      const preparation = prepareHeadHudPaint(context, view, 1024, 768);
      expect(preparation.kind).toBe(view.kind);
      drawHeadHud(context, preparation);
    }
    expect(context.clears).toEqual([[0, 0, 1024, 768], [0, 0, 1024, 768]]);
    expect(context.fills).toHaveLength(0);
  });
});
