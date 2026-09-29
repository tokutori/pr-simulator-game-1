import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { keyboardIntent } from "../../web/src/game/keyboard-intent.js";
import { BrowserPilotInput } from "../../web/src/game/browser-input.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { executeGameSessionOperation } from "../../web/src/app/game-session-operation.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import type { FlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

function hudProfile(session: GameSessionBridge) {
  const values = session.information_profile_codes();
  return {
    telemetry: values[0] === 1,
    attitude: values[1] === 1,
    wind: values[2] === 1,
    flightPath: values[3] === 1,
    angleOfAttack: values[4] === 1,
    warnings: values[5] === 1
  };
}

describe("generated WebAssembly browser binding", () => {
  it("plays an independent Title demo without exporting it as a player flight", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      expect(session.flight_record_sample_count()).toBe(0);
      session.enter_attract();
      expect(session.phase_code()).toBe(10);
      expect(session.flight_record_sample_count()).toBeGreaterThan(100);
      expect(session.flight_record_summary()[1]).toBeGreaterThan(0);
      expect(session.flight_record_sample_at(0, 0)).toHaveLength(36);
      expect(() => session.export_flight_record_json()).toThrow();

      session.leave_attract();
      expect(session.phase_code()).toBe(0);
      expect(session.flight_record_sample_count()).toBe(0);
    } finally {
      session.free();
    }
  });

  it("runs a fixed-rate flight through fractional water contact", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    expect(physics_hz()).toBe(100);

    const session = new GameSessionBridge(0);
    try {
      expect(session.phase_code()).toBe(0);
      session.open_setup();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      expect(session.phase_code()).toBe(2);
      session.fail_briefing(0);
      expect(session.phase_code()).toBe(8);
      session.retry_briefing();
      expect(session.phase_code()).toBe(2);
      session.cancel_briefing();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      expect(session.advance_countdown()).toBe(0);
      const initial = parseFlightSnapshot(session.launch());
      expect(initial.tick).toBe(0);
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let index = 0; index < 3_000 && snapshot.terminal === "airborne"; index += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(Number.isInteger(snapshot.tick)).toBe(true);
      expect(snapshot.contactFraction).toBeGreaterThanOrEqual(0);
      expect(snapshot.contactFraction).toBeLessThanOrEqual(1);
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(200);
      expect(session.advance_tick(0, 0, 0, 0)).toEqual(session.snapshot());
      session.retry();
      expect(session.phase_code()).toBe(3);
      session.open_setup();
      session.return_to_title();
      expect(session.phase_code()).toBe(0);
    } finally {
      session.free();
    }
  });

  it("keeps a short pilot-position keyboard input within the playable glide range", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      session.open_setup();
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      session.advance_countdown();
      session.launch();

      let targetPositionMeters = 0;
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        const pressed = tick >= 160 && tick < 170 ? new Set(["KeyJ"]) : new Set<string>();
        const intent = keyboardIntent(pressed, targetPositionMeters, physics_hz());
        targetPositionMeters = intent.pilotPositionMeters;
        snapshot = parseFlightSnapshot(session.advance_tick(
          intent.roll,
          intent.pitch,
          intent.yaw,
          intent.pilotPositionMeters
        ));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(180);
      expect(snapshot.scoreCourseMeters).toBeLessThanOrEqual(230);
      expect(Math.abs(snapshot.pilotPositionMeters)).toBeGreaterThan(0.03);
    } finally {
      session.free();
    }
  });

  it("routes a neutral-confirmed gamepad input through WASM flight", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    const target = { addEventListener() {}, removeEventListener() {} } as unknown as Window;
    const input = new BrowserPilotInput(target);
    const neutralGamepad = { connected: true, axes: [0, 0, 0, 0] } as unknown as Gamepad;
    const activeGamepad = { connected: true, axes: [0, 0, 0, 0.18] } as unknown as Gamepad;
    try {
      session.open_setup();
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      session.advance_countdown();
      session.launch();
      input.readIntent([neutralGamepad]);

      let snapshot = parseFlightSnapshot(session.snapshot());
      let minimumPilotPositionMeters = snapshot.pilotPositionMeters;
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        const intent = input.readIntent(tick >= 160 && tick < 170 ? [activeGamepad] : [neutralGamepad]);
        snapshot = parseFlightSnapshot(session.advance_tick(
          intent.roll,
          intent.pitch,
          intent.yaw,
          intent.pilotPositionMeters
        ));
        minimumPilotPositionMeters = Math.min(minimumPilotPositionMeters, snapshot.pilotPositionMeters);
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(180);
      expect(snapshot.scoreCourseMeters).toBeLessThanOrEqual(230);
      expect(minimumPilotPositionMeters).toBeLessThan(-0.005);
    } finally {
      input.dispose();
      session.free();
    }
  });

  it.each([
    { mode: 0, label: "Manual" },
    { mode: 1, label: "Shared" },
    { mode: 2, label: "Automatic" }
  ])("runs the $label control mode through pause, water contact, and retry", ({ mode }) => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      session.open_setup();
      session.set_control_mode(mode);
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      session.advance_countdown();
      session.launch();

      session.pause(0);
      expect(session.phase_code()).toBe(6);
      session.resume();
      expect(session.phase_code()).toBe(5);

      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(session.phase_code()).toBe(7);
      session.retry();
      expect(session.phase_code()).toBe(3);
    } finally {
      session.free();
    }
  });

  it.each(["screen", "webxr", "phone-vr"] as const)("runs UI actions through one Rust-owned flow on %s", (mode) => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      let model: AppModel = Object.freeze({
        ...createInitialAppModel(),
        presentation: Object.freeze({ type: "ready", mode })
      });
      const projectedScene = (): string => {
        const phaseCode = session.phase_code();
        const snapshot = phaseCode === 5 || phaseCode === 6
          ? parseFlightSnapshot(session.snapshot())
          : null;
        model = updateApp(model, {
          type: "game-session-synced",
          phaseCode,
          controlModeCode: session.control_mode_code(),
          difficulty: {
            presetCode: session.difficulty_preset_code(),
            informationCode: session.information_level_code(),
            hudProfile: hudProfile(session),
            assistanceCode: session.assistance_level_code(),
            weatherCode: session.weather_class_code()
          },
          configurationMetadata: null,
          countdownRemaining: session.countdown_remaining(),
          canResume: session.can_resume(),
          snapshot
        }).model;
        return createGameViewModel(model, snapshot).scene;
      };
      const requestOperation = (controlId: string, expectedOperation: string) => {
        const requested = updateApp(model, {
          type: "ui-action",
          action: { type: "activate", controlId }
        });
        const effect = requested.effects.find((entry) => entry.type === "game-session-operation");
        expect(effect).toEqual({
          type: "game-session-operation",
          operation: expectedOperation,
          requestId: requested.model.pendingGameRequestId
        });
        expect(effect?.type).toBe("game-session-operation");
        if (effect?.type !== "game-session-operation") throw new Error(`No GameSession operation for ${controlId}`);
        let result;
        try {
          result = executeGameSessionOperation(session, effect.operation);
        } catch (error: unknown) {
          throw new Error(`${controlId} failed: ${String(error)}`, { cause: error });
        }
        model = requested.model;
        return { requestId: requested.model.pendingGameRequestId as number, result };
      };
      const completeOperation = (requestId: number, terminalSnapshot?: FlightSnapshot): void => {
        const phaseCode = session.phase_code();
        model = updateApp(model, {
          type: "game-operation-completed",
          requestId,
          phaseCode,
          controlModeCode: session.control_mode_code(),
          difficulty: {
            presetCode: session.difficulty_preset_code(),
            informationCode: session.information_level_code(),
            hudProfile: hudProfile(session),
            assistanceCode: session.assistance_level_code(),
            weatherCode: session.weather_class_code()
          },
          configurationMetadata: null,
          countdownRemaining: session.countdown_remaining(),
          canResume: session.can_resume(),
          snapshot: terminalSnapshot ?? (phaseCode === 5 || phaseCode === 6 ? parseFlightSnapshot(session.snapshot()) : null)
        }).model;
      };

      expect(projectedScene()).toBe("Title");
      const openSetup = requestOperation("game-title-start", "open-setup");
      completeOperation(openSetup.requestId);
      expect(projectedScene()).toBe("FlightSetup");
      for (const [controlId, operation] of [
        ["game-setup-mode-shared", "set-control-shared"],
        ["game-setup-mode-automatic", "set-control-automatic"],
        ["game-setup-mode-manual", "set-control-manual"],
        ["game-setup-preset", "cycle-difficulty-preset"],
        ["game-setup-information", "cycle-information-level"],
        ["game-setup-assistance", "cycle-assistance-level"],
        ["game-setup-weather", "cycle-weather-class"]
      ] as const) {
        const configured = requestOperation(controlId, operation);
        completeOperation(configured.requestId);
        expect(projectedScene()).toBe("FlightSetup");
      }
      session.prepare();
      session.fail_briefing(0);
      expect(projectedScene()).toBe("Briefing");
      expect(model.gameSession.kind).toBe("briefing-failed");
      const retryBriefing = requestOperation("game-briefing-retry", "retry-briefing");
      completeOperation(retryBriefing.requestId);
      expect(projectedScene()).toBe("Briefing");
      const cancelBriefing = requestOperation("game-briefing-cancel", "cancel-briefing");
      completeOperation(cancelBriefing.requestId);
      expect(projectedScene()).toBe("FlightSetup");
      const prepare = requestOperation("game-setup-start", "prepare");
      completeOperation(prepare.requestId);
      expect(projectedScene()).toBe("Briefing");
      const cancelReadyBriefing = requestOperation("game-briefing-cancel", "cancel-briefing");
      completeOperation(cancelReadyBriefing.requestId);
      expect(projectedScene()).toBe("FlightSetup");
      const prepareAgain = requestOperation("game-setup-start", "prepare");
      completeOperation(prepareAgain.requestId);
      expect(projectedScene()).toBe("Briefing");
      const startFlight = requestOperation("game-briefing-start", "start-flight");
      expect(startFlight.result.kind).toBe("countdown-started");
      completeOperation(startFlight.requestId);
      expect(projectedScene()).toBe("Countdown");
      const cancelCountdown = requestOperation("game-countdown-cancel", "cancel-countdown");
      completeOperation(cancelCountdown.requestId);
      expect(projectedScene()).toBe("Briefing");
      const restartCountdown = requestOperation("game-briefing-start", "start-flight");
      completeOperation(restartCountdown.requestId);
      while (session.countdown_remaining() > 0) session.advance_countdown();
      session.launch();
      expect(projectedScene()).toBe("Flight");
      const pause = requestOperation("game-flight-pause", "pause");
      completeOperation(pause.requestId);
      expect(projectedScene()).toBe("Flight");
      const settings = updateApp(model, {
        type: "ui-action",
        action: { type: "activate", controlId: "game-pause-open-settings" }
      });
      expect(settings.model.gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "settings" } });
      model = settings.model;
      model = updateApp(model, {
        type: "ui-action",
        action: { type: "activate", controlId: "game-pause-settings-back" }
      }).model;
      const resume = requestOperation("game-flight-resume", "resume");
      completeOperation(resume.requestId);

      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }
      expect(snapshot.terminal).toBe("water-contact");
      expect(projectedScene()).toBe("Result");
      const retry = requestOperation("game-result-retry", "retry");
      completeOperation(retry.requestId);
      expect(projectedScene()).toBe("Briefing");
      const retryStart = requestOperation("game-briefing-start", "start-flight");
      completeOperation(retryStart.requestId);
      expect(projectedScene()).toBe("Countdown");
      while (session.countdown_remaining() > 0) session.advance_countdown();
      session.launch();
      expect(projectedScene()).toBe("Flight");
      const abort = requestOperation("game-flight-abort", "abort");
      expect(abort.result.kind).toBe("aborted");
      if (abort.result.kind !== "aborted") throw new Error("Abort did not return a terminal snapshot");
      completeOperation(abort.requestId, parseFlightSnapshot(abort.result.terminalSnapshot));
      expect(projectedScene()).toBe("Result");
      const returnToTitle = requestOperation("game-result-title", "return-to-title");
      completeOperation(returnToTitle.requestId);
      expect(projectedScene()).toBe("Title");
      const demo = requestOperation("game-title-demo", "enter-attract");
      completeOperation(demo.requestId);
      expect(projectedScene()).toBe("Title");
      expect(model.gameSession.kind).toBe("attract");
      const leaveDemo = requestOperation("game-attract-return", "leave-attract");
      completeOperation(leaveDemo.requestId);
      expect(projectedScene()).toBe("Title");
      const setupAgain = requestOperation("game-title-start", "open-setup");
      completeOperation(setupAgain.requestId);
      const setupBack = requestOperation("game-setup-back", "return-to-title");
      completeOperation(setupBack.requestId);
      expect(projectedScene()).toBe("Title");
    } finally {
      session.free();
    }
  });
});
