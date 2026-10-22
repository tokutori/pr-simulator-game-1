import { readFileSync } from "node:fs";
import { describe, expect, expectTypeOf, it, vi } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, gameSessionSnapshot, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, GameSessionUiState } from "../../web/src/app/app-state.js";
import { sharedGameUiPlatform, sharedHudNode } from "./shared-game-ui-fixture.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createFlightUiHudModel } from "../../web/src/app/flight-frame-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { projectTailGameSession } from "../../web/src/app/session-snapshot.js";
import type { TailGameSessionProjection } from "../../web/src/app/session-snapshot.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral = ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" as const } });

function projection(session: TailAppSessionFacade): TailGameSessionProjection {
  return projectTailGameSession(session.readSnapshot(), session.readLifecycle());
}

function launch(session: TailAppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  session.observeCountdown(session.captureCountdownToken(), 3, true);
}

function session(): TailAppSessionFacade {
  return createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
}

function observeSharedUi(model: AppModel, facade: TailAppSessionFacade, mode: "screen" | "phone-vr" | "webxr" = "screen"): AppModel {
  return updateApp(model, { type: "shared-game-ui-observed", view: facade.readSharedUi(sharedGameUiPlatform({
    presentation_mode: mode === "phone-vr" ? "phone_vr" : mode,
    flight_controller_failure: model.flightExecution.kind === "stopped" ? model.flightExecution.message : null
  })) }).model;
}

function sharedView(model: AppModel) {
  if (model.sharedGameUi === null) throw new Error("Missing shared Rust UI observation");
  return createGameViewModel(model.sharedGameUi);
}

