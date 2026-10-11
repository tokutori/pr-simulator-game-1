import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel } from "../../web/src/app/app-state.js";
import { currentFlightDisplayFixture } from "../game-check/current-session-fixture.js";
import { createFlightDisplayHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { browserHeadHudContext, drawHeadHud, headHudCanvasSize, headHudPaintedTextInk, prepareHeadHudPaint, validateHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import type { HeadHudDrawingContext, HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import { createHeadHudView } from "../../web/src/presentation/head-hud-view.js";
import type { HeadHudView } from "../../web/src/presentation/head-hud-view.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { projectHeadPoint } from "../../web/src/render/contracts/viewer-frame.js";
import { IDENTITY_POSE, pose, transformPoint, vec3 } from "../../web/src/render/contracts/math.js";
import { FLIGHT_MENU_GEOMETRY } from "../../web/src/render/contracts/ui.js";
import { convexQuadsOverlap } from "./hud-canvas-fixture.js";

class RecordingHeadContext implements HeadHudDrawingContext {
  readonly locales: string[] = [];
  negativeAscent = false;
  negativeDescent = false;
  readonly typography: { kind: "measure" | "draw"; locale: string; font: string; baseline: string; align: string }[] = [];
  private locale = "";
  private font = "";
  private baseline = "";
  private align = "";
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
  fillText(...args: [string, number, number]): void {
    this.typography.push({ kind: "draw", locale: this.locale, font: this.font, baseline: this.baseline, align: this.align });
    this.texts.push({ value: args[0], alpha: this.alpha, arguments: args.length });
  }
  measureText(value: string): HeadHudTextMetrics {
    this.typography.push({ kind: "measure", locale: this.locale, font: this.font, baseline: this.baseline, align: this.align });
    const width = Array.from(value).reduce((sum, character) => sum + this.fontSize * (character.charCodeAt(0) > 255 ? 1 : 0.65), 0);
    return { width, left: this.excessiveInk ? width * 10 : 0, right: width,
      ascent: this.invalidMetrics ? Number.NaN : this.fontSize * (this.tinyInk ? 0.1 : this.negativeAscent ? -0.1 : this.negativeDescent ? 1 : 0.7),
      descent: this.fontSize * (this.tinyInk ? 0.05 : this.negativeAscent ? 1 : this.negativeDescent ? -0.1 : 0.2) };
  }
  save(): void { this.states.push({ alpha: this.alpha, fontSize: this.fontSize }); }
  restore(): void {
    const state = this.states.pop();
    if (state === undefined) throw new Error("Unbalanced Canvas restore");
    this.alpha = state.alpha;
    this.fontSize = state.fontSize;
  }
  setFont(value: string): void { this.fonts.push(value); this.font = value; this.fontSize = Number(value.match(/([\d.]+)px/)?.[1] ?? 1); }
  setGlobalAlpha(value: number): void { this.alpha = value; }
  rect(...dimensions: [number, number, number, number]): void { this.clips.push(dimensions); }
  setFillStyle(): void {}
  setLocale(locale: string): void { this.locales.push(locale); this.locale = locale; }
  setStrokeStyle(): void {}
  setTextBaseline(value: "top" | "middle"): void { this.baseline = value; }
  setTextAlign(value: "left"): void { this.align = value; }
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
  const valuesSnapshot = {
    ...currentFlightDisplayFixture(5),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 8, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 2, east: -1, down: 0 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
  const camera = new PerspectiveCamera(60, aspect, 0.05, 100);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  const model = createFlightDisplayHudModel(valuesSnapshot, 4, createInitialAppModel().difficulty.hudProfile);
  const view = createHeadHudView(model, captureConfiguredViewerFrame(camera, stereo), "ja");
  if (view.kind !== "visible") throw new Error("Expected visible fixture");
  return view;
}

describe("Head HUD Canvas preflight and painting", () => {
  it.each([true, false])("sets detached canvas language with context language support %s", (contextLanguageSupported) => {
    const canvas = { lang: "" };
    const context = contextLanguageSupported ? { canvas, lang: "inherit" } : { canvas };
    const adapter = browserHeadHudContext(context as unknown as CanvasRenderingContext2D);
    adapter.setLocale("ja");
    expect(canvas.lang).toBe("ja");
    if ("lang" in context) expect(context.lang).toBe("ja");
    else expect(Object.hasOwn(context, "lang")).toBe(false);
  });

  it("uses the explicit view locale for measurement and painting", () => {
    const view = flightView();
    const size = headHudCanvasSize(view.layer);
    const context = new RecordingHeadContext();
    const preparation = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(context.locales).toEqual(["ja"]);
    drawHeadHud(context, preparation);
    expect(context.locales).toEqual(["ja", "ja"]);
    expect(context.fonts.at(-1)).toBe(context.fonts[0]);
    expect(context.typography.some((state) => state.kind === "measure")).toBe(true);
    expect(context.typography.some((state) => state.kind === "draw")).toBe(true);
    expect(context.typography.every((state) => state.locale === "ja" && state.font === context.fonts[0] && state.baseline === "middle" && state.align === "left")).toBe(true);
  });

  it.each(["negativeAscent", "negativeDescent"] as const)("preserves valid %s metrics and their actual ink bounds", (field) => {
    const view = flightView();
    const size = headHudCanvasSize(view.layer);
    const context = new RecordingHeadContext();
    context[field] = true;
    const preparation = prepareHeadHudPaint(context, view, size.width, size.height);
    expect(preparation.kind).toBe("ready");
    if (preparation.kind !== "ready") throw new Error("Signed ink plan missing");
    expect(preparation.cards.every((card) => [...card.label, ...card.value].every((line) => (field === "negativeAscent" ? line.metrics.ascent : line.metrics.descent) < 0))).toBe(true);
    expect(headHudPaintedTextInk(preparation).every((ink) => ink.height > 0 && (field === "negativeAscent" ? ink.top > ink.baseline : ink.top + ink.height < ink.baseline))).toBe(true);
    drawHeadHud(context, preparation);
    expect(context.texts.length).toBeGreaterThan(0);
  });

  it.each([1280 / 720, 720 / 1280].flatMap((aspect) => [0, 4].map((code) => ({ aspect, code }))))("fits all cues with envelope warning and long values for Information $code at aspect $aspect", ({ aspect, code }) => {
    const valuesSnapshot = {
    ...currentFlightDisplayFixture(7),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 8, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 123.4 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 123.4, airspeedMetersPerSecond: 12.3, groundspeedMetersPerSecond: 23.4,
      windVelocityNedMetersPerSecond: { north: -12.3, east: 5.6, down: -8.9 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } },
    finalization: { reason: "out_of_valid_envelope" as const, disposition: "failed" as const, terminalTick: 0, terminalFraction: 0, scoreMeters: [1234.5, 0, 1234.5] as const, failure: null }
  };
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    const model = createFlightDisplayHudModel(valuesSnapshot, code as 0 | 4, createInitialAppModel().difficulty.hudProfile);
    const view = createHeadHudView(model, viewer, "ja");
    if (view.kind !== "visible") throw new Error("Missing warning-and-long-values layout");
    expect(view.layer.elements.some((element) => element.id === "head-warning")).toBe(true);
    const size = headHudCanvasSize(view.layer);
    expect(validateHeadHudPaint(prepareHeadHudPaint(new RecordingHeadContext(), view, size.width, size.height), viewer).kind).toBe("ready");
  });
  it.each([1280 / 720, 720 / 1280].flatMap((aspect) => [0, 1, 2, 3, 4].map((code) => ({ aspect, code }))))("fits Information $code with nominal Menu and measured ink at aspect $aspect", ({ aspect, code }) => {
    const valuesSnapshot = {
    ...currentFlightDisplayFixture(5),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 0, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 2, east: -1, down: 0 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
    const snapshot = valuesSnapshot;
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    if (viewer.source === "unavailable") throw new Error("Missing binocular fixture");
    const hudModel = createFlightDisplayHudModel(snapshot, code as 0 | 1 | 2 | 3 | 4, createInitialAppModel().difficulty.hudProfile);
    const view = createHeadHudView(hudModel, viewer, "ja");
    if (view.kind !== "visible") throw new Error("Missing Information layout");
    const size = headHudCanvasSize(view.layer);
    expect(validateHeadHudPaint(prepareHeadHudPaint(new RecordingHeadContext(), view, size.width, size.height), viewer).kind).toBe("ready");
    const menuPose = pose(vec3(0, FLIGHT_MENU_GEOMETRY.centerY, -FLIGHT_MENU_GEOMETRY.distanceMeters), IDENTITY_POSE.orientation);
    const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const;
    for (const eye of viewer.eyes) {
      const menuProjection = corners.map(([horizontal, vertical]) => projectHeadPoint(eye,
        transformPoint(menuPose, vec3(horizontal * FLIGHT_MENU_GEOMETRY.width / 2, vertical * FLIGHT_MENU_GEOMETRY.height / 2, 0))));
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
    const valuesSnapshot = {
    ...currentFlightDisplayFixture(5),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 8, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 2, east: -1, down: 0 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
    const camera = new PerspectiveCamera(60, aspect, 0.05, 100); camera.updateMatrixWorld(true);
    const stereo = new StereoCamera(); stereo.aspect = 0.5;
    const viewer = captureConfiguredViewerFrame(camera, stereo);
    const custom = { telemetry: (mask & 1) !== 0, attitude: (mask & 2) !== 0, wind: (mask & 4) !== 0,
      flightPath: (mask & 8) !== 0, angleOfAttack: (mask & 16) !== 0, warnings: (mask & 32) !== 0 };
    const view = createHeadHudView(createFlightDisplayHudModel(valuesSnapshot, 4, custom), viewer, "ja");
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
