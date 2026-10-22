import { afterEach, describe, expect, it, vi } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel } from "../../web/src/app/app-state.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createFlightFrameViewDraft, createFlightUiHudModel, finalizeFlightFrameView } from "../../web/src/app/flight-frame-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { readFileSync } from "node:fs";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { FlightGamepadObservation } from "../../web/src/generated/flight-gamepad.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { sharedGameUiPlatform, sharedHudNode } from "./shared-game-ui-fixture.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const disposals: (() => void)[] = [];
afterEach(() => { for (const dispose of disposals.splice(0)) dispose(); });

describe("Flight controller shared stop diagnosis", () => {
  it.each(["screen", "phone-vr", "webxr"] as const)("surfaces the real fail port in %s for every Information level", (mode) => {
    for (const informationCode of [0, 1, 2, 3, 4]) {
      const trial = fixture(mode, informationCode);
      const retained = trial.stop();
      const model = trial.model();
      expect(model.flightExecution).toMatchObject({ kind: "stopped", message: "Gamepad permission denied", snapshot: { frame: retained.frame, phase: "flight_paused" } });
      expect(model.gameSession.phase).toBe("flight_paused");
      expect(trial.controller.currentSnapshot.frame).toEqual(retained.frame);
      expect(trial.onTerminal).not.toHaveBeenCalled();
      const hud = createFlightUiHudModel(model);
      if (mode === "screen") {
        if (hud === null) throw new Error("Missing shared Screen Flight HUD");
        expect(sharedHudNode(hud, "hud-controller-failure").content.value).toContain("Gamepad permission denied");
        expect(sharedHudNode(hud, "hud-last-valid-state")).toMatchObject({ label: "最後の有効状態", content: { value: "0.01 s" } });
      } else expect(hud).toBeNull();
      const draft = createFlightFrameViewDraft(model, viewer(), "ja");
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing retained controller state");
      expect(draft.model.flightExecution).toBe(model.flightExecution);
      const view = finalizeFlightFrameView(draft, draft.headHud);
      expect(view.description).toContain("停止");
      const diagnosis = view.panels[0]?.controls.find((control) => control.id === "game-controller-stopped");
      if (diagnosis?.kind !== "status") throw new Error("Missing stop diagnosis");
      expect(diagnosis.value).toContain("Gamepad permission denied");
      expect(viewExposesAction(view, { type: "activate", controlId: "game-paused-abort" })).toBe(true);
      if (mode !== "screen") {
        expect(draft.headHud.kind).toBe("absent");
        expect(view.panels.flatMap(panel => panel.controls).some(control => control.id === "game-controller-stopped")).toBe(true);
      }
    }
  });

  it("retains the diagnosis through Pause, focus, backend changes and page restoration while rejecting Resume", () => {
    const trial = fixture("screen");
    trial.stop();
    const diagnosis = trial.model().flightExecution;
    trial.syncPhase("flight_paused");
    const stopped = trial.model();
    expect(updateApp(stopped, { type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" } })).toEqual({ model: stopped, effects: [] });
    trial.update({ type: "ui-action", action: { type: "focus", controlId: "game-flight-resume" } });
    trial.update({ type: "page-suspended" });
    trial.update({ type: "page-restored" });
    trial.update({ type: "game-session-status", message: "Backend restored" });
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      const model = trial.inMode(mode);
      expect(model.flightExecution).toBe(diagnosis);
      if (model.sharedGameUi === null) throw new Error("Missing shared UI observation");
      const view = createGameViewModel(model.sharedGameUi);
      expect(viewExposesAction(view, { type: "activate", controlId: "game-flight-resume" })).toBe(false);
      expect(viewExposesAction(view, { type: "activate", controlId: "game-paused-abort" })).toBe(true);
      expect(view.description).toContain("最後の有効");
    }
    const reads = trial.readGamepads.mock.calls.length;
    trial.controller.synchronizeSnapshot(trial.session.readSnapshot());
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(1_010);
    expect(trial.readGamepads).toHaveBeenCalledTimes(reads);
  });

  it("replaces the fail-port generation only after successful reset and rejects old callbacks", () => {
    const trial = fixture("phone-vr");
    trial.stop();
    const original = trial.binding;
    const replacement = trial.bind();
    const stopped = trial.model();
    expect(trial.model()).toBe(stopped);
    trial.controller.reset(trial.session.readSnapshot(), replacement.port);
    trial.update({ type: "flight-controller-ready", identity: replacement.identity });
    expect(trial.model().flightExecution.kind).toBe("ready");
    const recovered = trial.model();
    original.port.fail("Old controller callback");
    expect(trial.model()).toBe(recovered);
    trial.update({ type: "flight-controller-ready", identity: original.identity });
    expect(trial.model()).toBe(recovered);
    trial.controller.onFrame(100);
    trial.controller.onFrame(110);
    expect(trial.model().flightExecution).toMatchObject({ kind: "stopped", identity: replacement.identity });
  });

  it("rejects callbacks from a replaced session and malformed registration identities", () => {
    const trial = fixture("webxr");
    trial.stop();
    const session = { ...trial.session };
    const current = trial.bind(session);
    trial.update({ type: "flight-controller-ready", identity: current.identity });
    const ready = trial.model();
    trial.binding.port.fail("Disposed session callback");
    expect(trial.model()).toBe(ready);
    for (const identity of [
      { sessionId: 0, controllerId: 10 },
      { sessionId: 3, controllerId: Number.NaN },
      { sessionId: 1, controllerId: current.identity.controllerId + 1 }
    ]) {
      trial.update({ type: "flight-controller-ready", identity });
      expect(trial.model()).toBe(ready);
    }
  });

  it("retains the adapter diagnosis across Abort and Result until a new Flight reset succeeds", () => {
    const trial = fixture("phone-vr");
    const retained = trial.stop();
    const diagnosis = trial.model().flightExecution;
    trial.update({ type: "ui-action", action: { type: "activate", controlId: "game-paused-abort" } });
    expect(trial.effects()).toEqual([expect.objectContaining({ type: "shared-game-session-command", command: { kind: "abort" } })]);
    expect(retained.phase).toBe("flight_running");
    trial.facade.executeSharedSessionCommand({ kind: "abort" });
    trial.controller.reset(trial.session.readSnapshot());
    trial.update({ type: "session-command-completed", ...trial.facade.readGameSessionProjection() });
    expect(trial.model().gameSession.phase).toBe("result");
    expect(trial.model().flightExecution).toBe(diagnosis);
    expect(trial.controller.currentSnapshot.phase).toBe("result");
    trial.facade.executeSharedSessionCommand({ kind: "retry" });
    trial.facade.executeSharedSessionCommand({ kind: "start_countdown" });
    trial.facade.observeCountdown(trial.facade.captureCountdownToken(), 3, true);
    const replacement = trial.bind();
    trial.controller.reset(trial.session.readSnapshot(), replacement.port);
    trial.update({ type: "flight-controller-ready", identity: replacement.identity });
    trial.syncPhase("flight_running");
    expect(trial.model().flightExecution).toEqual({ kind: "ready", identity: replacement.identity });
    const hud = createFlightUiHudModel(trial.model());
    if (hud === null) throw new Error("Missing shared replacement Flight HUD");
    expect(hud.groups.flatMap(group => group.nodes).some(node => node.id === "hud-controller-failure" || node.id === "hud-last-valid-state")).toBe(false);
  });

  it("keeps normal terminal notification and Result persistence separate from adapter stops", () => {
    const trial = fixture("screen");
    trial.endNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.model().gameSession.phase).toBe("result");
    expect(trial.model().flightExecution.kind).toBe("ready");
    expect(trial.effects()).toEqual(expect.arrayContaining([expect.objectContaining({ type: "persist-flight-record" })]));
    const result = trial.model();
    trial.binding.port.fail("Late callback after Result");
    expect(trial.model().gameSession).toBe(result.gameSession);
    expect(trial.model().status).toContain("Late callback after Result");
  });
});

function fixture(mode: "screen" | "phone-vr" | "webxr", informationCode = 0) {
  const facade = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  facade.executeSharedSessionCommand({ kind: "open_setup" });
  const information = (["full", "standard", "minimal", "realistic", "custom"] as const)[informationCode];
  if (information === undefined) throw new RangeError("Unknown fixture information level");
  facade.executeSharedSessionCommand({ kind: "select_information", information });
  if (informationCode === 4) {
    for (const cue of ["telemetry", "attitude", "wind", "flight_path", "angle_of_attack", "warnings"] as const) {
      facade.executeSharedSessionCommand({ kind: "set_cue", cue, visible: false });
    }
  }
  facade.executeSharedSessionCommand({ kind: "prepare" });
  facade.executeSharedSessionCommand({ kind: "start_countdown" });
  facade.observeCountdown(facade.captureCountdownToken(), 3, true);
  const initial = createInitialAppModel();
  let model: AppModel = { ...updateApp(initial, { type: "game-session-synced", ...facade.readGameSessionProjection() }).model,
    presentation: { type: "ready", mode } };
  let terminal = false;
  let permissionDenied = false;
  let effects: ReturnType<typeof updateApp>["effects"] = [];
  const platform = () => {
    const currentMode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
    return sharedGameUiPlatform({
      presentation_mode: currentMode === "phone-vr" ? "phone_vr" : currentMode,
      flight_controller_failure: model.flightExecution.kind === "stopped" ? model.flightExecution.message : null
    });
  };
  const observeShared = (): void => {
    model = updateApp(model, { type: "shared-game-ui-observed", view: facade.readSharedUi(platform()) }).model;
  };
  observeShared();
  const update = (message: AppMessage): void => {
    const previous = model;
    const next = updateApp(model, message);
    model = next.model;
    effects = next.effects;
    for (const effect of next.effects) {
      if (effect.type !== "update-shared-game-ui") continue;
      const shared = facade.updateSharedUi(effect.action, platform());
      observeShared();
      effects = [];
      for (const typed of shared.effects) {
        const interpreted = updateApp(model, { type: "shared-game-ui-effect", effect: typed });
        model = interpreted.model;
        effects = [...effects, ...interpreted.effects];
      }
    }
    if (model !== previous) observeShared();
  };
  const session = {
    ...facade.flightPort,
    pause: (reason: number) => { facade.flightPort.pause(reason); },
    clear_pause_reason: (reason: number) => { facade.flightPort.clear_pause_reason(reason); },
    readSnapshot: () => facade.flightPort.readSnapshot(),
    advanceFrame: vi.fn((timestamp: number, gamepads: readonly FlightGamepadObservation[], available: boolean) => {
      const report = facade.flightPort.advanceFrame(timestamp, gamepads, available);
      if (terminal && report.advance.kind === "ticks" && report.advance.count > 0) {
        terminal = false;
        facade.executeSharedSessionCommand({ kind: "abort" });
        const snapshot = facade.flightPort.readSnapshot();
        return { ...report, phase_changed: true, snapshot, render_pose: snapshot.render_pose };
      }
      return report;
    }),
    free: vi.fn(() => { facade.dispose(); })
  };
  const display = { render: vi.fn(), setVisible: vi.fn() };
  const bindings = new FlightControllerUiBindings();
  const binding = bindings.bindAppDisplay(session, display, () => facade.readGameSessionProjection(), () => controller.currentSnapshot, update);
  const readGamepads = vi.fn(() => {
    if (permissionDenied) throw new Error("Gamepad permission denied");
    return [];
  });
  const input = { readGamepads, dispose: vi.fn() };
  const syncPhase = (phase: "flight_running" | "flight_paused" | "result"): void => {
    if (phase === "flight_paused" && facade.readLifecycle().phase === "flight_running") facade.executeSharedSessionCommand({ kind: "pause" });
    const projection = facade.readGameSessionProjection();
    if (projection.phase !== phase) throw new Error("Unexpected authoritative phase");
    update({ type: "game-session-synced", ...projection });
  };
  const onTerminal = vi.fn(() => { syncPhase("result"); });
  const controller = new TailFlightController(session, input, { setFlightPose: vi.fn() }, binding.port,
    onTerminal);
  update({ type: "flight-controller-ready", identity: binding.identity });
  disposals.push(() => { controller.dispose(); });
  return {
    facade, controller, session, binding, readGamepads, onTerminal, update, syncPhase,
    model: () => model,
    inMode: (nextMode: "screen" | "phone-vr" | "webxr") => {
      model = { ...model, presentation: { type: "ready", mode: nextMode } };
      observeShared();
      return model;
    },
    effects: () => effects,
    endNextTick: () => { terminal = true; },
    bind: (nextSession = session) => bindings.bindAppDisplay(nextSession, display, () => facade.readGameSessionProjection(), () => controller.currentSnapshot, update),
    stop: () => {
      controller.onFrame(0);
      controller.onFrame(10);
      const retained = controller.currentSnapshot;
      permissionDenied = true;
      controller.onFrame(20);
      return retained;
    }
  };
}

function viewer() {
  const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 100_000);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  camera.updateMatrixWorld(true);
  return captureConfiguredViewerFrame(camera, stereo);
}
