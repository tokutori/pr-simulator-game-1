import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createFlightFrameViewDraft, failFlightMenuFrame, finalizeFlightFrameView, flightMenuFailureRecovery } from "../../web/src/app/flight-frame-view.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { headHudCanvasSize, prepareHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { Window } from "happy-dom";
import { FlightController } from "../../web/src/game/flight-controller.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";

function flightModel(mode: "screen" | "phone-vr" | "webxr", phase = 5): AppModel {
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
  const session = gameSessionState(phase, 0, parseFlightSnapshot(values), true);
  if (session === null) throw new Error("Missing session fixture");
  return { ...createInitialAppModel(), presentation: { type: "ready", mode }, gameSession: session };
}

function configuredViewer() {
  const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 100_000);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  camera.updateMatrixWorld(true);
  return captureConfiguredViewerFrame(camera, stereo);
}

describe("Immutable same-frame Flight view finalization", () => {
  it("preserves the Screen controller failure status through current and later Runtime view frames", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const evidence = { loop: null as ((timestamp: number) => void) | null, frames: 0 };
    const renderer: RendererAdapter = {
      startLoop(callback) { evidence.loop = (timestamp) => { callback(timestamp, unavailableViewerFrame("not-stereo")); }; },
      beginViewFrame() {}, stopLoop() { evidence.loop = null; }, render() { evidence.frames++; },
      setFlightPose() {}, setLakeVisualCondition() {}, setFlightCameraMode() {}, setCinematicCameraView() {},
      transformTrackingPose: (value) => value, resize() {}, setStereoPresentation() {}, setSelectRayHandler() {}, dispose() {}
    };
    const packed = new Array<number>(33).fill(0); packed[7] = 1; packed[19] = -1;
    const controller = new FlightController({ snapshot: () => packed, advance_tick: () => { throw new Error("Rust tick failed"); }, free() {} }, {
      readIntent: () => ({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 }), reset() {}, suspend() {}, resume() {}, dispose() {}
    }, renderer, hud, 100, () => []);
    const model = flightModel("screen");
    const runtime = new PresentationRuntime(renderer, [new ScreenPresentationBackend(() => ({ x: 1280, y: 720, pixelRatio: 1 }))],
      (viewer) => { const draft = createFlightFrameViewDraft(model, controller.currentSnapshot, viewer); return finalizeFlightFrameView(draft, draft.headHud); },
      (timestamp) => { controller.onFrame(timestamp); });
    try {
      expect(await runtime.start("screen")).toEqual({ ok: true });
      if (evidence.loop === null) throw new Error("Missing Screen loop");
      evidence.loop(0);
      evidence.loop(20);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe("飛行処理を停止した: Rust tick failed");
      evidence.loop(40);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe("飛行処理を停止した: Rust tick failed");
      expect(evidence.frames).toBe(3);
    } finally { controller.dispose(); await runtime.dispose(); await window.happyDOM.abort(); }
  });
  it("uses one captured model, snapshot and shared HUD through measurement and finalization", () => {
    let currentModel = flightModel("phone-vr");
    const captured = currentModel;
    const snapshot = captured.gameSession.kind === "flight" ? captured.gameSession.snapshot : null;
    const draft = createFlightFrameViewDraft(captured, snapshot, configuredViewer());
    expect(draft.model).toBe(captured);
    expect(draft.snapshot).toBe(snapshot);
    expect(draft.analysis).toBe(captured.flightAnalysis);
    expect(draft.headHud.kind).toBe("visible");
    if (draft.headHud.kind !== "visible") throw new Error("Missing Head layout");
    const size = headHudCanvasSize(draft.headHud.layer);
    const preparation = prepareHeadHudPaint(new HudCanvasFixture(), draft.headHud, size.width, size.height);
    expect(preparation.kind).toBe("ready");
    currentModel = flightModel("screen", 6);
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(currentModel).not.toBe(draft.model);
    expect(view.headHud).toBe(draft.headHud.layer);
    expect(view.activeOverlay).toBeNull();
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-flight-pause"]);
    expect(draft.hud?.readouts).toContain("ALT 12.0 m");
  });

  it.each(["invalid-view-geometry", "viewer-unavailable"] as const)("keeps %s local to the HUD and explains it in the same Menu", (reason) => {
    const model = flightModel("webxr");
    const snapshot = model.gameSession.kind === "flight" ? model.gameSession.snapshot : null;
    const before = JSON.stringify(model);
    const draft = createFlightFrameViewDraft(model, snapshot, unavailableViewerFrame(reason));
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(view.headHud.kind).toBe("absent");
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-head-hud-unavailable", "game-flight-pause"]);
    expect(JSON.stringify(model)).toBe(before);
    expect(model.presentation).toEqual({ type: "ready", mode: "webxr" });
  });

  it("keeps a normal Pause action and same-frame explanation when Head text cannot fit", () => {
    const model = flightModel("phone-vr");
    const draft = createFlightFrameViewDraft(model, model.gameSession.kind === "flight" ? model.gameSession.snapshot : null, configuredViewer());
    const view = finalizeFlightFrameView(draft, { kind: "unavailable", reason: "text-overflow" });
    expect(view.headHud).toEqual({ kind: "absent" });
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-head-hud-unavailable", "game-flight-pause"]);
    expect(model.presentation).toEqual({ type: "ready", mode: "phone-vr" });
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("keeps Pause Head absent in %s", (mode) => {
    const model = flightModel(mode, 6);
    const snapshot = model.gameSession.kind === "paused-flight" ? model.gameSession.snapshot : null;
    const draft = createFlightFrameViewDraft(model, snapshot, configuredViewer());
    expect(draft.headHud).toEqual({ kind: "absent" });
    expect(finalizeFlightFrameView(draft, draft.headHud).headHud).toEqual({ kind: "absent" });
  });

  it.each(["phone-vr", "webxr"] as const)("hands off a failed %s Menu only after invalidating both surfaces", (mode) => {
    const model = flightModel(mode);
    const draft = createFlightFrameViewDraft(model, model.gameSession.kind === "flight" ? model.gameSession.snapshot : null, configuredViewer());
    const failure = failFlightMenuFrame(draft, finalizeFlightFrameView(draft, draft.headHud), "text-overflow");
    expect(failure.viewModel.panels).toEqual([]);
    expect(failure.viewModel.headHud).toEqual({ kind: "absent" });
    expect(model.presentation).toEqual({ type: "ready", mode });
    const focusedModel = updateApp(model, { type: "ui-action", action: { type: "focus", controlId: null } }).model;
    const message = flightMenuFailureRecovery(focusedModel, failure);
    expect(message).toEqual({ type: "backend-ended", mode, message: "VR Flight Menu rendering failed: text-overflow" });
    if (message === null) throw new Error("Missing guarded recovery");
    const recovery = updateApp(focusedModel, message);
    expect(recovery.model.presentation.type).toBe("transitioning");
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: model.nextRequestId }]);
    expect(flightMenuFailureRecovery({ ...model, nextRequestId: model.nextRequestId + 1 }, failure)).toBeNull();
    expect(flightMenuFailureRecovery({ ...model, presentation: { type: "ready", mode } }, failure)).toBeNull();
    expect(flightMenuFailureRecovery({ ...model, presentation: { type: "ready", mode: "screen" } }, failure)).toBeNull();
  });
});
