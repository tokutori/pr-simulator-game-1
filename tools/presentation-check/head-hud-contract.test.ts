import { describe, expect, expectTypeOf, it } from "vitest";
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
    expectTypeOf<HeadHudElement>().not.toHaveProperty("ratio");
    const hud = createHeadHudFixture();
    const view = { ...createSceneFixture("Flight"), headHud: hud };
    expect(() => { validateUiViewModel(view); }).not.toThrow();
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-status" })).toBe(false);
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-action" })).toBe(true);
  });

  it("resolves only a visible view with an available raw center-head pose", () => {
    const hud = createHeadHudFixture();
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
    const hud = createHeadHudFixture();
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
    const hud = createHeadHudFixture();
    const element = hud.elements[0];
    if (element === undefined) throw new Error("Missing HUD element fixture");
    expect(() => { validateHeadHudLayer({ ...hud, elements: [{ ...element, bounds: hud.clearRegion }] }); }).toThrow(/clear region/);
    expect(() => { validateHeadHudLayer({ ...hud, elements: [{ ...element, bounds: { left: 0, top: -0.1, width: 0.1, height: 0.1 } }] }); }).toThrow(RangeError);
    expect(() => { validateHeadHudLayer({ ...hud, elements: [element, element] }); }).toThrow(/Duplicate/);
  });

  it("rejects non-finite instrument values without conflating instrument states", () => {
    const hud = createHeadHudFixture();
    const bounds = { left: 0, top: 0.7, width: 0.1, height: 0.1 };
    const invalid: readonly HeadHudElement[] = [
      { id: "attitude", label: "ADI", bounds, kind: "attitude", rollDegrees: Number.NaN, pitchDegrees: 0, flightPathAngleDegrees: null },
      { id: "path", label: "ADI", bounds, kind: "attitude", rollDegrees: 0, pitchDegrees: 0, flightPathAngleDegrees: Number.NaN },
      { id: "heading", label: "HDG", bounds, kind: "heading", degrees: Number.NaN },
      { id: "wind", label: "WIND", bounds, kind: "wind", degrees: Number.NaN, value: "invalid" },
      { id: "aoa", label: "AoA", bounds, kind: "angle-of-attack", degrees: Number.NaN, value: "invalid" },
      { id: "path", label: "PATH", bounds, kind: "flight-path", degrees: Number.NaN, value: "invalid" }
    ];
    for (const element of invalid) expect(() => { validateHeadHudLayer({ ...hud, elements: [element] }); }).toThrow(RangeError);
  });
});
