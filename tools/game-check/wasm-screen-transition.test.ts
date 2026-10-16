import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Window } from "happy-dom";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { analysisCursorData, createInitialAppModel, flightAnalysisData, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppModel } from "../../web/src/app/app-state.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";

function observeSharedUi(session: TailAppSessionFacade, model: AppModel): AppModel {
  return updateApp(model, { type: "shared-game-ui-observed", view: session.readSharedUi(sharedGameUiPlatform({
    operation_pending: model.pendingGameRequestId !== null
  })) }).model;
}

function sharedUiTransition(session: TailAppSessionFacade, model: AppModel, action: UiAction): ReturnType<typeof updateApp> {
  const transition = updateApp(observeSharedUi(session, model), { type: "ui-action", action });
  const effects: AppEffect[] = [];
  let next = transition.model;
  for (const effect of transition.effects) {
    if (effect.type !== "update-shared-game-ui") {
      effects.push(effect);
      continue;
    }
    const update = session.updateSharedUi(effect.action, sharedGameUiPlatform());
    next = observeSharedUi(session, next);
    for (const typed of update.effects) {
      const interpreted = updateApp(next, { type: "shared-game-ui-effect", effect: typed });
      next = interpreted.model;
      effects.push(...interpreted.effects);
    }
  }
  return { model: next, effects };
}

beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
});

