import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { analysisScenarioId, projectAnalysisCursor, projectAnalysisView } from "../../web/src/game/flight-analysis-view.js";
import type { NamedAnalysisCursor, NamedAnalysisDataset } from "../../web/src/game/flight-analysis-view.js";
import type { NamedWindGridRequest } from "../../web/src/game/named-record-analysis.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";
import { resolveAttractCameraMode, resolveReplayCameraMode } from "../../web/src/render/camera/camera-director.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const windRequest: NamedWindGridRequest = { northMinimumMeters: -10, eastMinimumMeters: -10, altitudeMeters: 10, spacingMeters: 5 };
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function result(ticks = 3, seed = 41): TailAppSessionFacade {
  const bridge = new HybridGameSessionBridge(0, seed, 42);
  bridge.open_setup();
  bridge.prepare();
  bridge.mark_briefing_ready();
  bridge.start_countdown(1);
  bridge.advance_countdown();
  bridge.launch();
  for (let index = 0; index < ticks; index += 1) bridge.advance_tick_json(neutral);
  bridge.abort();
  return new TailAppSessionFacade(bridge, physics_hz());
}

function dataset(session: TailAppSessionFacade, request: NamedWindGridRequest = windRequest): NamedAnalysisDataset {
  return session.readAnalysisDataset({ kind: "available", value: request });
}

function cursor(session: TailAppSessionFacade, data: NamedAnalysisDataset, time = 0): NamedAnalysisCursor {
  return session.queryAnalysisCursor(time, data);
}

function archive(version: number): string {
  const bridge = new GameSessionBridge(0);
  try {
    bridge.open_setup();
    bridge.prepare();
    bridge.mark_briefing_ready();
    bridge.start_countdown(1);
    bridge.advance_countdown();
    bridge.launch();
    bridge.advance_tick(0, 0, 0, 0);
    bridge.abort();
    const saved = JSON.parse(bridge.export_flight_record_json()) as Record<string, unknown>;
    saved.schema_version = version;
    const header = saved.header as Record<string, unknown>;
    if (version < 5) delete header.personal_best_key;
    if (version < 4) delete header.physics_model_version;
    if (version < 3) delete header.score_definition_version;
    if (version < 2) delete (header.difficulty as Record<string, unknown>).hud_profile;
    return JSON.stringify(saved);
  } finally {
    bridge.free();
  }
}

