import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Window } from "happy-dom";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, gameSessionSnapshot, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppModel } from "../../web/src/app/app-state.js";
import { queryRuntimeRecordPose, readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";

beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
});

describe("Screen UI to current WebAssembly GameSession transitions", () => {
  it.each(["manual_abort", "water_contact"] as const)("loads Result cursor effects through WASM after %s", (terminal) => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    try {
      launchCurrentSession(session);
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      session.flightPort.advance_tick_json(neutralTailInput);
      session.flightPort.advance_tick_json(neutralTailInput);
      if (terminal === "manual_abort") session.executeOperation("abort");
      else for (let tick = 2; tick < 4_000 && session.readLifecycle().phaseCode === 5; tick += 1) {
        session.flightPort.advance_tick_json(neutralTailInput);
      }
      const before = session.flightPort.snapshot_json();
      const encoded = session.exportRecordJson();
      const entered = updateApp(model, { type: "game-session-synced", ...session.readGameSessionProjection() });
      model = entered.model;
      const load = entered.effects.find((effect) => effect.type === "load-flight-analysis");
      if (load?.type !== "load-flight-analysis") throw new Error("Result did not request Analysis");
      const analysis = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(analysis.context.finalization.reason).toBe(terminal);
      const completeCursor = (transition: ReturnType<typeof updateApp>): void => {
        model = transition.model;
        const query = transition.effects.find((effect) => effect.type === "load-flight-analysis-cursor");
        if (query?.type !== "load-flight-analysis-cursor") throw new Error("Result cursor query was not requested");
        const sample = session.queryAnalysisCursor(query.timeSeconds, analysis);
        model = updateApp(model, { type: "flight-analysis-cursor-loaded", requestId: query.requestId, sample }).model;
        expect(model.analysisCursorSample?.timeSeconds).toBeCloseTo(query.timeSeconds, 12);
        expect(model.pendingAnalysisCursorRequestId).toBeNull();
        expect(model.gameSession.kind).toBe("result");
      };
      completeCursor(updateApp(model, { type: "flight-analysis-loaded", requestId: load.requestId, data: analysis }));
      expect(model.analysisCursorSample?.state).toEqual(analysis.samples[0]?.state);
      model = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-result-open-analysis" } }).model;
      for (const timeSeconds of [0.005, analysis.summary.durationSeconds]) {
        completeCursor(updateApp(model, { type: "ui-action", action: { type: "set-range", controlId: "game-analysis-cursor", value: timeSeconds } }));
      }
      expect(model.analysisCursorSample?.state).toEqual(analysis.samples.at(-1)?.state);
      expect(session.flightPort.snapshot_json()).toBe(before);
      expect(session.exportRecordJson()).toBe(encoded);
    } finally { session.dispose(); }
  });

  it("completes Title-to-Result-to-Retry and Replay through actual DOM controls", async () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    const window = new Window();
    vi.stubGlobal("window", window);
    vi.stubGlobal("document", window.document);
    vi.stubGlobal("HTMLElement", window.HTMLElement);
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = window.document.createElement("main") as unknown as HTMLElement;
    window.document.body.append(root as never);
    let model: AppModel = { ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" } };
    let lastEffects: readonly AppEffect[] = [];
    const render = (): void => {
      adapter.render(createGameViewModel(model, gameSessionSnapshot(model.gameSession), model.flightAnalysis));
    };
    const adapter = new ScreenUiAdapter(root, (action) => {
      const transition = updateApp(model, { type: "ui-action", action });
      model = transition.model;
      lastEffects = transition.effects;
      const operation = transition.effects.find((effect) => effect.type === "game-session-operation");
      if (operation?.type === "game-session-operation") {
        session.executeOperation(operation.operation);
        const completed = updateApp(model, { type: "game-operation-completed", requestId: operation.requestId, ...readRuntimeSessionProjection(session) });
        model = completed.model;
        lastEffects = completed.effects;
      }
      render();
    });
    const activate = (controlId: string): void => {
      const button = root.querySelector<HTMLButtonElement>(`button[data-control-id="${controlId}"]`);
      if (button === null || button.disabled) throw new Error(`Enabled control not found: ${controlId}`);
      button.click();
      expect(model.status).not.toMatch(/unavailable|rejected|InvalidTransition/i);
    };
    const synchronize = (): void => {
      const transition = updateApp(model, { type: "game-session-synced", ...readRuntimeSessionProjection(session) });
      model = transition.model;
      lastEffects = transition.effects;
      render();
    };
    const completeRecordQueries = (effects: readonly AppEffect[]): void => {
      for (const effect of effects) {
        if (effect.type === "load-flight-analysis") {
          const data = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
          const loaded = updateApp(model, { type: "flight-analysis-loaded", requestId: effect.requestId, data });
          model = loaded.model;
          completeRecordQueries(loaded.effects);
        } else if (effect.type === "load-flight-analysis-cursor") {
          if (model.flightAnalysis === null) throw new Error("Cursor requires observed Analysis");
          const sample = session.queryAnalysisCursor(effect.timeSeconds, model.flightAnalysis);
          model = updateApp(model, { type: "flight-analysis-cursor-loaded", requestId: effect.requestId, sample }).model;
        } else if (effect.type === "load-flight-replay-pose") {
          if (model.flightAnalysis === null) throw new Error("Replay pose requires observed Analysis");
          const { pose } = queryRuntimeRecordPose(session, model.flightAnalysis, effect.timeSeconds);
          model = updateApp(model, { type: "flight-replay-pose-loaded", requestId: effect.requestId, pose }).model;
        }
      }
    };
    const runClockEffect = (effect: Extract<AppEffect, { type: "control-replay-clock" }>): void => {
      switch (effect.command.kind) {
        case "synchronize": if (effect.command.seekTimeSeconds !== null) session.seekPlayback(effect.command.seekTimeSeconds); break;
        case "play": session.setPlaybackPlaying(true); break;
        case "pause": session.setPlaybackPlaying(false); break;
        case "seek": session.seekPlayback(effect.command.timeSeconds); break;
        case "rate": session.setPlaybackRate(effect.command.rateCode); break;
        case "advance": session.advancePlayback(effect.command.elapsedSeconds); break;
      }
      const clock = session.readPlaybackClock();
      const completed = updateApp(model, { type: "replay-clock-command-completed",
        requestId: effect.requestId, generation: effect.generation,
        state: { timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: clock.kind === "playing" } });
      model = completed.model;
      completeRecordQueries(completed.effects);
      render();
    };
    try {
      synchronize();
      activate("game-title-start");
      expect(model.gameSession.kind).toBe("setup");
      activate("game-setup-select-information-4");
      const windCue = root.querySelector<HTMLInputElement>('input[data-control-id="game-setup-information-wind"]');
      if (windCue === null) throw new Error("Custom wind cue toggle is missing");
      windCue.checked = false;
      windCue.dispatchEvent(new window.Event("change", { bubbles: true }) as unknown as Event);
      expect(session.readDifficulty().hudProfile.wind).toBe(false);
      activate("game-setup-start");
      expect(model.gameSession.kind).toBe("briefing-ready");
      activate("game-briefing-start");
      expect(model.gameSession.kind).toBe("countdown");
      activate("game-countdown-cancel");
      activate("game-briefing-start");
      while (session.advanceCountdown() > 0) continue;
      session.launch();
      synchronize();
      activate("game-flight-pause");
      activate("game-pause-open-settings");
      expect(model.gameSession.kind === "paused-flight" && model.gameSession.overlay.kind).toBe("settings");
      activate("game-pause-settings-back");
      activate("game-flight-resume");
      session.flightPort.advance_tick_json(neutralTailInput);
      session.flightPort.advance_tick_json(neutralTailInput);
      activate("game-flight-abort");
      expect(model.gameSession.kind).toBe("result");
      const record = JSON.parse(session.exportRecordJson()) as { schema_version: number; header: { difficulty: { hud_profile: { wind: boolean } } } };
      expect(record.schema_version).toBe(6);
      expect(record.header.difficulty.hud_profile.wind).toBe(false);
      const requestId = model.pendingAnalysisRequestId;
      if (requestId === null) throw new Error("Result did not request Analysis");
      const analysis = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const loaded = updateApp(model, { type: "flight-analysis-loaded", requestId, data: analysis });
      model = loaded.model;
      completeRecordQueries(loaded.effects);
      render();
      activate("game-result-open-analysis");
      const cursor = updateApp(model, { type: "ui-action", action: { type: "set-range", controlId: "game-analysis-cursor", value: 0.005 } });
      model = cursor.model;
      completeRecordQueries(cursor.effects);
      render();
      activate("game-result-replay");
      expect(model.gameSession.kind).toBe("replay");
      const synchronizeClock = lastEffects.find((effect) => effect.type === "control-replay-clock");
      if (synchronizeClock?.type !== "control-replay-clock") throw new Error("Replay clock synchronization was not requested");
      completeRecordQueries(lastEffects);
      runClockEffect(synchronizeClock);
      expect(session.readPlaybackClock().timeSeconds).toBeCloseTo(0.005);
      activate("game-replay-play-pause");
      const play = lastEffects.find((effect) => effect.type === "control-replay-clock");
      if (play?.type !== "control-replay-clock") throw new Error("Replay play was not requested");
      runClockEffect(play);
      const advance = updateApp(model, { type: "replay-clock-tick", generation: model.replayClockGeneration, elapsedSeconds: 0.1 });
      model = advance.model;
      const advanceClock = advance.effects.find((effect) => effect.type === "control-replay-clock");
      if (advanceClock?.type !== "control-replay-clock") throw new Error("Replay advance was not requested");
      runClockEffect(advanceClock);
      expect(model.replayPlaying).toBe(false);
      expect(model.analysisCursorTimeSeconds).toBeCloseTo(analysis.summary.durationSeconds);
      activate("game-replay-return");
      activate("game-result-retry");
      expect(model.gameSession.kind).toBe("briefing-ready");
      activate("game-briefing-cancel");
      activate("game-setup-back");
      activate("game-title-demo");
      expect(model.gameSession.kind).toBe("attract");
      activate("game-attract-return");
      expect(model.gameSession.kind).toBe("title");
    } finally {
      session.dispose();
      await window.happyDOM.abort();
      vi.unstubAllGlobals();
    }
  });
});