describe("Screen UI to current WebAssembly GameSession transitions", () => {
  it.each(["webxr", "phone-vr"] as const)("stops the actual Rust Replay clock during %s interruption", (mode) => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      launchCurrentSession(session);
      session.flightPort.advance_tick_json(neutralTailInput);
      session.flightPort.advance_tick_json(neutralTailInput);
      session.executeOperation("abort");
      session.executeOperation("enter-replay");
      session.setPlaybackPlaying(true);
      const record = session.exportRecordJson();
      const initial = updateApp(createInitialAppModel(), { type: "game-session-synced", ...readRuntimeSessionProjection(session) }).model;
      const active: AppModel = { ...initial, presentation: { type: "ready", mode },
        replayClock: { ...initial.replayClock, generation: 7,
          observation: { ...initial.replayClock.observation, playing: true } } };
      const recovery = updateApp(active, { type: "backend-ended", mode, message: "Tracking lost" });
      const pause = recovery.effects.find((effect) => effect.type === "control-replay-clock");
      if (pause?.type !== "control-replay-clock" || pause.command.kind !== "pause") throw new Error("Interruption did not stop Replay");
      const clock = session.setPlaybackPlaying(false);
      expect(clock.kind).toBe("paused");
      expect(session.advancePlayback(10).timeSeconds).toBe(clock.timeSeconds);
      expect(updateApp(recovery.model, { type: "replay-clock-tick", generation: 7, elapsedSeconds: 10 }).model).toBe(recovery.model);
      const paused = updateApp(recovery.model, { type: "replay-clock-command-completed", requestId: pause.requestId, generation: pause.generation,
        state: { timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: false } });
      expect(paused.effects.some((effect) => effect.type === "schedule-replay-clock-tick")).toBe(false);
      expect(session.readLifecycle().phaseCode).toBe(9);
      expect(session.exportRecordJson()).toBe(record);
    } finally { session.dispose(); }
  });

  it.each(["allocation", "display", "resync", "resync_retry"] as const)("resynchronizes the observed Result record after a Retry %s failure", (failure) => {
    const bridge = new HybridGameSessionBridge(0, 0, 0);
    const session = new TailAppSessionFacade(bridge, physics_hz());
    const free = vi.spyOn(bridge, "free");
    const resyncFailure = failure === "resync" || failure === "resync_retry";
    try {
      launchCurrentSession(session);
      session.flightPort.advance_tick_json(neutralTailInput);
      session.executeOperation("abort");
      const initial: AppModel = { ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" } };
      const entered = updateApp(initial, { type: "game-session-synced", ...readRuntimeSessionProjection(session) });
      const initialLoad = entered.effects.find((effect) => effect.type === "load-flight-analysis");
      if (initialLoad?.type !== "load-flight-analysis") throw new Error("Result did not request Analysis");
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const loaded = updateApp(entered.model, { type: "flight-analysis-loaded", requestId: initialLoad.requestId, data: dataset });
      const initialCursor = loaded.effects.find((effect) => effect.type === "load-flight-analysis-cursor");
      if (initialCursor?.type !== "load-flight-analysis-cursor") throw new Error("Result did not request its cursor");
      let model = updateApp(loaded.model, { type: "flight-analysis-cursor-loaded", requestId: initialCursor.requestId,
        sample: session.queryAnalysisCursor(initialCursor.timeSeconds, dataset) }).model;
      model = sharedUiTransition(session, model, { type: "activate", controlId: "game-result-open-analysis" }).model;
      const sought = sharedUiTransition(session, model, { type: "set-range", controlId: "game-analysis-cursor", value: 0.005 });
      const oldCursor = sought.effects.find((effect) => effect.type === "load-flight-analysis-cursor");
      if (oldCursor?.type !== "load-flight-analysis-cursor") throw new Error("Analysis did not request its cursor");
      const oldSample = session.queryAnalysisCursor(oldCursor.timeSeconds, dataset);
      const requested = sharedUiTransition(session, sought.model, { type: "activate", controlId: "game-result-retry" });
      const operation = requested.effects.find((effect) => effect.type === "shared-game-session-command");
      if (operation?.type !== "shared-game-session-command") throw new Error("Result did not request Retry");
      expect(operation.command).toEqual({ kind: "retry" });
      const before = session.flightPort.snapshot_json();
      const record = session.exportRecordJson();
      const token = session.captureQueryToken();
      if (failure === "display") {
        session.executeSharedSessionCommand(operation.command);
        vi.spyOn(bridge, "snapshot_json").mockReturnValueOnce("[]");
        expect(() => readRuntimeSessionProjection(session)).toThrow();
      } else {
        vi.spyOn(bridge, "apply_game_ui_session_command_json").mockImplementationOnce((command) => {
          expect(JSON.parse(command)).toEqual({ kind: "retry" });
          throw new RangeError("Record(AllocationFailed)");
        });
        expect(() => session.executeSharedSessionCommand(operation.command)).toThrow("Record(AllocationFailed)");
      }
      expect(session.acceptQuery(token, "old completion").kind).toBe("stale");
      expect(session.isAnalysisDatasetCurrent(dataset)).toBe(false);
      expect(() => session.queryAnalysisCursor(0.005, dataset)).toThrow("stale");
      const message = failure === "display" ? "Snapshot display conversion failed" : "Record(AllocationFailed)";
      if (resyncFailure) {
        vi.spyOn(bridge, "snapshot_json").mockReturnValueOnce("[]");
        expect(() => readRuntimeSessionProjection(session)).toThrow();
      }
      const rejected = updateApp(requested.model, { type: "game-operation-failed", requestId: operation.requestId, message,
        analysisSourceInvalidated: !session.isAnalysisDatasetCurrent(dataset),
        ...(resyncFailure ? {} : { currentSession: readRuntimeSessionProjection(session) }) });
      expect(rejected.model.pendingGameRequestId).toBeNull();
      expect(rejected.model.status).toBe(message);
      expect(flightAnalysisData(rejected.model.analysis)).toBeNull();
      expect(analysisCursorData(rejected.model.analysisCursor)).toBeNull();
      expect(updateApp(rejected.model, { type: "flight-analysis-cursor-loaded", requestId: oldCursor.requestId,
        sample: oldSample }).model).toBe(rejected.model);
      if (failure === "display") {
        expect(session.readLifecycle().phaseCode).toBe(3);
        expect(rejected.model.gameSession.kind).toBe("briefing-ready");
        expect(rejected.effects).toEqual([]);
      } else {
        expect(session.readLifecycle().phaseCode).toBe(7);
        expect(session.flightPort.snapshot_json()).toBe(before);
        expect(session.exportRecordJson()).toBe(record);
        expect(rejected.model.gameSession.kind).toBe("result");
        expect(rejected.model.resultTab).toBe("summary");
        expect(rejected.model.analysisCursorTimeSeconds).toBe(0);
        {
          let recovered = rejected;
          if (resyncFailure) {
            expect(rejected.effects).toEqual([]);
            if (failure === "resync_retry") {
              const retried = sharedUiTransition(session, rejected.model, { type: "activate", controlId: "game-result-retry" });
              const retry = retried.effects.find((effect) => effect.type === "shared-game-session-command");
              if (retry?.type !== "shared-game-session-command") throw new Error("Result did not allow another Retry");
              vi.spyOn(bridge, "apply_game_ui_session_command_json").mockImplementationOnce(() => {
                throw new RangeError("Record(AllocationFailed)");
              });
              expect(() => session.executeSharedSessionCommand(retry.command)).toThrow("Record(AllocationFailed)");
              expect(flightAnalysisData(retried.model.analysis)).toBeNull();
              recovered = updateApp(retried.model, { type: "game-operation-failed", requestId: retry.requestId, message,
                analysisSourceInvalidated: false, currentSession: readRuntimeSessionProjection(session) });
            } else {
              recovered = updateApp(rejected.model, { type: "game-session-synced", ...readRuntimeSessionProjection(session) });
            }
          }
          const reload = recovered.effects.find((effect) => effect.type === "load-flight-analysis");
          if (reload?.type !== "load-flight-analysis") throw new Error("Failed Retry did not refresh stale Analysis");
          expect(recovered.effects).toEqual([reload]);
          const repeated = updateApp(recovered.model, { type: "game-session-synced", ...readRuntimeSessionProjection(session) });
          expect(repeated.effects).toEqual([]);
          expect(repeated.model.analysis).toBe(recovered.model.analysis);
          expect(updateApp(repeated.model, { type: "flight-analysis-loaded", requestId: initialLoad.requestId,
            data: dataset }).model).toBe(repeated.model);
          expect(() => session.queryAnalysisCursor(0.005, dataset)).toThrow("stale");
          const invalidatedLoading = updateApp({ ...repeated.model, pendingGameRequestId: operation.requestId }, {
            type: "game-operation-failed", requestId: operation.requestId, message,
            analysisSourceInvalidated: !session.isAnalysisDatasetCurrent(dataset), currentSession: readRuntimeSessionProjection(session)
          });
          expect(invalidatedLoading.effects).toEqual([{ type: "load-flight-analysis", requestId: repeated.model.analysis.nextRequestId }]);
          expect(invalidatedLoading.model.analysis).toEqual({ kind: "loading", requestId: repeated.model.analysis.nextRequestId,
            nextRequestId: repeated.model.analysis.nextRequestId + 1, retained: null });
          expect(updateApp(invalidatedLoading.model, { type: "flight-analysis-loaded", requestId: reload.requestId,
            data: dataset }).model).toBe(invalidatedLoading.model);
          const currentDataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
          expect(currentDataset.context).toEqual(dataset.context);
          expect(currentDataset.summary).toEqual(dataset.summary);
          const reloaded = updateApp(repeated.model, { type: "flight-analysis-loaded", requestId: reload.requestId, data: currentDataset });
          const cursor = reloaded.effects.find((effect) => effect.type === "load-flight-analysis-cursor");
          if (cursor?.type !== "load-flight-analysis-cursor") throw new Error("Refreshed Analysis did not request its cursor");
          model = updateApp(reloaded.model, { type: "flight-analysis-cursor-loaded", requestId: cursor.requestId,
            sample: session.queryAnalysisCursor(cursor.timeSeconds, currentDataset) }).model;
          expect(flightAnalysisData(model.analysis)).toBe(currentDataset);
          model = sharedUiTransition(session, model, { type: "activate", controlId: "game-result-open-analysis" }).model;
          const continued = sharedUiTransition(session, model, { type: "set-range", controlId: "game-analysis-cursor", value: 0.005 });
          const continuedCursor = continued.effects.find((effect) => effect.type === "load-flight-analysis-cursor");
          if (continuedCursor?.type !== "load-flight-analysis-cursor") throw new Error("Failed Retry did not allow Analysis seek");
          model = updateApp(continued.model, { type: "flight-analysis-cursor-loaded", requestId: continuedCursor.requestId,
            sample: session.queryAnalysisCursor(continuedCursor.timeSeconds, currentDataset) }).model;
          expect(analysisCursorData(model.analysisCursor)?.timeSeconds).toBe(0.005);
          const retryAgain = sharedUiTransition(session, model, { type: "activate", controlId: "game-result-retry" });
          expect(retryAgain.effects).toEqual([{ type: "shared-game-session-command", command: { kind: "retry" },
            requestId: retryAgain.model.pendingGameRequestId }]);
        }
      }
    } finally { session.dispose(); session.dispose(); }
    expect(free).toHaveBeenCalledTimes(1);
  });

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
        expect(analysisCursorData(model.analysisCursor)?.timeSeconds).toBeCloseTo(query.timeSeconds, 12);
        expect(model.analysisCursor.kind).toBe("ready");
        expect(model.gameSession.kind).toBe("result");
      };
      completeCursor(updateApp(model, { type: "flight-analysis-loaded", requestId: load.requestId, data: analysis }));
      expect(analysisCursorData(model.analysisCursor)?.state).toEqual(analysis.samples[0]?.state);
      model = sharedUiTransition(session, model, { type: "activate", controlId: "game-result-open-analysis" }).model;
      for (const timeSeconds of [0.005, analysis.summary.durationSeconds]) {
        completeCursor(sharedUiTransition(session, model, { type: "set-range", controlId: "game-analysis-cursor", value: timeSeconds }));
      }
      expect(analysisCursorData(model.analysisCursor)?.state).toEqual(analysis.samples.at(-1)?.state);
      expect(session.flightPort.snapshot_json()).toBe(before);
      expect(session.exportRecordJson()).toBe(encoded);
    } finally { session.dispose(); }
  });

  it("completes Title-to-Result-to-Retry and Replay through actual DOM controls", async () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    const window = new Window();
    window.document.documentElement.style.fontSize = "16px";
    vi.stubGlobal("window", window);
    vi.stubGlobal("document", window.document);
    vi.stubGlobal("HTMLElement", window.HTMLElement);
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = window.document.createElement("main") as unknown as HTMLElement;
    window.document.body.append(root as never);
    let model: AppModel = { ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" } };
    let lastEffects: readonly AppEffect[] = [];
    const render = (): void => {
      model = observeSharedUi(session, model);
      if (model.sharedGameUi === null) throw new Error("Missing shared Rust UI");
      adapter.render(createGameViewModel(model.sharedGameUi));
    };
    const adapter = new ScreenUiAdapter(root, (action) => {
      const transition = sharedUiTransition(session, model, action);
      model = transition.model;
      lastEffects = transition.effects;
      const operation = transition.effects.find((effect) => effect.type === "shared-game-session-command");
      if (operation?.type === "shared-game-session-command") {
        session.executeSharedSessionCommand(operation.command);
        const completed = updateApp(model, { type: "game-operation-completed", requestId: operation.requestId, ...readRuntimeSessionProjection(session) });
        model = completed.model;
        lastEffects = completed.effects;
      } else {
        const clockAction = transition.effects.find((effect) => effect.type === "shared-replay-action");
        if (clockAction?.type === "shared-replay-action") {
          session.executeSharedRecordAction(clockAction.action);
          const clock = session.readPlaybackClock();
          const completed = updateApp(model, { type: "replay-clock-command-completed",
            requestId: clockAction.requestId, generation: clockAction.generation,
            state: { timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: clock.kind === "playing" } });
          model = completed.model;
          lastEffects = completed.effects;
          completeRecordQueries(completed.effects);
        }
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
          const analysis = flightAnalysisData(model.analysis);
          if (analysis === null) throw new Error("Cursor requires observed Analysis");
          const sample = session.queryAnalysisCursor(effect.timeSeconds, analysis);
          model = updateApp(model, { type: "flight-analysis-cursor-loaded", requestId: effect.requestId, sample }).model;
        } else if (effect.type === "load-flight-replay-pose") {
          const analysis = flightAnalysisData(model.analysis);
          if (analysis === null) throw new Error("Replay pose requires observed Analysis");
          const pose = session.queryReplayCamera(effect.timeSeconds, model.replayCameraMode, true, analysis);
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
      session.observeCountdown(session.captureCountdownToken(), 3, true);
      synchronize();
      activate("game-flight-pause");
      activate("game-pause-open-settings");
      expect(model.gameSession.kind === "paused-flight" && model.sharedGameUi?.selections.pause_overlay).toBe("settings");
      activate("game-pause-settings-back");
      activate("game-flight-resume");
      session.flightPort.advance_tick_json(neutralTailInput);
      session.flightPort.advance_tick_json(neutralTailInput);
      activate("game-flight-abort");
      expect(model.gameSession.kind).toBe("result");
      const record = JSON.parse(session.exportRecordJson()) as { schema_version: number; header: { difficulty: { hud_profile: { wind: boolean } } } };
      expect(record.schema_version).toBe(6);
      expect(record.header.difficulty.hud_profile.wind).toBe(false);
      if (model.analysis.kind !== "loading") throw new Error("Result did not request Analysis");
      const requestId = model.analysis.requestId;
      const analysis = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const loaded = updateApp(model, { type: "flight-analysis-loaded", requestId, data: analysis });
      model = loaded.model;
      completeRecordQueries(loaded.effects);
      render();
      activate("game-result-open-analysis");
      const cursor = sharedUiTransition(session, model, { type: "set-range", controlId: "game-analysis-cursor", value: 0.005 });
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
      expect(session.readPlaybackClock().kind).toBe("playing");
      const advance = updateApp(model, { type: "replay-clock-tick", generation: model.replayClock.generation, elapsedSeconds: 0.1 });
      model = advance.model;
      const advanceClock = advance.effects.find((effect) => effect.type === "control-replay-clock");
      if (advanceClock?.type !== "control-replay-clock") throw new Error("Replay advance was not requested");
      runClockEffect(advanceClock);
      expect(model.replayClock.observation.playing).toBe(false);
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
