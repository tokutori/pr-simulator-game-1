import { describe, expect, expectTypeOf, it } from "vitest";
import { currentSharedGameUiFixture } from "../game-check/shared-game-ui-fixture.js";
import { NO_HEAD_HUD, resolveHeadHudFrame, validateHeadHudLayer } from "../../web/src/render/contracts/head-hud.js";
import type { HeadHudElement, VisibleHeadHud } from "../../web/src/render/contracts/head-hud.js";
import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { validateUiViewModel, viewExposesAction } from "../../web/src/render/contracts/ui.js";
import { createHeadHudFixture, createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

describe("Non-interactive Head HUD contract", () => {
  it("fixes the anchor and excludes controls and dispatch actions from the typed HUD", () => {
    expectTypeOf<VisibleHeadHud["anchor"]>().toEqualTypeOf<"head">();
    expectTypeOf<VisibleHeadHud>().not.toHaveProperty("controls");
    expectTypeOf<HeadHudElement>().not.toHaveProperty("action");
    expectTypeOf<Extract<HeadHudElement, { kind: "angle-of-attack" }>["ratio"]>().toEqualTypeOf<number>();
    const hud = createHeadHudFixture(currentSharedGameUiFixture().theme);
    const view = { ...createSceneFixture("Flight"), headHud: hud };
    expect(() => { validateUiViewModel(view); }).not.toThrow();
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-status" })).toBe(false);
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-action" })).toBe(true);
  });

  it("resolves only a visible view with an available raw center-head pose", () => {
    const hud = createHeadHudFixture(currentSharedGameUiFixture().theme);
    const head = pose(vec3(0.01, 0.02, 0.03), IDENTITY_POSE.orientation);
    expect(resolveHeadHudFrame(NO_HEAD_HUD, head)).toBe(NO_HEAD_HUD);
    expect(resolveHeadHudFrame(hud, null)).toBe(NO_HEAD_HUD);
    const frame = resolveHeadHudFrame(hud, head);
    expect(frame).toEqual({ kind: "visible", trackingFromHead: head, view: hud });
    expect(Object.isFrozen(frame)).toBe(true);
  });

  it("keeps Screen HUD ownership in the existing DOM adapter", async () => {
    const backend = new ScreenPresentationBackend(() => ({ x: 800, y: 600, pixelRatio: 1 }));
    await backend.start();
    try { expect(backend.currentFrame(100).headHud).toBe(NO_HEAD_HUD); }
    finally { await backend.stop(); }
  });

  it("validates independent background and foreground alpha and positive geometry", () => {
    const hud = createHeadHudFixture(currentSharedGameUiFixture().theme);
    expect(() => { validateHeadHudLayer(hud); }).not.toThrow();
    expect(hud.locale).toBe("ja");
    expect(() => { validateHeadHudLayer({ ...hud, locale: "" }); }).toThrow(RangeError);
    for (const invalid of [-0.1, 1.1, Number.NaN]) {
      expect(() => { validateHeadHudLayer({ ...hud, backgroundAlpha: invalid }); }).toThrow(RangeError);
      expect(() => { validateHeadHudLayer({ ...hud, foregroundAlpha: invalid }); }).toThrow(RangeError);
    }
    expect(() => { validateHeadHudLayer({ ...hud, size: { width: 0, height: 1 } }); }).toThrow(RangeError);
    expect(() => { validateHeadHudLayer({ ...hud, localPose: { position: vec3(0, 0, -1), orientation: { w: 2, x: 0, y: 0, z: 0 } } }); }).toThrow(RangeError);
  });

  it("rejects overlap with the declared clear region and invalid normalized bounds", () => {
    const hud = createHeadHudFixture(currentSharedGameUiFixture().theme);
    const element = hud.elements[0];
    if (element === undefined) throw new Error("Missing HUD element fixture");
    expect(() => { validateHeadHudLayer({ ...hud, elements: [{ ...element, bounds: hud.clearRegion }] }); }).toThrow(/clear region/);
    expect(() => { validateHeadHudLayer({ ...hud, elements: [{ ...element, bounds: { left: 0, top: -0.1, width: 0.1, height: 0.1 } }] }); }).toThrow(RangeError);
    expect(() => { validateHeadHudLayer({ ...hud, elements: [element, element] }); }).toThrow(/Duplicate/);
  });

  it("rejects non-finite instrument values without conflating instrument states", () => {
    const hud = createHeadHudFixture(currentSharedGameUiFixture().theme);
    const bounds = { left: 0, top: 0.7, width: 0.1, height: 0.1 };
    const geometry = { kind: "attitude" as const, horizon: { rotation_degrees: 0, pitch_offset_ratio: 0 }, flight_path_y_ratio: null };
    const invalid: readonly HeadHudElement[] = [
      { id: "attitude", label: "ADI", bounds, kind: "attitude", rollDegrees: Number.NaN, pitchDegrees: 0, flightPathAngleDegrees: null, geometry },
      { id: "path", label: "ADI", bounds, kind: "attitude", rollDegrees: 0, pitchDegrees: 0, flightPathAngleDegrees: Number.NaN, geometry },
      { id: "heading", label: "HDG", bounds, kind: "heading", degrees: Number.NaN, value: "invalid", ticks: [] },
      { id: "wind", label: "WIND", bounds, kind: "wind", degrees: Number.NaN, value: "invalid" },
      { id: "aoa", label: "AoA", bounds, kind: "angle-of-attack", degrees: Number.NaN, value: "invalid", ratio: 0.5 },
      { id: "path", label: "PATH", bounds, kind: "flight-path", degrees: Number.NaN, value: "invalid", yRatio: 0.5 },
      { id: "geometry", label: "ADI", bounds, kind: "attitude", rollDegrees: 0, pitchDegrees: 0, flightPathAngleDegrees: null,
        geometry: { ...geometry, horizon: { rotation_degrees: Number.NaN, pitch_offset_ratio: 0 } } },
      { id: "geometry", label: "HDG", bounds, kind: "heading", degrees: 0, value: "0°", ticks: [{ position_ratio: 1.1, major: true, label: "N" }] },
      { id: "geometry", label: "AoA", bounds, kind: "angle-of-attack", degrees: 5, value: "5", ratio: Number.NaN },
      { id: "geometry", label: "PATH", bounds, kind: "flight-path", degrees: 5, value: "5", yRatio: -0.1 }
    ];
    for (const element of invalid) expect(() => { validateHeadHudLayer({ ...hud, elements: [element] }); }).toThrow(RangeError);
  });
});
