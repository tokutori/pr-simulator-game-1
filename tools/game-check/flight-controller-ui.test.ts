import { afterEach, describe, expect, it, vi } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel } from "../../web/src/app/app-state.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createFlightFrameViewDraft, createFlightUiHudModel, finalizeFlightFrameView } from "../../web/src/app/flight-frame-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { readFileSync } from "node:fs";
import { initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { currentFlightDisplayFixture, launchCurrentSession } from "./current-session-fixture.js";
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
      expect(model.flightExecution).toMatchObject({ kind: "stopped", message: "Gamepad permission denied", snapshot: retained });
      expect(model.gameSession.kind).toBe("flight");
      expect(trial.controller.currentDisplaySnapshot).toEqual(retained);
      expect(trial.onTerminal).not.toHaveBeenCalled();
      const hud = createFlightUiHudModel(model, currentFlightDisplayFixture());
      expect(hud.status).toBe("飛行処理停止");
      expect(hud.warning).toContain("Gamepad permission denied");
      expect(hud.telemetry).toContain("Last valid · tick 1");
      const draft = createFlightFrameViewDraft(model, currentFlightDisplayFixture(), viewer(), "ja");
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing retained controller state");
      expect(draft.snapshot).toBe(model.flightExecution.snapshot);
      const view = finalizeFlightFrameView(draft, draft.headHud);
      expect(view.description).toContain("停止");
      const diagnosis = view.panels[0]?.controls.find((control) => control.id === "game-controller-stopped");
      if (diagnosis?.kind !== "status") throw new Error("Missing stop diagnosis");
      expect(diagnosis.value).toContain("Gamepad permission denied");
      expect(viewExposesAction(view, { type: "activate", controlId: "game-flight-abort" })).toBe(true);
      if (mode !== "screen") {
        expect(draft.headHud.kind).toBe("visible");
        if (view.headHud.kind !== "visible") throw new Error("Missing stop Head HUD");
        const warning = view.headHud.elements.find((element) => element.id === "head-warning");
        const readouts = view.headHud.elements.find((element) => element.id === "head-readouts");
        if (warning?.kind !== "text" || readouts?.kind !== "text") throw new Error("Missing stop Head HUD text");
        expect(warning.value).toContain("Gamepad permission denied");
        expect(readouts.label).toBe("飛行処理停止");
        expect(readouts.value).toContain("Last valid");
      }
    }
  });

  it("retains the diagnosis through Pause, focus, backend changes and page restoration while rejecting Resume", () => {
    const trial = fixture("screen");
    trial.stop();
    const diagnosis = trial.model().flightExecution;
    trial.syncPhase(6);
    const stopped = trial.model();
    expect(updateApp(stopped, { type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" } })).toEqual({ model: stopped, effects: [] });
    trial.update({ type: "ui-action", action: { type: "focus", controlId: "game-flight-resume" } });
    trial.update({ type: "page-suspended" });
    trial.update({ type: "page-restored" });
    trial.update({ type: "game-session-status", message: "Backend restored" });
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      const model = { ...trial.model(), presentation: { type: "ready" as const, mode } };
      expect(model.flightExecution).toBe(diagnosis);
      const view = createGameViewModel(model, trial.controller.currentDisplaySnapshot);
      expect(viewExposesAction(view, { type: "activate", controlId: "game-flight-resume" })).toBe(false);
      expect(viewExposesAction(view, { type: "activate", controlId: "game-paused-abort" })).toBe(true);
      expect(view.description).toContain("最後の有効");
    }
    const reads = trial.readGamepads.mock.calls.length;
    trial.controller.resume();
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
    expect(() => { trial.controller.reset("{}", replacement.port); }).toThrow(RangeError);
    expect(trial.model()).toBe(stopped);
    trial.controller.reset(trial.session.snapshot_json(), replacement.port);
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
    trial.update({ type: "ui-action", action: { type: "activate", controlId: "game-flight-abort" } });
    expect(trial.effects()).toEqual([expect.objectContaining({ type: "game-session-operation", operation: "abort" })]);
    const requestId = trial.model().pendingGameRequestId;
    if (requestId === null) throw new Error("Missing Abort request");
    expect(retained.kind).toBe("tail_flight");
    trial.facade.executeOperation("abort");
    trial.controller.reset(trial.session.snapshot_json());
    trial.update({ type: "game-operation-completed", requestId, ...trial.facade.readGameSessionProjection() });
    expect(trial.model().gameSession.kind).toBe("result");
    expect(trial.model().flightExecution).toBe(diagnosis);
    expect(trial.controller.currentSnapshot.phaseCode).toBe(7);
    trial.facade.executeOperation("retry");
    trial.facade.executeOperation("start-flight");
    while (trial.facade.advanceCountdown() > 0) continue;
    trial.facade.launch();
    const replacement = trial.bind();
    trial.controller.reset(trial.session.snapshot_json(), replacement.port);
    trial.update({ type: "flight-controller-ready", identity: replacement.identity });
    trial.syncPhase(5);
    expect(trial.model().flightExecution).toEqual({ kind: "ready", identity: replacement.identity });
    expect(createFlightUiHudModel(trial.model(), trial.controller.currentDisplaySnapshot).status).not.toBe("飛行処理停止");
  });

  it("keeps normal terminal notification and Result persistence separate from adapter stops", () => {
    const trial = fixture("screen");
    trial.endNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.model().gameSession.kind).toBe("result");
    expect(trial.model().flightExecution.kind).toBe("ready");
    expect(trial.effects()).toEqual(expect.arrayContaining([expect.objectContaining({ type: "persist-flight-record" })]));
    const result = trial.model();
    trial.binding.port.fail("Late callback after Result");
    expect(trial.model()).toBe(result);
  });
});

