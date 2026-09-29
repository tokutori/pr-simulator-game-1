import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import { executeGameSessionOperation } from "../../web/src/app/game-session-operation.js";
import type { AppEffect, AppModel, GameSessionProjection } from "../../web/src/app/app-state.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { loadFlightAnalysis, queryFlightRecordRenderPoseAt, queryFlightRecordSampleAt } from "../../web/src/game/flight-record-query.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
let wasmInitialized = false;

function initializeWasm(): void {
  if (wasmInitialized) return;
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  wasmInitialized = true;
}

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

describe("Screen UI to WebAssembly GameSession transitions", () => {
  it("completes the visible Title-to-Result-to-Retry flow through actual DOM clicks", async () => {
    initializeWasm();
    const session = new GameSessionBridge(0);
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = window.document.createElement("main") as unknown as HTMLElement;
    window.document.body.append(root as never);
    let model: AppModel = Object.freeze({
      ...createInitialAppModel(),
      presentation: Object.freeze({ type: "ready", mode: "screen" })
    });

    const sessionProjection = (): GameSessionProjection => {
      const phaseCode = session.phase_code();
      return {
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
        snapshot: [5, 6, 7].includes(phaseCode) ? parseFlightSnapshot(session.snapshot()) : null
      };
    };

    const render = (): void => {
      const projection = sessionProjection();
      model = updateApp(model, { type: "game-session-synced", ...projection }).model;
      adapter.render(createGameViewModel(model, projection.snapshot, model.flightAnalysis));
    };

    const adapter = new ScreenUiAdapter(root, (action) => {
      const requested = updateApp(model, { type: "ui-action", action });
      model = requested.model;
      adapter.render(createGameViewModel(model, sessionProjection().snapshot, model.flightAnalysis));
      const effect = requested.effects.find((candidate) => candidate.type === "game-session-operation");
      if (effect?.type !== "game-session-operation") return;
      const result = executeGameSessionOperation(session, effect.operation);
      const projection = sessionProjection();
      model = updateApp(model, {
        type: "game-operation-completed",
        requestId: effect.requestId,
        ...projection
      }).model;
      adapter.render(createGameViewModel(model, projection.snapshot, model.flightAnalysis));
      if (result.kind === "countdown-started") return;
      if (model.status.length > 0) throw new Error(model.status);
    });

    const activate = (controlId: string): void => {
      const button = root.querySelector(`button[data-control-id="${controlId}"]`) as unknown as HTMLButtonElement | null;
      if (button === null) throw new Error(`Visible control not found: ${controlId}`);
      if (button.disabled) throw new Error(`Visible control is disabled: ${controlId}`);
      button.click();
      expect(model.status).not.toMatch(/unavailable|rejected|InvalidTransition/i);
    };

    try {
      render();
      expect(model.gameSession.kind).toBe("title");
      activate("game-title-start");
      expect(model.gameSession.kind).toBe("setup");
      for (let index = 0; index < 4; index += 1) activate("game-setup-information");
      const windCue = root.querySelector('input[data-control-id="game-setup-information-wind"]') as unknown as HTMLInputElement | null;
      if (windCue === null) throw new Error("Custom wind cue toggle is missing");
      windCue.checked = false;
      windCue.dispatchEvent(new window.Event("change", { bubbles: true }) as unknown as Event);
      expect(session.information_level_code()).toBe(4);
      expect(Array.from(session.information_profile_codes())).toEqual([1, 1, 0, 0, 0, 0]);
      expect(model.difficulty.hudProfile.wind).toBe(false);
      activate("game-setup-start");
      expect(model.gameSession.kind).toBe("briefing-ready");
      activate("game-briefing-start");
      expect(model.gameSession.kind).toBe("countdown");
      while (session.countdown_remaining() > 0) session.advance_countdown();
      session.launch();
      render();
      expect(model.gameSession.kind).toBe("flight");

      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }
      expect(snapshot.terminal).toBe("water-contact");
      render();
      expect(model.gameSession.kind).toBe("result");
      activate("game-result-retry");
      expect(model.gameSession.kind).toBe("briefing-ready");
    } finally {
      session.free();
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("plays and seeks from visible Replay controls using the Rust-owned clock", async () => {
    initializeWasm();
    const session = new GameSessionBridge(0);
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = window.document.createElement("main") as unknown as HTMLElement;
    window.document.body.append(root as never);
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    session.advance_tick(0, 0, 0, 0);
    session.abort();
    const flightAnalysis = loadFlightAnalysis(session, physics_hz());
    const resultState = gameSessionState(7, 0, parseFlightSnapshot(session.snapshot()));
    if (resultState === null) throw new Error("Rust Result snapshot was rejected by the UI projection");
    let model: AppModel = Object.freeze({
      ...createInitialAppModel(),
      presentation: Object.freeze({ type: "ready", mode: "screen" }),
      gameSession: resultState,
      flightAnalysis,
      resultTab: "analysis",
      analysisCursorTimeSeconds: 0.005,
      pendingGameRequestId: null,
      nextRequestId: 2
    });
    let lastActionEffects: readonly AppEffect[] = [];
    const render = (): void => {
      adapter.render(createGameViewModel(model, model.gameSession.kind === "result" ? model.gameSession.snapshot : null, model.flightAnalysis));
    };
    const adapter = new ScreenUiAdapter(root, (action) => {
      const transition = updateApp(model, { type: "ui-action", action });
      model = transition.model;
      lastActionEffects = transition.effects;
      render();
    });
    const activate = (controlId: string): readonly AppEffect[] => {
      const button = root.querySelector(`button[data-control-id="${controlId}"]`) as unknown as HTMLButtonElement | null;
      if (button === null || button.disabled) throw new Error(`Enabled control not found: ${controlId}`);
      button.click();
      return lastActionEffects;
    };
    const runClockEffect = (effect: Extract<AppEffect, { type: "control-replay-clock" }>): void => {
      switch (effect.command.kind) {
        case "synchronize":
          if (effect.command.seekTimeSeconds !== null) session.seek_playback(effect.command.seekTimeSeconds);
          break;
        case "play": session.set_playback_playing(true); break;
        case "pause": session.set_playback_playing(false); break;
        case "seek": session.seek_playback(effect.command.timeSeconds); break;
        case "rate": session.set_playback_rate_code(effect.command.rateCode); break;
        case "advance": session.advance_playback(effect.command.elapsedSeconds); break;
      }
      const state = session.playback_clock_state();
      const completed = updateApp(model, {
        type: "replay-clock-command-completed",
        requestId: effect.requestId,
        generation: effect.generation,
        state: {
          timeSeconds: state[0] ?? Number.NaN,
          rateCode: (state[1] ?? -1) as 0 | 1 | 2,
          playing: state[2] === 1
        }
      });
      model = completed.model;
      for (const query of completed.effects) {
        if (query.type === "load-flight-replay-pose") {
          const pose = queryFlightRecordRenderPoseAt(
            session, physics_hz(), query.timeSeconds, flightAnalysis.initialPilotPositionMeters
          );
          model = updateApp(model, { type: "flight-replay-pose-loaded", requestId: query.requestId, pose }).model;
        } else if (query.type === "load-flight-analysis-cursor") {
          const sample = queryFlightRecordSampleAt(session, physics_hz(), query.timeSeconds);
          model = updateApp(model, { type: "flight-analysis-cursor-loaded", requestId: query.requestId, sample }).model;
        }
      }
      render();
    };

    try {
      render();
      const replayOperation = activate("game-result-replay").find((effect) => effect.type === "game-session-operation");
      if (replayOperation?.type !== "game-session-operation") throw new Error("Replay operation was not requested");
      session.enter_replay();
      const entered = updateApp(model, {
        type: "game-operation-completed", requestId: replayOperation.requestId, phaseCode: 9, controlModeCode: 0,
        difficulty: model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
      });
      model = entered.model;
      const synchronize = entered.effects.find((effect) => effect.type === "control-replay-clock");
      if (synchronize?.type !== "control-replay-clock") throw new Error("Replay clock synchronization was not requested");
      runClockEffect(synchronize);
      expect(model.analysisCursorTimeSeconds).toBeCloseTo(0.005);
      expect(session.playback_clock_state()[0]).toBeCloseTo(0.005);

      const playEffect = activate("game-replay-play-pause").find((effect) => effect.type === "control-replay-clock");
      if (playEffect?.type !== "control-replay-clock") throw new Error("Replay play command was not requested");
      runClockEffect(playEffect);
      expect(model.replayPlaying).toBe(true);

      const tick = updateApp(model, { type: "replay-clock-tick", generation: model.replayClockGeneration, elapsedSeconds: 0.005 });
      const advanceEffect = tick.effects.find((effect) => effect.type === "control-replay-clock");
      model = tick.model;
      if (advanceEffect?.type !== "control-replay-clock") throw new Error("Replay advance command was not requested");
      runClockEffect(advanceEffect);
      expect(model.analysisCursorTimeSeconds).toBeCloseTo(0.01);
      expect(model.replayPlaying).toBe(false);
      expect(model.status).not.toMatch(/unavailable|rejected|InvalidTransition/i);

      const returnOperation = activate("game-replay-return").find((effect) => effect.type === "game-session-operation");
      if (returnOperation?.type !== "game-session-operation") throw new Error("Replay return operation was not requested");
      session.leave_replay();
      const returned = updateApp(model, {
        type: "game-operation-completed", requestId: returnOperation.requestId, phaseCode: 7, controlModeCode: 0,
        difficulty: model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
      });
      model = returned.model;
      expect(model.gameSession.kind).toBe("result");
      expect(model.analysisCursorTimeSeconds).toBeCloseTo(0.01);
    } finally {
      session.free();
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });
});