describe("raw Rust snapshot application observations", () => {
  it("correlates named lifecycle states with their required display payload", () => {
    expectTypeOf<Extract<GameSessionUiState, { phase: "flight_running" }>["display"]["value"]["phase"]>().toEqualTypeOf<"flight_running">();
    expectTypeOf<Extract<GameSessionUiState, { phase: "flight_paused" }>["display"]["value"]["phase"]>().toEqualTypeOf<"flight_paused">();
    expectTypeOf<Extract<GameSessionUiState, { phase: "replay" }>["returnTarget"]>().toEqualTypeOf<"title" | "result">();
    expectTypeOf<Extract<GameSessionUiState, { phase: "boot" }>>().not.toHaveProperty("display");
  });

  it("holds the generated menu, live, paused and Result values without a phase alias model", () => {
    const facade = session();
    try {
      let model = createInitialAppModel();
      for (const kind of ["open_setup", "prepare"] as const) {
        facade.executeSharedSessionCommand({ kind });
        const menu = projection(facade);
        expect(menu.display).toEqual({ kind: "unavailable", reason: "menu_phase" });
        model = updateApp(model, { type: "game-session-synced", ...menu }).model;
        expect(gameSessionSnapshot(model.gameSession)).toBeNull();
      }
      model = observeSharedUi(model, facade);
      expect(sharedView(model).panels.flatMap((panel) => panel.controls).some((control) =>
        control.kind === "status" && control.id === "game-briefing-controls" && control.value.includes("↑") && control.value.includes("↓"))).toBe(true);
      facade.executeSharedSessionCommand({ kind: "start_countdown" });
      facade.observeCountdown(facade.captureCountdownToken(), 3, true);
      for (const phase of ["flight_running", "flight_paused", "result"] as const) {
        const current = projection(facade);
        expect(current.phase).toBe(phase);
        model = observeSharedUi(updateApp(model, { type: "game-session-synced", ...current }).model, facade);
        const snapshot = gameSessionSnapshot(model.gameSession);
        expect(current.display.kind).toBe("available");
        if (current.display.kind !== "available") throw new Error("Missing Rust display");
        expect(snapshot).toBe(current.display.value);
        expect(snapshot).not.toHaveProperty("positionNed");
        if (phase === "result") expect(createFlightUiHudModel(model)).toBeNull();
        else {
          const hud = createFlightUiHudModel(model);
          expect(hud).not.toBeNull();
          expect(hud?.groups.flatMap((group) => group.nodes).some((node) => node.id === "hud-controls")).toBe(false);
        }
        if (phase === "flight_running") facade.executeSharedSessionCommand({ kind: "pause" });
        if (phase === "flight_paused") facade.executeSharedSessionCommand({ kind: "abort" });
      }
      expect(model.gameSession.phase).toBe("result");
    } finally { facade.dispose(); }
  });

  it("uses one last valid tail snapshot for Screen and Head HUD and rejects old controller notifications", () => {
    const facade = session();
    try {
      launch(facade);
      facade.flightPort.advanceTick(neutral);
      const current = projection(facade);
      if (current.phase !== "flight_running") throw new Error("Expected a live tail projection");
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...current }).model;
      const bindings = new FlightControllerUiBindings();
      const display = { render: vi.fn(), setVisible: vi.fn() };
      const dispatch = (message: AppMessage): void => {
        const next = updateApp(model, message).model;
        if (next !== model) model = observeSharedUi(next, facade);
      };
      const old = bindings.bindAppDisplay(facade.flightPort, display, () => projection(facade), () => current.display.value, dispatch);
      dispatch({ type: "flight-controller-ready", identity: old.identity });
      const replacement = bindings.bindAppDisplay(facade.flightPort, display, () => projection(facade), () => current.display.value, dispatch);
      dispatch({ type: "flight-controller-ready", identity: replacement.identity });
      const ready = model;
      old.port.fail("Late controller callback");
      expect(model).toBe(ready);
      replacement.port.fail("Gamepad permission denied");
      expect(model.flightExecution).toMatchObject({ kind: "stopped", snapshot: current.display.value });
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing last valid state");
      expect(model.flightExecution.snapshot).toBe(current.display.value);
      for (const mode of ["screen", "phone-vr", "webxr"] as const) {
        const projected = observeSharedUi({ ...model, presentation: { type: "ready" as const, mode } }, facade, mode);
        const hud = createFlightUiHudModel(projected);
        if (hud === null) throw new Error("Missing shared stopped Flight HUD");
        expect(sharedHudNode(hud, "hud-controller-failure").content.value).toContain("Gamepad permission denied");
        expect(sharedHudNode(hud, "hud-last-valid-state")).toMatchObject({ label: "最後の有効状態", content: { value: "0.01 s" } });
        expect(sharedView(projected).description).toContain("停止");
      }
      expect(updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-abort" } }).effects)
        .toEqual([{ type: "update-shared-game-ui", action: { kind: "control_input", input: { type: "activate", controlId: "game-flight-abort" } } }]);
    } finally {
      facade.dispose();
    }
  });

  it("retains a pre-Pause live snapshot when adapter synchronization fails after Rust has paused", () => {
    const facade = session();
    try {
      launch(facade);
      facade.flightPort.advanceTick(neutral);
      const beforePause = projection(facade);
      if (beforePause.phase !== "flight_running") throw new Error("Expected a live tail projection");
      facade.executeSharedSessionCommand({ kind: "pause" });
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...projection(facade) }).model;
      const bindings = new FlightControllerUiBindings();
      const binding = bindings.bindAppDisplay(facade.flightPort, { render: vi.fn(), setVisible: vi.fn() },
        () => projection(facade), () => beforePause.display.value, (message) => { model = observeSharedUi(updateApp(model, message).model, facade); });
      model = updateApp(model, { type: "flight-controller-ready", identity: binding.identity }).model;
      binding.port.fail("Pause synchronization failed");
      expect(model.gameSession).toMatchObject({ phase: "flight_paused", display: { value: { phase: "flight_paused" } } });
      expect(model.flightExecution).toMatchObject({ kind: "stopped", snapshot: { phase: "flight_running" } });
      if (model.flightExecution.kind !== "stopped") throw new Error("Missing retained stop projection");
      expect(model.flightExecution.snapshot).toBe(beforePause.display.value);
      const resume = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" } });
      const request = resume.effects[0];
      if (request?.type !== "update-shared-game-ui") throw new Error("Missing widget request");
      expect(() => facade.updateSharedUi(request.action, sharedGameUiPlatform({
        flight_controller_failure: "Pause synchronization failed"
      }))).toThrow(/ActionUnavailable/);
      expect(updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-paused-abort" } }).effects)
        .toEqual([{ type: "update-shared-game-ui", action: { kind: "control_input", input: { type: "activate", controlId: "game-paused-abort" } } }]);
    } finally {
      facade.dispose();
    }
  });

});