function fixture(mode: "screen" | "phone-vr" | "webxr", informationCode = 0) {
  const facade = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  launchCurrentSession(facade);
  const initial = createInitialAppModel();
  let model: AppModel = { ...updateApp(initial, { type: "game-session-synced", ...facade.readGameSessionProjection() }).model,
    presentation: { type: "ready", mode }, difficulty: { ...initial.difficulty, informationCode, hudProfile: {
      telemetry: false, attitude: false, wind: false, flightPath: false, angleOfAttack: false, warnings: false
    } } };
  let terminal = false;
  let permissionDenied = false;
  let effects: ReturnType<typeof updateApp>["effects"] = [];
  const update = (message: AppMessage): void => {
    const next = updateApp(model, message);
    model = next.model;
    effects = next.effects;
  };
  const session = {
    snapshot_json: () => facade.flightPort.snapshot_json(),
    control_profile_json: () => facade.flightPort.control_profile_json(),
    advance_tick_json: vi.fn((json: string) => {
      if (terminal) { facade.executeOperation("abort"); return facade.flightPort.snapshot_json(); }
      return facade.flightPort.advance_tick_json(json);
    }),
    free: vi.fn(() => { facade.dispose(); })
  };
  const display = { render: vi.fn(), setVisible: vi.fn() };
  const input = {
    readDemand: () => ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
      pilotPositionCommand: { kind: "hold" as const } }),
    reset: vi.fn(), suspend: vi.fn(), resume: vi.fn(), dispose: vi.fn()
  };
  const bindings = new FlightControllerUiBindings();
  const binding = bindings.bindDisplay(session, display, () => controller.currentDisplaySnapshot, update);
  const readGamepads = vi.fn(() => {
    if (permissionDenied) throw new Error("Gamepad permission denied");
    return [];
  });
  const syncPhase = (phaseCode: 5 | 6 | 7): void => {
    if (phaseCode === 6 && facade.readLifecycle().phaseCode === 5) facade.executeOperation("pause");
    const projection = facade.readGameSessionProjection();
    if (projection.phaseCode !== phaseCode) throw new Error("Unexpected authoritative phase");
    update({ type: "game-session-synced", ...projection, difficulty: model.difficulty });
  };
  const onTerminal = vi.fn(() => { syncPhase(7); });
  const controller = new TailFlightController(session, input, { setFlightPose: vi.fn() }, binding.port,
    physics_hz(), readGamepads, onTerminal);
  update({ type: "flight-controller-ready", identity: binding.identity });
  disposals.push(() => { controller.dispose(); });
  return {
    facade, controller, session, binding, readGamepads, onTerminal, update, syncPhase,
    model: () => model,
    effects: () => effects,
    endNextTick: () => { terminal = true; },
    bind: (nextSession = session) => bindings.bindDisplay(nextSession, display, () => controller.currentDisplaySnapshot, update),
    stop: () => {
      controller.onFrame(0);
      controller.onFrame(10);
      const retained = controller.currentDisplaySnapshot;
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