describe("pure saved Analysis view projection", () => {
  it("uses Rust composite CG and telemetry, preserving original finalization and reasoned metrics", () => {
    const session = result();
    try {
      const data = dataset(session);
      const view = projectAnalysisView(data);
      const sample = data.samples[0];
      if (sample === undefined) throw new Error("Expected the initial Rust sample");
      expect(sample.state.datumPositionNedMeters).not.toEqual(sample.state.telemetry.compositeCgPositionNedMeters);
      expect(view.samples[0]).toMatchObject({ northMeters: sample.state.telemetry.compositeCgPositionNedMeters[0],
        eastMeters: sample.state.telemetry.compositeCgPositionNedMeters[1], altitudeMeters: sample.state.telemetry.altitudeMeters,
        airspeedMetersPerSecond: sample.state.telemetry.airspeedMetersPerSecond });
      expect(view.summary.score).toBe(data.summary.scoreMeters);
      expect(view.summary.maximumAngleOfAttackRadians).toBe(data.summary.maximumAngleOfAttackRadians);
      expect(view.origin).toMatchObject({ kind: "named_record", context: data.context });
      expect(projectAnalysisCursor(cursor(session, data), data)).toEqual(view.samples[0]);
      expect(data.samples[0]).toBe(sample);
    } finally { session.dispose(); }
  });

  it("rejects different sample, summary, wind and cursor contexts without committing partial Model updates", () => {
    const session = result();
    const other = result(3, 43);
    try {
      const data = dataset(session);
      const otherData = dataset(other);
      expect(() => projectAnalysisView({ ...data, context: otherData.context })).toThrow(/different records/);
      expect(() => projectAnalysisView({ ...data, windGrid: otherData.windGrid })).toThrow(/different records/);
      expect(() => projectAnalysisCursor(cursor(other, otherData), data)).toThrow(/another record/);
      const model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      const loaded = updateApp({ ...model, pendingAnalysisRequestId: 4 }, { type: "flight-analysis-loaded", requestId: 4, data }).model;
      const currentCursor = cursor(session, data);
      const previous = { ...loaded, analysisCursorSample: currentCursor, pendingAnalysisCursorRequestId: 7 };
      const rejected = updateApp(previous, { type: "flight-analysis-cursor-loaded", requestId: 7, sample: cursor(other, otherData) });
      expect(rejected.model.analysisCursorSample).toBe(currentCursor);
      expect(rejected.model.flightAnalysis).toBe(data);
      expect(rejected.model.pendingAnalysisCursorRequestId).toBeNull();
      expect(rejected.model.status).toContain("一致しない");
      expect(rejected.effects).toEqual([]);
      const rejectedData = updateApp({ ...previous, pendingAnalysisRequestId: 8 }, {
        type: "flight-analysis-loaded", requestId: 8, data: { ...data, summary: otherData.summary }
      });
      expect(rejectedData.model.flightAnalysis).toBe(data);
      expect(rejectedData.effects).toEqual([]);
      expect(updateApp({ ...previous, pendingAnalysisCursorRequestId: 9 }, {
        type: "flight-analysis-cursor-loaded", requestId: 7, sample: currentCursor
      }).model).toMatchObject({ pendingAnalysisCursorRequestId: 9 });
    } finally { session.dispose(); other.dispose(); }
  });

  it("preserves one-sample zero-duration records and does not generate a missing score or wind", () => {
    const session = result(0);
    try {
      const loaded = dataset(session);
      const data: NamedAnalysisDataset = { ...loaded, windGrid: { kind: "unavailable", reason: "not_requested" } };
      expect(projectAnalysisView(data)).toMatchObject({ samples: [expect.objectContaining({ timeSeconds: 0 })],
        summary: { durationSeconds: 0 }, windGrid: { kind: "unavailable", reason: "not_requested" } });
      expect(resolveReplayCameraMode("auto", data, 0, "screen")).toBe("pilot");
      expect(resolveAttractCameraMode(data, 0, "phone-vr")).toBe("pilot");
    } finally { session.dispose(); }
  });

  it.each([1, 2, 3, 4, 5])("retains v%i saved legacy controls and missing metrics in named Replay", (version) => {
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 81, 82), physics_hz());
    try {
      const saved = JSON.parse(archive(version)) as Record<string, unknown>;
      (saved.finalization as Record<string, unknown>).score_m = null;
      for (const sample of saved.samples as Record<string, unknown>[]) (sample.telemetry as Record<string, unknown>).angle_of_attack_rad = null;
      session.openArchive(JSON.stringify(saved));
      const data = dataset(session);
      const view = projectAnalysisView(data);
      expect(data.samples.every((sample) => sample.controls.layout === "legacy_three_axis")).toBe(true);
      expect(view.origin).toMatchObject({ kind: "named_record", context: { phase: "replay", controlLayout: "legacy_three_axis", controlIdentity: null } });
      expect(view.summary.score).toEqual({ kind: "unavailable", reason: "score_not_recorded" });
      expect(view.summary.maximumAngleOfAttackRadians).toEqual({ kind: "unavailable", reason: "no_defined_sample" });
      expect(projectAnalysisCursor(cursor(session, data, 0.005), data).timeSeconds).toBe(0.005);
    } finally { session.dispose(); }
  });

  it("shows unknown environment and original saved failure without selecting the player's map or zero wind", () => {
    const source = result();
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 81, 82), physics_hz());
    try {
      const saved = JSON.parse(source.exportRecordJson()) as Record<string, unknown>;
      const header = saved.header as Record<string, unknown>;
      header.environment_version = 99;
      header.scenario_id = 999;
      Object.assign(saved.finalization as Record<string, unknown>, { reason: "out_of_valid_envelope", disposition: "failed", failure: {
        dynamics: { load: { aerodynamic: { hybrid: { site: { proxy: { surface: "horizontal_tail", index: 1 } },
          cause: { wind: "outside_grid" }, limit: null, stage: "second" } } } }
      } });
      session.openArchive(JSON.stringify(saved));
      const data = dataset(session);
      const view = projectAnalysisView(data);
      expect(view.windGrid).toEqual({ kind: "unavailable", reason: "unregistered_environment_identity" });
      expect(view.origin).toMatchObject({ context: { finalization: { failure: { kind: "dynamics" } } } });
      expect(analysisScenarioId(data, 1)).toBe(999);
      const gameSession = gameSessionState(9, 0, session.queryRecordDisplay(0), false, null, "tail_incidence");
      if (gameSession === null) throw new Error("Expected the Rust Replay display");
      for (const mode of ["screen", "phone-vr", "webxr"] as const) {
        const model = { ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode } as const,
          flightAnalysis: data, analysisCursorSample: cursor(session, data), replayViewMode: "analysis" as const };
        const ui = createGameViewModel(model, null);
        validateUiViewModel(ui);
        const controls = ui.panels.flatMap((panel) => panel.controls);
        const readout = controls.find((control) => control.id === "game-replay-analysis-cursor-values");
        if (readout?.kind !== "status") throw new Error("Expected the saved cursor and wind readout");
        expect(readout.value).toContain("unregistered_environment_identity");
        const chart = controls.find((control) => control.kind === "chart");
        if (chart?.kind !== "chart") throw new Error("Expected the saved Analysis chart");
        expect(chart.series.some((series) => series.label.includes("彦根"))).toBe(false);
      }
    } finally { source.dispose(); session.dispose(); }
  });

  it("uses named demo final score separately from cursor position and applies the same camera policy in VR", () => {
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 81, 82), physics_hz());
    try {
      session.executeOperation("enter-attract");
      const data = dataset(session);
      const cursorSample = cursor(session, data);
      const gameSession = gameSessionState(10, 0, session.queryRecordDisplay(0), false, null, "tail_incidence");
      if (gameSession === null) throw new Error("Expected the Rust Attract display");
      const model = { ...createInitialAppModel(), gameSession, flightAnalysis: data, analysisCursorSample: cursorSample };
      const distance = createGameViewModel(model, null).panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-attract-distance");
      const expected = data.summary.scoreMeters.kind === "available" ? `${data.summary.scoreMeters.value.courseParallelMeters.toFixed(1)} m` : "距離未記録";
      expect(distance).toMatchObject({ label: "確定距離 / 保存時刻", value: `${expected} · t 0.00 s` });
      expect(resolveAttractCameraMode(data, 0, "screen")).toBe("platform");
      expect(resolveAttractCameraMode(data, 0, "webxr")).toBe("pilot");
      const resultSession = result();
      try {
        const phaseMismatch = updateApp({ ...model, pendingAnalysisRequestId: 5 }, {
          type: "flight-analysis-loaded", requestId: 5, data: dataset(resultSession)
        });
        expect(phaseMismatch.model.flightAnalysis).toBe(data);
        expect(phaseMismatch.model.status).toContain("phase");
      } finally { resultSession.dispose(); }
    } finally { session.dispose(); }
  });

  it("refreshes named Result and Replay contexts while retaining the shared cursor without persisting twice", () => {
    const session = result();
    try {
      const data = dataset(session);
      let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
      model = { ...model, flightAnalysis: data, analysisCursorTimeSeconds: 0.015, analysisCursorSample: cursor(session, data, 0.015), pendingGameRequestId: 5 };
      session.executeOperation("enter-replay");
      const display = session.queryRecordDisplay(0.015);
      if (display.kind !== "tail_record") throw new Error("Expected the current two-tail record");
      const projection = { controlLayout: "tail_incidence" as const, phaseCode: 9 as const, countdownRemaining: 0,
        controlModeCode: 0, difficulty: model.difficulty, configurationMetadata: model.configurationMetadata, canResume: false,
        display: { kind: "available" as const, value: display } };
      const replay = updateApp(model, { type: "game-operation-completed", requestId: 5, ...projection });
      expect(replay.model.flightAnalysis).toBeNull();
      expect(replay.model.analysisCursorSample).toBeNull();
      expect(replay.model.analysisCursorTimeSeconds).toBe(0.015);
      expect(replay.effects).toContainEqual({ type: "load-flight-analysis", requestId: model.nextAnalysisRequestId });
      expect(replay.effects.some((effect) => effect.type === "persist-flight-record")).toBe(false);
      const replayData = dataset(session);
      const loaded = updateApp(replay.model, { type: "flight-analysis-loaded", requestId: model.nextAnalysisRequestId, data: replayData });
      expect(loaded.model.flightAnalysis).toBe(replayData);
      session.executeOperation("leave-replay");
      const returned = updateApp({ ...loaded.model, pendingGameRequestId: 6 }, {
        type: "game-operation-completed", requestId: 6, ...session.readGameSessionProjection()
      });
      expect(returned.model.flightAnalysis).toBeNull();
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
      model = { ...model, flightAnalysis: data, analysisCursorTimeSeconds: 0.015, pendingGameRequestId: 5 };
      session.executeOperation("enter-replay");
      const display = session.queryRecordDisplay(0.015);
      if (display.kind !== "tail_record") throw new Error("Expected the current two-tail record");
      let replay = updateApp(model, { type: "game-operation-completed", requestId: 5,
        controlLayout: "tail_incidence", phaseCode: 9, countdownRemaining: 0, controlModeCode: 0,
        difficulty: model.difficulty, configurationMetadata: model.configurationMetadata, canResume: false,
        display: { kind: "available", value: display } }).model;
      const oldRequestId = replay.pendingAnalysisRequestId;
      if (oldRequestId === null) throw new Error("Expected the named Replay load request");
      const replayData = dataset(session);
      if (loadingState === "failed") replay = updateApp(replay, {
        type: "flight-analysis-failed", requestId: oldRequestId, message: "injected query failure"
      }).model;
      session.executeOperation("leave-replay");
      const returned = updateApp({ ...replay, pendingGameRequestId: 6 }, {
        type: "game-operation-completed", requestId: 6, ...session.readGameSessionProjection()
      });
      const freshRequestId = returned.model.pendingAnalysisRequestId;
      expect(freshRequestId).not.toBeNull();
      expect(freshRequestId).not.toBe(oldRequestId);
      expect(returned.effects).toEqual([{ type: "load-flight-analysis", requestId: freshRequestId }]);
      expect(returned.model.analysisCursorTimeSeconds).toBe(0.015);
      expect(updateApp(returned.model, { type: "flight-analysis-loaded", requestId: oldRequestId, data: replayData }).model).toBe(returned.model);
      if (freshRequestId === null) throw new Error("Expected a fresh Result request");
      const resultData = dataset(session);
      const loaded = updateApp(returned.model, { type: "flight-analysis-loaded", requestId: freshRequestId, data: resultData });
      expect(loaded.model.flightAnalysis).toBe(resultData);
      expect(loaded.effects.some((effect) => effect.type === "persist-flight-record")).toBe(false);
    } finally { session.dispose(); }
  });
});
