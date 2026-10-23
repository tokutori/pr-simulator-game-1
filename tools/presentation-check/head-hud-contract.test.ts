import { describe, expect, it } from "vitest";
import { NO_HEAD_HUD, resolveHeadHudFrame } from "../../web/src/render/contracts/head-hud.js";
import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";
import { createHeadHudFixture, createSceneFixture } from "./ui-adapter-fixture.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

describe("Non-interactive Head HUD adapter", () => {
  it("keeps HUD painting separate from interactive menu actions", () => {
    const view = { ...createSceneFixture("Flight"), headHud: createHeadHudFixture() };
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-status" })).toBe(false);
    expect(viewExposesAction(view, { type: "activate", controlId: "flight-action" })).toBe(true);
  });

  it("attaches the surface only when a raw center-head pose is available", () => {
    const hud = createHeadHudFixture();
    const head = pose(vec3(0.01, 0.02, 0.03), IDENTITY_POSE.orientation);
    expect(resolveHeadHudFrame(NO_HEAD_HUD, head)).toBe(NO_HEAD_HUD);
    expect(resolveHeadHudFrame(hud, null)).toBe(NO_HEAD_HUD);
    expect(resolveHeadHudFrame(hud, head)).toEqual({ kind: "visible", trackingFromHead: head, view: hud });
  });

  it("keeps Screen HUD ownership in the DOM adapter", async () => {
    const backend = new ScreenPresentationBackend(() => ({ x: 1280, y: 720, pixelRatio: 1 }));
    await backend.start();
    try { expect(backend.currentFrame(100).headHud).toBe(NO_HEAD_HUD); }
    finally { await backend.stop(); }
  });
});
