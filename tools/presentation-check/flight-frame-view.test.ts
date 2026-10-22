import { referenceFrameFixture } from "./menu-fixture.js";
import { describe, expect, it } from "vitest";
import { fixturePresentation } from "./menu-fixture.js";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createFlightFrameViewDraft, createFlightUiHudModel, finalizeFlightFrameView, menuFrameFailureRecovery } from "../../web/src/app/flight-frame-view.js";
import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import type { SessionPhase } from "../../web/src/app/session-snapshot.js";
import { headHudCanvasSize, prepareHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { Window } from "happy-dom";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { launchCurrentSession } from "../game-check/current-session-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";

function flightModel(mode: "screen" | "phone-vr" | "webxr", phase: SessionPhase = "flight_running", headFailure: string | null = null,
  replayView?: "cinematic" | "telemetry" | "analysis"): AppModel {
  return withSharedGameUiSession(phase, { platform: { presentation_mode: mode === "phone-vr" ? "phone_vr" : mode === "webxr" ? "hmd" : mode,
    head_hud_failure: headFailure }, actions: replayView === undefined ? [] : [{ kind: "set_replay_view", view: replayView }] },
  (session, platform) => ({ ...createInitialAppModel(), presentation: { type: "ready", mode },
    gameSession: readRuntimeSessionProjection(session), sharedGameUi: session.readSharedUi(platform) }));
}

function configuredViewer() {
  const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 100_000);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  camera.updateMatrixWorld(true);
  return captureConfiguredViewerFrame(camera, stereo);
}

