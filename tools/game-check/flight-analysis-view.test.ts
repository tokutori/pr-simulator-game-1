import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { analysisCursorData, createInitialAppModel, flightAnalysisData, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { projectRuntimePlaybackClock, readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { selectSharedReplayView, sharedGameUiPlatform } from "./shared-game-ui-fixture.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { NamedAnalysisCursor, NamedAnalysisDataset } from "../../web/src/game/flight-analysis-view.js";
import type { WindGridRequest } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const windRequest: WindGridRequest = { north_min_m: -10, east_min_m: -10, altitude_m: 10, spacing_m: 5 };
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function result(ticks = 3, seed = 41): TailAppSessionFacade {
  const bridge = new HybridGameSessionBridge(0, seed, 42);
  bridge.open_setup();
  bridge.prepare();
  bridge.mark_briefing_ready();
  bridge.start_countdown(1);
  bridge.observe_countdown(bridge.countdown_token(), 1, true);
  for (let index = 0; index < ticks; index += 1) bridge.advance_tick_json(neutral);
  bridge.abort();
  return new TailAppSessionFacade(bridge, physics_hz());
}

function dataset(session: TailAppSessionFacade, request: WindGridRequest = windRequest): NamedAnalysisDataset {
  return session.readAnalysisDataset({ kind: "available", value: request });
}

function cursor(session: TailAppSessionFacade, data: NamedAnalysisDataset, time = 0): NamedAnalysisCursor {
  return session.queryAnalysisCursor(time, data);
}

function observeUi(model: AppModel, session: TailAppSessionFacade): AppModel {
  return updateApp(model, { type: "shared-game-ui-observed", view: session.readSharedUi(sharedGameUiPlatform()) }).model;
}

function selectAnalysisCursor(session: TailAppSessionFacade, timeSeconds: number): void {
  const platform = sharedGameUiPlatform();
  session.updateSharedUi({ kind: "set_result_tab", tab: "analysis" }, platform);
  session.updateSharedUi({ kind: "seek_analysis", time_seconds: timeSeconds }, platform);
}

function enterReplayAtCursor(session: TailAppSessionFacade, timeSeconds: number): void {
  const platform = sharedGameUiPlatform();
  session.updateSharedUi({ kind: "session", command: { kind: "enter_replay" } }, platform);
  session.executeSharedSessionCommand({ kind: "enter_replay" });
  selectSharedReplayView(session, platform, "telemetry");
  session.updateSharedUi({ kind: "seek_replay", time_seconds: timeSeconds }, platform);
  session.executeSharedRecordAction({ kind: "seek_replay", time_seconds: timeSeconds });
}

function observeReplayClock(model: AppModel, session: TailAppSessionFacade): AppModel {
  const clock = model.replayClock;
  if (clock.kind !== "requesting") throw new Error("Expected the runtime playback clock observation request");
  return updateApp(observeUi(model, session), {
    type: "replay-clock-command-completed", requestId: clock.requestId, generation: clock.generation,
    state: projectRuntimePlaybackClock(session.readPlaybackClock())
  }).model;
}

function leaveReplay(session: TailAppSessionFacade): void {
  session.updateSharedUi({ kind: "session", command: { kind: "leave_replay" } }, sharedGameUiPlatform());
  session.executeSharedSessionCommand({ kind: "leave_replay" });
}

describe("saved Analysis asynchronous application state", () => {
  it("rejects foreign cursor contexts and stale requests while retaining the accepted dataset", () => {
    const session = result();
    const other = result(3, 43);
    try {
      const data = dataset(session);
      const currentCursor = cursor(session, data);
      const model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      const loaded = updateApp({ ...model, analysis: { kind: "loading", requestId: 4, nextRequestId: 5, retained: null } },
        { type: "flight-analysis-loaded", requestId: 4, data }).model;
      const previous: AppModel = { ...loaded, analysisCursor: { kind: "loading", requestId: 7, nextRequestId: 8, retained: currentCursor } };
      const rejected = updateApp(previous, { type: "flight-analysis-cursor-loaded", requestId: 7, sample: cursor(other, dataset(other)) });
      expect(analysisCursorData(rejected.model.analysisCursor)).toBe(currentCursor);
      expect(flightAnalysisData(rejected.model.analysis)).toBe(data);
      expect(rejected.model.analysisCursor.kind).toBe("failed");
      expect(rejected.effects).toEqual([]);
      expect(updateApp({ ...previous, analysisCursor: { kind: "loading", requestId: 9, nextRequestId: 10, retained: currentCursor } },
        { type: "flight-analysis-cursor-loaded", requestId: 7, sample: currentCursor }).model)
        .toMatchObject({ analysisCursor: { kind: "loading", requestId: 9, nextRequestId: 10 } });
    } finally { session.dispose(); other.dispose(); }
  });

  it("refreshes named Result and Replay contexts while retaining the shared cursor without persisting twice", () => {
    const session = result();
    try {
      const data = dataset(session);
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      selectAnalysisCursor(session, 0.015);
      model = observeUi({ ...model, analysis: { kind: "ready", data, nextRequestId: model.analysis.nextRequestId },
        analysisCursor: { kind: "ready", sample: cursor(session, data, 0.015), nextRequestId: model.analysisCursor.nextRequestId }, pendingGameRequestId: 5 }, session);
      enterReplayAtCursor(session, 0.015);
      const replayTransition = updateApp(model, { type: "game-operation-completed", requestId: 5, ...readRuntimeSessionProjection(session) });
      const replay = { ...replayTransition, model: observeReplayClock(replayTransition.model, session) };
      expect(flightAnalysisData(replay.model.analysis)).toBeNull();
      expect(analysisCursorData(replay.model.analysisCursor)).toBeNull();
      expect(replay.model.analysisCursorTimeSeconds).toBe(0.015);
      expect(replay.effects).toContainEqual({ type: "load-flight-analysis", requestId: model.analysis.nextRequestId });
      expect(replay.effects.some((effect) => effect.type === "persist-flight-record")).toBe(false);
      const replayData = dataset(session);
      const loaded = updateApp(replay.model, { type: "flight-analysis-loaded", requestId: model.analysis.nextRequestId, data: replayData });
      expect(flightAnalysisData(loaded.model.analysis)).toBe(replayData);
      leaveReplay(session);
      const returnedTransition = updateApp({ ...loaded.model, pendingGameRequestId: 6 }, {
        type: "game-operation-completed", requestId: 6, ...session.readGameSessionProjection()
      });
      const returned = { ...returnedTransition, model: observeUi(returnedTransition.model, session) };
      expect(flightAnalysisData(returned.model.analysis)).toBeNull();
      expect(returned.model.analysisCursorTimeSeconds).toBe(0.015);
      expect(returned.effects.some((effect) => effect.type === "persist-flight-record")).toBe(false);
      expect(returned.effects.some((effect) => effect.type === "load-flight-analysis")).toBe(true);
    } finally { session.dispose(); }
  });

  it.each(["pending", "failed"] as const)("reloads Result after returning before the Replay dataset is available (%s)", (loadingState) => {
    const session = result();
    try {
      const data = dataset(session);
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      selectAnalysisCursor(session, 0.015);
      model = observeUi({ ...model, analysis: { kind: "ready", data, nextRequestId: model.analysis.nextRequestId }, pendingGameRequestId: 5 }, session);
      enterReplayAtCursor(session, 0.015);
      let replay = observeReplayClock(updateApp(model, { type: "game-operation-completed", requestId: 5,
        ...readRuntimeSessionProjection(session) }).model, session);
      if (replay.analysis.kind !== "loading") throw new Error("Expected the named Replay load request");
      const oldRequestId = replay.analysis.requestId;
      const replayData = dataset(session);
      if (loadingState === "failed") replay = updateApp(replay, {
        type: "flight-analysis-failed", requestId: oldRequestId, message: "injected query failure"
      }).model;
      leaveReplay(session);
      const returnedTransition = updateApp({ ...replay, pendingGameRequestId: 6 }, {
        type: "game-operation-completed", requestId: 6, ...session.readGameSessionProjection()
      });
      const returned = { ...returnedTransition, model: observeUi(returnedTransition.model, session) };
      if (returned.model.analysis.kind !== "loading") throw new Error("Expected a fresh Result request");
      const freshRequestId = returned.model.analysis.requestId;
      expect(freshRequestId).not.toBe(oldRequestId);
      expect(returned.effects).toEqual([{ type: "load-flight-analysis", requestId: freshRequestId }]);
      expect(returned.model.analysisCursorTimeSeconds).toBe(0.015);
      expect(updateApp(returned.model, { type: "flight-analysis-loaded", requestId: oldRequestId, data: replayData }).model).toBe(returned.model);
      const resultData = dataset(session);
      const loaded = updateApp(returned.model, { type: "flight-analysis-loaded", requestId: freshRequestId, data: resultData });
      expect(flightAnalysisData(loaded.model.analysis)).toBe(resultData);
      expect(loaded.effects.some((effect) => effect.type === "persist-flight-record")).toBe(false);
    } finally { session.dispose(); }
  });
});
