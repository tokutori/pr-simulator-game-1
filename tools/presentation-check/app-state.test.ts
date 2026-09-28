import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";

describe("Boot application state", () => {
  it("initializes once and accepts only the matching completion", () => {
    const initial = createInitialAppModel();
    const initialized = updateApp(initial, { type: "initialize" });
    expect(initialized.model.presentation).toEqual({ type: "initializing", requestId: 1 });
    expect(initialized.effects).toEqual([{ type: "initialize-presentation", requestId: 1 }]);
    expect(updateApp(initialized.model, { type: "initialize" })).toEqual({ model: initialized.model, effects: [] });

    const stale = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 2,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "ready"
    });
    expect(stale.model).toBe(initialized.model);

    const completed = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 1,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "Screen active; WebXR supported"
    });
    expect(completed.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(completed.effects).toEqual([{ type: "load-stored-flight-records", requestId: 1 }]);
    expect(createBootViewModel(completed.model).panels[0]?.controls.find((control) => control.id === "boot-enter-webxr")?.enabled).toBe(true);
  });

  it("loads only the latest stored-record index response", () => {
    const ready = readyModel();
    const refreshed = updateApp(ready, { type: "refresh-stored-flight-records" });
    expect(refreshed.effects).toEqual([{ type: "load-stored-flight-records", requestId: 2 }]);
    const stale = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 1, records: [{ id: 8, savedAt: "2026-09-28T00:00:00.000Z" }]
    });
    expect(stale.model).toBe(refreshed.model);
    const loaded = updateApp(refreshed.model, {
      type: "stored-flight-records-loaded", requestId: 2, records: [{ id: 9, savedAt: "2026-09-28T00:00:00.000Z" }]
    });
    expect(loaded.model.storedFlightRecords.map((record) => record.id)).toEqual([9]);
  });

  it("opens a selected stored record through the Rust-owned Replay phase", () => {
    const ready = updateApp({ ...readyModel(), gamePhaseCode: 0 }, {
      type: "stored-flight-records-loaded", requestId: 1,
      records: [{ id: 4, savedAt: "2026-09-28T00:00:00.000Z" }]
    }).model;
    const opening = updateApp(ready, {
      type: "ui-action", action: { type: "activate", controlId: "game-title-open-record-4" }
    });
    expect(opening.effects).toEqual([{ type: "open-stored-flight-record", id: 4, requestId: 2 }]);
    const entered = updateApp(opening.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 9, controlModeCode: 0,
      difficulty: opening.model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(entered.effects).toEqual([{ type: "load-flight-analysis", requestId: 1 }]);
    expect(entered.model.gamePhaseCode).toBe(9);
    expect(updateApp(opening.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-title-open-record-4" }
    }).model).toBe(opening.model);
  });

  it("serializes permission requests and backend changes", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    expect(requested.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "requesting" });
    expect(requested.effects).toEqual([{ type: "request-permission", mode: "webxr", requestId: 2 }]);
    expect(createBootViewModel(requested.model).panels[0]?.controls.filter((control) => control.kind === "button" && control.id.startsWith("boot-enter-")).every((control) => !control.enabled)).toBe(true);

    const competing = updateApp(requested.model, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    expect(competing.model).toBe(requested.model);
    expect(competing.effects).toHaveLength(0);

    const permitted = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    expect(permitted.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "starting" });
    expect(permitted.effects).toEqual([{ type: "switch-backend", mode: "webxr", requestId: 2 }]);

    const active = updateApp(permitted.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "webxr",
      ok: true, message: "", successStatus: "WebXR active"
    });
    expect(active.model.presentation).toEqual({ type: "ready", mode: "webxr" });
    expect(active.model.status).toBe("WebXR active");
  });

  it("cancels a stale successful permission result", () => {
    const ready = readyModel();
    const transition = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    const stale = updateApp(transition.model, {
      type: "permission-completed", requestId: 1, mode: "webxr", ok: true, message: "granted"
    });
    expect(stale.model).toBe(transition.model);
    expect(stale.effects).toEqual([{ type: "cancel-pending-request", mode: "webxr" }]);
  });

  it("reports the backend actually active after a failed switch", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    const starting = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    const fallback = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "screen",
      ok: false, message: "Phone startup failed", successStatus: "WebXR active"
    });
    expect(fallback.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(fallback.model.status).toBe("Phone startup failed; Screen is active");

    const noBackend = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: null,
      ok: false, message: "Screen recovery failed", successStatus: "WebXR active"
    });
    expect(noBackend.model.presentation).toEqual({ type: "failed", message: "Screen recovery failed" });
  });

  it("restores Screen after an unexpected active backend end and ignores stale ends", () => {
    const active = { ...readyModel(), presentation: Object.freeze({ type: "ready" as const, mode: "webxr" as const }) };
    const recovery = updateApp(active, { type: "backend-ended", mode: "webxr", message: "Session ended" });
    expect(recovery.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "webxr", to: "screen", phase: "stopping" });
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: 2 }]);
    const screen = readyModel();
    const stale = updateApp(screen, { type: "backend-ended", mode: "webxr", message: "Late end" });
    expect(stale.model).toBe(screen);
  });

  it("cancels pending permissions and disposes presentation on page hide", () => {
    const pending = updateApp(readyModel(), { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    const hidden = updateApp(pending.model, { type: "page-hidden" });
    expect(hidden.model.presentation).toEqual({ type: "hidden" });
    expect(hidden.effects).toEqual([
      { type: "cancel-pending-request", mode: "webxr" },
      { type: "cancel-pending-request", mode: "phone-vr" },
      { type: "dispose-presentation" }
    ]);
    const latePermission = updateApp(hidden.model, {
      type: "permission-completed", requestId: 2, mode: "phone-vr", ok: true, message: "granted"
    });
    expect(latePermission.model).toBe(hidden.model);
    expect(latePermission.effects).toEqual([{ type: "cancel-pending-request", mode: "phone-vr" }]);
  });

  it("serializes game operations and ignores stale completions", () => {
    const title = { ...readyModel(), gamePhaseCode: 0 };
    const setup = updateApp(title, { type: "ui-action", action: { type: "activate", controlId: "game-title-start" } });
    expect(setup.model.pendingGameRequestId).toBe(2);
    expect(setup.effects).toEqual([{ type: "game-session-operation", operation: "open-setup", requestId: 2 }]);
    const competing = updateApp(setup.model, { type: "ui-action", action: { type: "activate", controlId: "game-title-start" } });
    expect(competing.model).toBe(setup.model);

    const completed = updateApp(setup.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 1, controlModeCode: 0,
      difficulty: { presetCode: 1, informationCode: 1, assistanceCode: 1, weatherCode: 2 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
    });
    expect(completed.model.gamePhaseCode).toBe(1);
    expect(completed.model.pendingGameRequestId).toBeNull();
    const stale = updateApp(completed.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 7, controlModeCode: 2,
      difficulty: { presetCode: 0, informationCode: 0, assistanceCode: 0, weatherCode: 1 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
    });
    expect(stale.model).toBe(completed.model);
  });

  it("projects the Rust-selected control mode after a Setup update", () => {
    const setupModel = { ...readyModel(), gamePhaseCode: 1 };
    const requested = updateApp(setupModel, { type: "ui-action", action: { type: "activate", controlId: "game-setup-assistance" } });
    expect(requested.effects).toEqual([{ type: "game-session-operation", operation: "cycle-assistance-level", requestId: 2 }]);
    const completed = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 1, controlModeCode: 2,
      difficulty: { presetCode: 4, informationCode: 0, assistanceCode: 0, weatherCode: 0 },
      configurationMetadata: null,
      countdownRemaining: 0, snapshot: null
    });
    expect(completed.model.controlModeCode).toBe(2);
    expect(completed.model.difficulty.assistanceCode).toBe(0);
  });

  it("retains resolved configuration metadata through Result projection", () => {
    const title = { ...readyModel(), gamePhaseCode: 7 };
    const configurationMetadata = {
      presetCode: 4, informationCode: 2, assistanceCode: 3, weatherCode: 4,
      catalogVersion: 1, scenarioId: 5, scenarioVersion: 1, aircraftModelVersion: 1,
      environmentVersion: 5, controllerProfileVersion: 4, seedLow: 0, seedHigh: 0
    };
    const completed = updateApp(title, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 2, assistanceCode: 3, weatherCode: 4 },
      configurationMetadata, countdownRemaining: 0, snapshot: null
    });
    expect(completed.model.configurationMetadata).toEqual(configurationMetadata);
  });

  it("persists and queries one record when the Rust session first enters Result", () => {
    const flight = { ...readyModel(), gamePhaseCode: 5 };
    const terminal = updateApp(flight, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(terminal.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
    expect(terminal.model.pendingAnalysisRequestId).toBe(1);
    const duplicate = updateApp(terminal.model, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: terminal.model.difficulty,
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(duplicate.effects).toEqual([]);
  });

  it("enters read-only Replay and rejects stale pose responses after seeking", () => {
    const flightAnalysis = Object.freeze({
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0.15,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const result: AppModel = {
      ...readyModel(),
      gamePhaseCode: 7,
      flightAnalysis,
      resultTab: "analysis",
      analysisCursorTimeSeconds: 0.5,
      pendingGameRequestId: null,
      nextRequestId: 2,
      nextReplayPoseRequestId: 1
    };
    const requested = updateApp(result, {
      type: "ui-action", action: { type: "activate", controlId: "game-result-replay" }
    });
    expect(requested.effects).toEqual([{ type: "game-session-operation", operation: "enter-replay", requestId: 2 }]);
    const entered = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 9, controlModeCode: 0,
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(entered.model.gamePhaseCode).toBe(9);
    expect(entered.model.resultTab).toBe("analysis");
    expect(entered.effects).toEqual([{ type: "load-flight-replay-pose", requestId: 1, timeSeconds: 0.5 }]);
    const sought = updateApp(entered.model, {
      type: "ui-action", action: { type: "set-range", controlId: "game-replay-cursor", value: 1.25 }
    });
    expect(sought.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 2, timeSeconds: 1.25 },
      { type: "load-flight-analysis-cursor", requestId: 1, timeSeconds: 1.25 }
    ]);
    const stale = updateApp(sought.model, {
      type: "flight-replay-pose-loaded", requestId: 1,
      pose: {
        datumPositionNed: { north: 0, east: 0, down: 0 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0, initialPilotPositionMeters: 0
      }
    });
    expect(stale.model.replayPose).toBeNull();
    expect(stale.model.pendingReplayPoseRequestId).toBe(2);
    const currentPose = updateApp(sought.model, {
      type: "flight-replay-pose-loaded", requestId: 2,
      pose: {
        datumPositionNed: { north: 12, east: 3, down: -8 },
        attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
        pilotPositionMeters: 0.2, initialPilotPositionMeters: 0.15
      }
    });
    const leave = updateApp(currentPose.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-return" }
    });
    expect(leave.effects).toEqual([{ type: "game-session-operation", operation: "leave-replay", requestId: 3 }]);
    const returned = updateApp(leave.model, {
      type: "game-operation-completed", requestId: 3, phaseCode: 7, controlModeCode: 0,
      difficulty: result.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(returned.model.gamePhaseCode).toBe(7);
    expect(returned.model.resultTab).toBe("analysis");
    expect(returned.model.analysisCursorTimeSeconds).toBe(1.25);
    expect(returned.model.replayPose).toBeNull();
    expect(returned.effects).toEqual([]);
  });

  it("plays the immutable record at selected speed and ignores invalidated clock ticks", () => {
    const flightAnalysis = Object.freeze({
      samples: Object.freeze([]),
      initialPilotPositionMeters: 0.15,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const replay: AppModel = {
      ...readyModel(),
      gamePhaseCode: 9,
      flightAnalysis,
      analysisCursorTimeSeconds: 0.5,
      pendingGameRequestId: null
    };
    const speed = updateApp(replay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-speed-2" }
    });
    expect(speed.model.replaySpeed).toBe(2);
    const play = updateApp(speed.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-play-pause" }
    });
    expect(play.model.replayPlaying).toBe(true);
    expect(play.effects).toEqual([{ type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 }]);
    const tick = updateApp(play.model, { type: "replay-clock-tick", generation: 1, elapsedSeconds: 0.25 });
    expect(tick.model.analysisCursorTimeSeconds).toBe(1);
    expect(tick.model.pendingReplayPoseRequestId).toBe(1);
    expect(tick.model.pendingAnalysisCursorRequestId).toBe(1);
    expect(tick.effects).toEqual([
      { type: "load-flight-replay-pose", requestId: 1, timeSeconds: 1 },
      { type: "load-flight-analysis-cursor", requestId: 1, timeSeconds: 1 },
      { type: "schedule-replay-clock-tick", generation: 1, delayMilliseconds: 50 }
    ]);
    const pause = updateApp(tick.model, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-play-pause" }
    });
    expect(pause.model.replayPlaying).toBe(false);
    const staleTick = updateApp(pause.model, { type: "replay-clock-tick", generation: 1, elapsedSeconds: 1 });
    expect(staleTick.model).toBe(pause.model);
  });

  it("stops playback exactly at record end", () => {
    const flightAnalysis = Object.freeze({
      samples: Object.freeze([]), initialPilotPositionMeters: 0,
      summary: Object.freeze({
        sampleCount: 2, durationSeconds: 2, maximumAltitudeMeters: 10,
        maximumAirspeedMetersPerSecond: 9, maximumGroundspeedMetersPerSecond: 10,
        maximumAngleOfAttackRadians: null, maximumAbsoluteRollRadians: 0, score: null,
        terminal: Object.freeze({ reason: "time-limit" as const, disposition: "complete" as const, timeSeconds: 2 })
      })
    });
    const replay: AppModel = {
      ...readyModel(), gamePhaseCode: 9, flightAnalysis,
      analysisCursorTimeSeconds: 1.9, replayPlaying: true, replaySpeed: 2, replayClockGeneration: 4
    };
    const ended = updateApp(replay, { type: "replay-clock-tick", generation: 4, elapsedSeconds: 0.1 });
    expect(ended.model.analysisCursorTimeSeconds).toBe(2);
    expect(ended.model.replayPlaying).toBe(false);
    expect(ended.model.replayClockGeneration).toBe(5);
    expect(ended.effects).toHaveLength(2);
  });

  it("allows Pilot and Chase selection only on the Screen backend", () => {
    const replay: AppModel = { ...readyModel(), gamePhaseCode: 9 };
    const chase = updateApp(replay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
    });
    expect(chase.model.replayCameraMode).toBe("chase");
    const vrReplay: AppModel = {
      ...replay,
      presentation: Object.freeze({ type: "ready", mode: "webxr" })
    };
    expect(updateApp(vrReplay, {
      type: "ui-action", action: { type: "activate", controlId: "game-replay-camera" }
    }).model).toBe(vrReplay);
  });

  it("persists an explicitly aborted flight on operation completion", () => {
    const paused = { ...readyModel(), gamePhaseCode: 6 };
    const requested = updateApp(paused, {
      type: "ui-action", action: { type: "activate", controlId: "game-paused-abort" }
    });
    const completed = updateApp(requested.model, {
      type: "game-operation-completed", requestId: 2, phaseCode: 7, controlModeCode: 0,
      difficulty: { presetCode: 4, informationCode: 0, assistanceCode: 3, weatherCode: 0 },
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    expect(completed.effects).toEqual([
      { type: "persist-flight-record" },
      { type: "load-flight-analysis", requestId: 1 }
    ]);
  });

  it("rejects stale Analysis responses after leaving Result", () => {
    const flight = { ...readyModel(), gamePhaseCode: 5 };
    const terminal = updateApp(flight, {
      type: "game-session-synced", phaseCode: 7, controlModeCode: 0,
      difficulty: flight.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    const setup = updateApp(terminal.model, {
      type: "game-session-synced", phaseCode: 1, controlModeCode: 0,
      difficulty: terminal.model.difficulty, configurationMetadata: null, countdownRemaining: 0, snapshot: null
    });
    const stale = updateApp(setup.model, {
      type: "flight-analysis-failed", requestId: 1, message: "stale"
    });
    expect(stale.model.gamePhaseCode).toBe(1);
    expect(stale.model.status).not.toContain("stale");
  });
});

function readyModel(): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Screen ready"
  });
  return ready.model;
}