describe("Immutable same-frame Flight view finalization", () => {
  it.each(["phone-vr", "webxr"] as const)("keeps independent Head FPS and projects shared HUD observations in all %s scene phases", (mode) => {
    for (const phase of ["title", "flight_setup", "briefing_ready", "countdown", "flight_running", "flight_paused", "result", "briefing_failed", "replay", "attract"] as const) {
      const model = flightModel(mode, phase);
      const before = JSON.stringify(model);
      const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja", 60);
      if (draft.headHud.kind !== "visible") throw new Error("Missing Head FPS view");
      const view = finalizeFlightFrameView(draft, draft.headHud);
      expect(view.headHud).toBe(draft.headHud.layer);
      if (view.headHud.kind !== "visible") throw new Error("Missing finalized Head FPS layer");
      expect(view.headHud.elements.some((element) => element.id === "head-frame-rate")).toBe(true);
      if (model.sharedGameUi?.hud === null) expect(view.headHud.elements.map((element) => element.id)).toEqual(["head-frame-rate"]);
      expect(viewExposesAction(view, { type: "activate", controlId: "head-frame-rate" })).toBe(false);
      expect(JSON.stringify(model)).toBe(before);
    }
    const model = flightModel("screen", "title");
    const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja", 60);
    expect(draft.headHud.kind).toBe("absent");
    expect(finalizeFlightFrameView(draft, draft.headHud).headHud.kind).toBe("absent");
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("projects nullable shared %s HUD policy without requiring a physical snapshot", (mode) => {
    for (const phase of ["flight_running", "flight_paused", "result", "attract"] as const) {
      const model = flightModel(mode, phase);
      const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja");
      const expected = phase === "flight_running" || (mode === "screen" && phase === "flight_paused");
      expect(model.sharedGameUi?.hud !== null).toBe(expected);
      expect(createFlightUiHudModel(model) !== null).toBe(expected);
      expect(draft.hud !== null).toBe(expected);
      expect(draft.headHud.kind).toBe(expected && mode !== "screen" ? "visible" : "absent");
    }
    for (const replayView of ["cinematic", "telemetry", "analysis"] as const) {
      const model = flightModel(mode, "replay", null, replayView);
      const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja");
      const expected = mode === "screen" && replayView === "telemetry";
      expect(model.sharedGameUi?.hud !== null).toBe(expected);
      expect(draft.hud !== null).toBe(expected);
      expect(draft.headHud.kind).toBe("absent");
    }
    const model = { ...flightModel(mode), sharedGameUi: null };
    expect(createFlightUiHudModel(model)).toBeNull();
    expect(createFlightFrameViewDraft(model, configuredViewer(), "ja").hud).toBeNull();
  });

  it.each([
    { mode: "screen", phase: "result" },
    { mode: "phone-vr", phase: "flight_paused" },
    { mode: "webxr", phase: "flight_paused" }
  ] as const)("keeps controller reset, synchronization and current rendering valid with absent $mode phase $phase HUD", async ({ mode, phase }) => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const model = flightModel(mode, phase);
    expect(createFlightUiHudModel(model)).toBeNull();
    const retained = Array.from(root.childNodes);
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    launchCurrentSession(session);
    const failures: string[] = [];
    const controller = new TailFlightController(session.flightPort, {
      readGamepads: () => [], dispose() {}
    }, { setFlightPose() {} }, {
      render: () => { hud.render(createFlightUiHudModel(model)); },
      fail: (message) => { failures.push(message); },
      setVisible: (visible) => { hud.setVisible(visible); }
    });
    try {
      expect(() => { controller.reset(session.flightPort.readSnapshot()); }).not.toThrow();
      expect(() => { controller.synchronizeSnapshot(session.flightPort.readSnapshot()); }).not.toThrow();
      expect(() => { controller.renderCurrentSnapshot(); }).not.toThrow();
      expect(root.hidden).toBe(true);
      expect(Array.from(root.childNodes)).toEqual(retained);
      expect(failures).toEqual([]);
    } finally {
      controller.dispose();
      session.dispose();
      await window.happyDOM.abort();
    }
  });

  it("recovers invalid Menu geometry only for the same model and presentation", () => {
    const frameModel = flightModel("phone-vr", "flight_paused");
    expect(menuFrameFailureRecovery(frameModel, frameModel, "phone-vr", "invalid-view-geometry"))
      .toEqual({ type: "backend-ended", mode: "phone-vr", message: "VR Menu rendering failed: invalid-view-geometry" });
    expect(menuFrameFailureRecovery({ ...frameModel }, frameModel, "phone-vr", "invalid-view-geometry")).toBeNull();
    expect(menuFrameFailureRecovery(frameModel, frameModel, "webxr", "invalid-view-geometry")).toBeNull();
    expect(menuFrameFailureRecovery(frameModel, frameModel, "phone-vr", "viewer-unavailable")).toBeNull();
    const screen = flightModel("screen", "flight_paused");
    expect(menuFrameFailureRecovery(screen, screen, "phone-vr", "invalid-view-geometry")).toBeNull();
  });

  it("preserves the Screen controller failure status through current and later Runtime view frames", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const evidence = { loop: null as ((timestamp: number) => void) | null, frames: 0 };
    const renderer: RendererAdapter = {
      startLoop(callback) { evidence.loop = (timestamp) => { callback(timestamp, unavailableViewerFrame("not-stereo")); }; },
      beginViewFrame() {}, stopLoop() { evidence.loop = null; }, render() { evidence.frames++; },
      setFlightPose() {}, setPreparedFlightPose() {}, setLakeVisualCondition() {}, setLakeSkyCondition() {}, setLakeVenueVisible() {}, setFlightCameraMode() {}, setCinematicCameraView() {},
      setLakeWaterQuality: () => Promise.resolve({ kind: "complete" }),
      projectTrackingFrame: (value) => referenceFrameFixture(value), resize() {}, setStereoPresentation() {}, setScreenRenderScale() {}, setScreenRenderAuto() {}, setSelectRayHandler() {}, dispose() {}
    };
    initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    launchCurrentSession(session);
    let model = flightModel("screen");
    const controller = new TailFlightController({ ...session.flightPort,
      advanceFrame: () => { throw new Error("Rust frame failed"); } }, {
      readGamepads: () => [], dispose() {}
    }, renderer, { render: () => { hud.render(createFlightUiHudModel(model)); },
      fail: (message) => {
        model = { ...model, sharedGameUi: currentSharedGameUiFixture("flight_running", { platform: { flight_controller_failure: message } }) };
        hud.render(createFlightUiHudModel(model));
      }, setVisible: (visible) => { hud.setVisible(visible); } });
    const runtime = new PresentationRuntime(renderer, [new ScreenPresentationBackend(() => ({ x: 1280, y: 720, pixelRatio: 1 }))],
      (viewer) => { const draft = createFlightFrameViewDraft(model, viewer, "ja"); return fixturePresentation(finalizeFlightFrameView(draft, draft.headHud)); },
      (timestamp) => { controller.onFrame(timestamp); });
    try {
      expect(await runtime.start("screen")).toEqual({ ok: true });
      if (evidence.loop === null) throw new Error("Missing Screen loop");
      evidence.loop(0);
      evidence.loop(20);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe("停止理由: Rust frame failed");
      evidence.loop(40);
      expect(root.querySelector('output[aria-live="polite"]')?.textContent).toBe("停止理由: Rust frame failed");
      expect(evidence.frames).toBe(3);
    } finally { controller.dispose(); session.dispose(); await runtime.dispose(); await window.happyDOM.abort(); }
  });
  it("uses one captured model and shared HUD through measurement and finalization", () => {
    let currentModel = flightModel("phone-vr");
    const captured = currentModel;
    const draft = createFlightFrameViewDraft(captured, configuredViewer(), "ja");
    expect(draft.model).toBe(captured);
    expect(draft.headHud.kind).toBe("visible");
    if (draft.headHud.kind !== "visible") throw new Error("Missing Head layout");
    const size = headHudCanvasSize(draft.headHud.layer);
    const preparation = prepareHeadHudPaint(new HudCanvasFixture(), draft.headHud, size.width, size.height);
    expect(preparation.kind).toBe("ready");
    currentModel = flightModel("screen", "flight_paused");
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(currentModel).not.toBe(draft.model);
    expect(view.headHud).toBe(draft.headHud.layer);
    expect(view.activeOverlay).toBeNull();
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-flight-pause"]);
    expect(draft.hud?.groups).toBe(captured.sharedGameUi?.hud?.groups);
  });

  it.each(["invalid-view-geometry", "viewer-unavailable"] as const)("keeps %s local to the HUD and explains it in the same Menu", (reason) => {
    const model = flightModel("webxr", "flight_running", reason);
    const before = JSON.stringify(model);
    const draft = createFlightFrameViewDraft(model, unavailableViewerFrame(reason), "ja");
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(view.headHud.kind).toBe("absent");
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-flight-pause", "game-head-hud-unavailable"]);
    const status = view.panels[0]?.controls.find((control) => control.id === "game-head-hud-unavailable");
    if (status?.kind !== "status") throw new Error("Missing Head HUD failure explanation");
    expect(status.value).toContain(reason);
    expect(JSON.stringify(model)).toBe(before);
    expect(model.presentation).toEqual({ type: "ready", mode: "webxr" });
  });

  it("keeps a normal Pause action and same-frame explanation when Head text cannot fit", () => {
    const model = flightModel("phone-vr", "flight_running", "text-overflow");
    const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja");
    const view = finalizeFlightFrameView(draft, { kind: "unavailable", reason: "text-overflow" });
    expect(view.headHud).toEqual({ kind: "absent" });
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-flight-pause", "game-head-hud-unavailable"]);
    const status = view.panels[0]?.controls.find((control) => control.id === "game-head-hud-unavailable");
    if (status?.kind !== "status") throw new Error("Missing Head HUD failure explanation");
    expect(status.value).toContain("text-overflow");
    expect(model.presentation).toEqual({ type: "ready", mode: "phone-vr" });
  });

  it("keeps telemetry noninteractive and hides it outside active VR Flight", () => {
    const model = flightModel("phone-vr");
    const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja");
    if (draft.headHud.kind !== "visible") throw new Error("Missing Head view");
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(view.headHud).toBe(draft.headHud.layer);
    expect(view.panels[0]?.anchor).toBe("menu");
    expect(viewExposesAction(view, { type: "activate", controlId: "head-attitude" })).toBe(false);
    const absent = finalizeFlightFrameView(draft, { kind: "absent" });
    expect(absent.panels[0]?.controls.some((control) => control.id === "game-head-hud-unavailable")).toBe(false);
    const screen = flightModel("screen");
    if (screen.sharedGameUi === null) throw new Error("Missing shared Screen view");
    expect(createGameViewModel(screen.sharedGameUi).headHud).toEqual({ kind: "absent" });
    for (const phase of ["title", "flight_setup", "briefing_ready", "countdown", "flight_paused", "result", "briefing_failed", "attract"] as const) {
      const phaseModel = flightModel("phone-vr", phase);
      if (phaseModel.sharedGameUi === null) throw new Error("Missing shared phase view");
      const projected = createGameViewModel(phaseModel.sharedGameUi);
      expect(projected.headHud).toEqual({ kind: "absent" });
      expect(projected.panels[0]?.anchor).toBe("menu");
    }
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("keeps Pause Head absent in %s", (mode) => {
    const model = flightModel(mode, "flight_paused");
    const draft = createFlightFrameViewDraft(model, configuredViewer(), "ja");
    expect(draft.headHud).toEqual({ kind: "absent" });
    expect(finalizeFlightFrameView(draft, draft.headHud).headHud).toEqual({ kind: "absent" });
  });

  it.each(["phone-vr", "webxr"] as const)("returns a failed %s Menu to Screen through the current guarded recovery", (mode) => {
    const model = flightModel(mode);
    expect(model.presentation).toEqual({ type: "ready", mode });
    const focusedModel = updateApp(model, { type: "ui-action", action: { type: "focus", controlId: null } }).model;
    const message = menuFrameFailureRecovery(focusedModel, focusedModel, mode, "context-unavailable");
    expect(message).toEqual({ type: "backend-ended", mode, message: "VR Menu rendering failed: context-unavailable" });
    if (message === null) throw new Error("Missing guarded recovery");
    const recovery = updateApp(focusedModel, message);
    expect(recovery.model.presentation.type).toBe("transitioning");
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: model.nextRequestId }]);
    expect(menuFrameFailureRecovery({ ...focusedModel, nextRequestId: focusedModel.nextRequestId + 1 }, focusedModel, mode, "context-unavailable")).toBeNull();
    expect(menuFrameFailureRecovery({ ...focusedModel, presentation: { type: "ready", mode } }, focusedModel, mode, "context-unavailable")).toBeNull();
    expect(menuFrameFailureRecovery({ ...focusedModel, presentation: { type: "ready", mode: "screen" } }, focusedModel, mode, "context-unavailable")).toBeNull();
  });
});
