import { readFileSync } from "node:fs";
import { describe, expect, expectTypeOf, it, vi } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, gameSessionSnapshot, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, GameSessionUiState, TailGameSessionProjection } from "../../web/src/app/app-state.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createFlightUiHudModel } from "../../web/src/app/flight-frame-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { isTerminalSessionSnapshot, projectTailGameSession } from "../../web/src/app/session-snapshot.js";
import type { TerminalSessionSnapshot } from "../../web/src/app/session-snapshot.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function projection(session: TailAppSessionFacade): TailGameSessionProjection {
  return projectTailGameSession(session.readSnapshot(), session.readLifecycle(), createInitialAppModel().difficulty, null);
}

function launch(session: TailAppSessionFacade): void {
  session.executeOperation("open-setup");
  session.executeOperation("prepare");
  session.executeOperation("start-flight");
  while (session.advanceCountdown() > 0) continue;
  session.launch();
}

function session(): TailAppSessionFacade {
  return createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
}

describe("single Rust snapshot application projection", () => {
  it("correlates the public live layout and phase types and requires terminal record proof", () => {
    expectTypeOf<Extract<GameSessionUiState, { kind: "flight"; controlLayout: "tail_incidence" }>["snapshot"]["phaseCode"]>().toEqualTypeOf<5>();
    expectTypeOf<Extract<GameSessionUiState, { kind: "paused-flight"; controlLayout: "tail_incidence" }>["snapshot"]["phaseCode"]>().toEqualTypeOf<6>();
    expectTypeOf<Extract<FlightDisplaySnapshot, { kind: "tail_record" }>>().not.toExtend<TerminalSessionSnapshot>();
    expectTypeOf<Extract<Extract<GameSessionUiState, { kind: "result"; controlLayout: "tail_incidence" }>["display"], { kind: "available" }>["value"]["kind"]>()
      .toEqualTypeOf<"tail_result" | "tail_record">();
    expectTypeOf<Extract<Extract<GameSessionUiState, { kind: "attract"; controlLayout: "tail_incidence" }>["display"], { kind: "available" }>["value"]["kind"]>()
      .toEqualTypeOf<"tail_record">();
  });

  it("derives menu, live, pause and Result from real Rust snapshots without legacy fields", () => {
    const facade = session();
    try {
      let model = createInitialAppModel();
      for (const operation of ["open-setup", "prepare"] as const) {
        facade.executeOperation(operation);
        const menu = projection(facade);
        expect(menu.display).toEqual({ kind: "unavailable", reason: "menu_phase" });
        model = updateApp(model, { type: "game-session-synced", ...menu }).model;
        expect(gameSessionSnapshot(model.gameSession)).toBeNull();
      }
      expect(createGameViewModel(model, null).panels.flatMap((panel) => panel.controls).some((control) =>
        control.kind === "status" && control.value.includes("独立したroll"))).toBe(true);
      facade.executeOperation("start-flight");
      while (facade.advanceCountdown() > 0) continue;
      facade.launch();
      for (const expectedPhase of [5, 6, 7] as const) {
        const current = projection(facade);
        expect(current.phaseCode).toBe(expectedPhase);
        model = updateApp(model, { type: "game-session-synced", ...current }).model;
        const snapshot = gameSessionSnapshot(model.gameSession);
        if (snapshot === null) throw new Error("Missing Rust display snapshot");
        expect(snapshot.controls.layout).toBe("tail_incidence");
        expect(snapshot).not.toHaveProperty("actuatorDeflectionRadians");
        expect(snapshot).not.toHaveProperty("tick");
        expect(createFlightUiHudModel(model, snapshot).controlsDescription).toContain("nose-up/down");
        if (expectedPhase === 5) facade.executeOperation("pause");
        if (expectedPhase === 6) facade.executeOperation("abort");
      }
      expect(model.gameSession.kind).toBe("result");
      expect(() => projectTailGameSession(facade.readSnapshot(), { ...facade.readLifecycle(), phaseCode: 5 }, model.difficulty, null)).toThrow("disagree");
    } finally {
      facade.dispose();
    }
  });

  it("rejects layout and phase mismatch without publishing a partial AppModel", () => {
    const facade = session();
    try {
      launch(facade);
      const current = projection(facade);
      if (current.phaseCode !== 5) throw new Error("Expected a live tail projection");
      const tail = current.display.value;
      expect(gameSessionState(6, 0, tail, false, null, "tail_incidence")).toBeNull();
      expect(gameSessionState(1, 0, tail, false, null, "tail_incidence")).toBeNull();
      const initial = createInitialAppModel();
      const malformed = { type: "game-session-synced", ...current, phaseCode: 6 } as unknown as AppMessage;
      expect(updateApp(initial, malformed).model.gameSession).toBe(initial.gameSession);
    } finally {
      facade.dispose();
    }
  });

  it("keeps Result at the recorded terminal stamp and Playback at its independent cursor", () => {
    const facade = session();
    try {
      launch(facade);
      facade.flightPort.advance_tick_json(neutral);
      facade.executeOperation("abort");
      const early = facade.queryRecordDisplay(0.005);
      const terminal = facade.queryRecordDisplay(0.01);
      expect(isTerminalSessionSnapshot(early)).toBe(false);
      expect(gameSessionState(7, 0, early, false, null, "tail_incidence")).toBeNull();
      const result = gameSessionState(7, 0, terminal, false, null, "tail_incidence");
      expect(result?.kind).toBe("result");
      expect(gameSessionSnapshot(result ?? { kind: "boot", phaseCode: -1 })).toBe(terminal);
      for (const phase of [9, 10]) {
        const playback = gameSessionState(phase, 0, early, false, null, "tail_incidence", phase === 9 ? "result" : undefined);
        expect(playback?.kind).toBe(phase === 9 ? "replay" : "attract");
        if (playback === null) throw new Error("Missing Playback projection");
        expect(gameSessionSnapshot(playback)).toBe(early);
      }
      expect(gameSessionState(7, 0, null, false, null, "tail_incidence")).toMatchObject({
        kind: "result", display: { kind: "unavailable", reason: "record_not_loaded" }
      });
    } finally {
      facade.dispose();
    }
  });

  it("uses one last valid tail snapshot for Screen and Head HUD and rejects old controller notifications", () => {
    const facade = session();
    try {
      launch(facade);
      facade.flightPort.advance_tick_json(neutral);
      const current = projection(facade);
      if (current.phaseCode !== 5) throw new Error("Expected a live tail projection");
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...current }).model;
      const bindings = new FlightControllerUiBindings();
      const display = { render: vi.fn(), setVisible: vi.fn() };
      const dispatch = (message: AppMessage): void => { model = updateApp(model, message).model; };
      const old = bindings.bindDisplay(facade.flightPort, display, () => current.display.value, dispatch);
      dispatch({ type: "flight-controller-ready", identity: old.identity });
      const replacement = bindings.bindDisplay(facade.flightPort, display, () => current.display.value, dispatch);
      dispatch({ type: "flight-controller-ready", identity: replacement.identity });
      const ready = model;
      old.port.fail("Late controller callback");
      expect(model).toBe(ready);
      replacement.port.fail("Gamepad permission denied");
      expect(model.flightExecution).toMatchObject({ kind: "stopped", snapshot: current.display.value });
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing last valid state");
      expect(model.flightExecution.snapshot).toBe(current.display.value);
      for (const mode of ["screen", "phone-vr", "webxr"] as const) {
        const projected = { ...model, presentation: { type: "ready" as const, mode } };
        const hud = createFlightUiHudModel(projected, current.display.value);
        expect(hud.warning).toContain("Gamepad permission denied");
        expect(hud.telemetry).toContain("Last valid · tick 1");
        expect(createGameViewModel(projected, current.display.value).description).toContain("停止");
      }
      expect(updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-abort" } }).effects)
        .toEqual([expect.objectContaining({ type: "game-session-operation", operation: "abort" })]);
    } finally {
      facade.dispose();
    }
  });

  it("retains a pre-Pause live snapshot when adapter synchronization fails after Rust has paused", () => {
    const facade = session();
    try {
      launch(facade);
      facade.flightPort.advance_tick_json(neutral);
      const beforePause = projection(facade);
      if (beforePause.phaseCode !== 5) throw new Error("Expected a live tail projection");
      facade.executeOperation("pause");
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...projection(facade) }).model;
      const bindings = new FlightControllerUiBindings();
      const binding = bindings.bindDisplay(facade.flightPort, { render: vi.fn(), setVisible: vi.fn() },
        () => beforePause.display.value, (message) => { model = updateApp(model, message).model; });
      model = updateApp(model, { type: "flight-controller-ready", identity: binding.identity }).model;
      binding.port.fail("Pause synchronization failed");
      expect(model.gameSession).toMatchObject({ kind: "paused-flight", phaseCode: 6, snapshot: { phaseCode: 6 } });
      expect(model.flightExecution).toMatchObject({ kind: "stopped", snapshot: { phaseCode: 5 } });
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing retained stop projection");
      expect(model.flightExecution.snapshot).toBe(beforePause.display.value);
      const resume = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" } });
      expect(resume.effects).toEqual([]);
      expect(updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-paused-abort" } }).effects)
        .toEqual([expect.objectContaining({ type: "game-session-operation", operation: "abort" })]);
    } finally {
      facade.dispose();
    }
  });

});
