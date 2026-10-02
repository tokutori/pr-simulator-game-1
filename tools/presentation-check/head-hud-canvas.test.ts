import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, gameSessionState } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { createFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { drawHeadHud, headHudCanvasSize, headHudPaintedTextInk, prepareHeadHudPaint, validateHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import type { HeadHudDrawingContext, HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import { createHeadHudView } from "../../web/src/presentation/head-hud-view.js";
import type { HeadHudView } from "../../web/src/presentation/head-hud-view.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { projectHeadPoint } from "../../web/src/render/contracts/viewer-frame.js";
import { composePose, IDENTITY_POSE, pose, transformPoint, vec3 } from "../../web/src/render/contracts/math.js";
import { FLIGHT_MENU_GEOMETRY } from "../../web/src/render/contracts/ui.js";
import { convexQuadsOverlap } from "./hud-canvas-fixture.js";

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
  tinyInk = false;

  clearRect(...dimensions: [number, number, number, number]): void { this.clears.push(dimensions); }
  fillRect(left: number, top: number, width: number, height: number): void { this.fills.push({ alpha: this.alpha, left, top, width, height }); }
  fillText(...args: [string, number, number]): void { this.texts.push({ value: args[0], alpha: this.alpha, arguments: args.length }); }
  measureText(value: string): HeadHudTextMetrics {
    const width = Array.from(value).reduce((sum, character) => sum + this.fontSize * (character.charCodeAt(0) > 255 ? 1 : 0.65), 0);
    return { width, left: this.excessiveInk ? width * 10 : 0, right: width,
      ascent: this.invalidMetrics ? Number.NaN : this.fontSize * (this.tinyInk ? 0.1 : 0.7), descent: this.fontSize * (this.tinyInk ? 0.05 : 0.2) };
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
  it.each([1280 / 720, 720 / 1280].flatMap((aspect) => [0, 4].map((code) => ({ aspect, code }))))("fits all cues with envelope warning and long values for Information $code at aspect $aspect", ({ aspect, code }) => {
    const values = new Array<number>(33).fill(0);
    values[4] = 8; values[7] = 1; values[16] = 3; values[17] = 1234.5; values[19] = -1;
    values[20] = 123.4; values[21] = 12.3; values[22] = 23.4; values[23] = -12.3;
    values[24] = 5.6; values[25] = -8.9; values[31] = 1; values[32] = 123.4;
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    const model = createFlightHudModel(parseFlightSnapshot(values), code as 0 | 4, createInitialAppModel().difficulty.hudProfile);
    const view = createHeadHudView(model, viewer);
    if (view.kind !== "visible") throw new Error("Missing warning-and-long-values layout");
    expect(view.layer.elements.some((element) => element.id === "head-warning")).toBe(true);
    const size = headHudCanvasSize(view.layer);
    expect(validateHeadHudPaint(prepareHeadHudPaint(new RecordingHeadContext(), view, size.width, size.height), viewer).kind).toBe("ready");
  });
  it.each([1280 / 720, 720 / 1280].flatMap((aspect) => [0, 1, 2, 3, 4].map((code) => ({ aspect, code }))))("fits Information $code with nominal Menu and measured ink at aspect $aspect", ({ aspect, code }) => {
    const values = new Array<number>(33).fill(0); values[7] = 1; values[19] = -1; values[20] = 12; values[21] = 8; values[22] = 9;
    values[23] = 2; values[24] = -1; values[31] = 1;
    const snapshot = parseFlightSnapshot(values);
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    if (viewer.source === "unavailable") throw new Error("Missing binocular fixture");
    const hudModel = createFlightHudModel(snapshot, code as 0 | 1 | 2 | 3 | 4, createInitialAppModel().difficulty.hudProfile);
    const view = createHeadHudView(hudModel, viewer);
    if (view.kind !== "visible") throw new Error("Missing Information layout");
    const size = headHudCanvasSize(view.layer);
    expect(validateHeadHudPaint(prepareHeadHudPaint(new RecordingHeadContext(), view, size.width, size.height), viewer).kind).toBe("ready");
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing Flight session");
    const menu = createGameViewModel({ ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode: "phone-vr" } }, snapshot, null, view).panels[0];
    if (menu === undefined) throw new Error("Missing small Menu");
    const menuPose = composePose(pose(vec3(0, 0, -FLIGHT_MENU_GEOMETRY.distanceMeters), IDENTITY_POSE.orientation), menu.localPose);
    const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const;
    for (const eye of viewer.eyes) {
      const menuProjection = corners.map(([horizontal, vertical]) => projectHeadPoint(eye,
        transformPoint(menuPose, vec3(horizontal * menu.size.width / 2, vertical * menu.size.height / 2, 0))));
      if (menuProjection.some((point) => point === null)) throw new Error("Missing Menu projection");
      for (const element of view.layer.elements) {
        const bounds = element.bounds;
        const cardProjection = corners.map(([horizontal, vertical]) => projectHeadPoint(eye, transformPoint(view.layer.localPose,
          vec3((bounds.left + (horizontal + 1) * bounds.width / 2 - 0.5) * view.layer.size.width,
            (0.5 - bounds.top - (1 - vertical) * bounds.height / 2) * view.layer.size.height, 0))));
        if (cardProjection.some((point) => point === null)) throw new Error("Missing card projection");
        expect(convexQuadsOverlap(cardProjection.filter((point) => point !== null), menuProjection.filter((point) => point !== null))).toBe(false);
      }
    }
  });

  it.each([1280 / 720, 720 / 1280].flatMap((aspect) => Array.from({ length: 64 }, (_, mask) => ({ aspect, mask }))))("preflights independent Custom mask $mask at aspect $aspect", ({ aspect, mask }) => {
    const values = new Array<number>(33).fill(0); values[4] = 8; values[7] = 1; values[19] = -1; values[20] = 12; values[21] = 8; values[22] = 9;
    values[23] = 2; values[24] = -1; values[31] = 1;
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    const custom = { telemetry: (mask & 1) !== 0, attitude: (mask & 2) !== 0, wind: (mask & 4) !== 0,
      flightPath: (mask & 8) !== 0, angleOfAttack: (mask & 16) !== 0, warnings: (mask & 32) !== 0 };
    const view = createHeadHudView(createFlightHudModel(parseFlightSnapshot(values), 4, custom), viewer);
    if (mask === 0 || mask === 32) { expect(view.kind).toBe("absent"); return; }
    if (view.kind !== "visible") throw new Error("Missing Custom layout");
    const size = headHudCanvasSize(view.layer);
    expect(validateHeadHudPaint(prepareHeadHudPaint(new RecordingHeadContext(), view, size.width, size.height), viewer).kind).toBe("ready");
  });
  it("rejects small measured ink before painting even if font em and wrapping fit", () => {
    const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 100);
    camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    const view = flightView();
    const size = headHudCanvasSize(view.layer);
    const context = new RecordingHeadContext();
    const ready = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(validateHeadHudPaint(ready, viewer).kind).toBe("ready");
    expect(headHudPaintedTextInk(ready).length).toBeGreaterThan(0);
    context.tinyInk = true;
    const small = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(small.kind).toBe("ready");
    const rejected = validateHeadHudPaint(small, viewer);
    expect(rejected.kind).toBe("unavailable");
    drawHeadHud(context, rejected);
    expect(context.texts).toEqual([]);
  });
  it.each([1280 / 720, 720 / 1280])("preserves physical aspect and font-em height at aspect %s", (aspect) => {
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
