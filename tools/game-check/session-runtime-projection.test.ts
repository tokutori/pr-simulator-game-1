import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { projectRuntimePlaybackClock, queryRuntimeRecordPose, readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { parseRuntimeEnvironmentSnapshot } from "../../web/src/game/runtime-environment.js";
import { venueMapForEnvironment } from "../../web/src/game/biwa-venue-map.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import { NO_ENVIRONMENT_BRIEFING } from "../../web/src/game/environment-briefing.js";
import { NO_HEAD_HUD_VIEW } from "../../web/src/presentation/head-hud-view.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function prepared() {
  const session = createAppSession({ controlLayout: "tail_incidence", controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  session.executeOperation("open-setup");
  session.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: 2 });
  session.executeOperation("prepare");
  return session;
}

function launch(session: TailAppSessionFacade): void {
  session.executeOperation("start-flight");
  expect(session.advanceCountdown()).toBe(2);
  expect(session.advanceCountdown()).toBe(1);
  expect(session.advanceCountdown()).toBe(0);
  session.launch();
}

function jsonObject(json: string): Record<string, unknown> {
  return JSON.parse(json) as Record<string, unknown>;
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

describe("application runtime session projection", () => {
  it("projects Rust menu, live, paused and terminal phases with the original tail identity", () => {
    const session = prepared();
    try {
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phaseCode: 3, controlLayout: "tail_incidence", display: { kind: "unavailable" } });
      launch(session);
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phaseCode: 5, display: { value: { kind: "tail_flight", phaseCode: 5 } } });
      session.executeOperation("pause");
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phaseCode: 6, display: { value: { phaseCode: 6 } } });
      session.executeOperation("abort");
      const projection = readRuntimeSessionProjection(session);
      expect(projection).toMatchObject({ phaseCode: 7, display: { value: { kind: "tail_result" } } });
      const summary = session.readRecordSummary();
      if (projection.phaseCode !== 7 || projection.display.kind !== "available") throw new Error("Expected the Rust terminal display");
      expect(projection.display.value.finalization).toEqual(summary.context.finalization);
    } finally { session.dispose(); }
  });

  it.each(["replay", "attract"] as const)("uses only named %s queries and its Rust clock", (phase) => {
    const session = prepared();
    try {
      if (phase === "replay") { launch(session); session.executeOperation("abort"); session.executeOperation("enter-replay"); }
      else { session.executeOperation("cancel-briefing"); session.executeOperation("return-to-title"); session.executeOperation("enter-attract"); }
      const projection = readRuntimeSessionProjection(session);
      expect(projection).toMatchObject({ phaseCode: phase === "replay" ? 9 : 10, display: { value: { kind: "tail_record" } } });
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const clock = session.setPlaybackPlaying(false);
      const queried = queryRuntimeRecordPose(session, dataset, clock.timeSeconds);
      expect(queried.display).toMatchObject({ kind: "tail_record", stamp: { timeSeconds: clock.timeSeconds } });
      expect(queried.pose.controls?.layout).toBe("tail_incidence");
      expect(projectRuntimePlaybackClock(clock)).toEqual({ timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: false });
    } finally { session.dispose(); }
  });

  it("preserves the legacy saved controls in a tail-owned archive without packed snapshot conversion", () => {
    const legacy = new GameSessionBridge(0);
    const session = prepared();
    try {
      legacy.open_setup(); legacy.prepare(); legacy.mark_briefing_ready();
      legacy.start_countdown(1); legacy.advance_countdown(); legacy.launch(); legacy.advance_tick(0, 0, 0, 0); legacy.abort();
      session.executeOperation("cancel-briefing"); session.executeOperation("return-to-title");
      session.openArchive(legacy.export_flight_record_json());
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phaseCode: 9, controlLayout: "tail_incidence", display: { value: { kind: "legacy_record" } } });
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const queried = queryRuntimeRecordPose(session, dataset, 0.005);
      expect(queried.pose.controls?.layout).toBe("legacy_three_axis");
      expect(queried.display.pilotPositionTargetNormalized.kind).toBe("unavailable");
    } finally { session.dispose(); legacy.free(); }
  });
});

describe("registered environment and venue projection", () => {
  it("restricts no-selection to the unprepared Title and rejects surplus availability fields", () => {
    const document = { schema_version: 1, context: { kind: "session", phase_code: 0 }, projection: { kind: "no_selection" } };
    expect(parseRuntimeEnvironmentSnapshot(JSON.stringify(document), 0)).toEqual({ kind: "unavailable", reason: "no_selection" });
    for (const phase of [1, 3, 7, 9, 10]) {
      expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify({ ...document, context: { kind: "session", phase_code: phase } }), phase)).toThrow(RangeError);
    }
    expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify({ ...document, projection: { kind: "no_selection", source: "record" } }), 0)).toThrow(RangeError);
    const session = prepared();
    try {
      const snapshot = session.readSnapshot();
      if (snapshot.identity.kind !== "prepared") throw new Error("Expected a sealed identity");
      const expectedIdentity = snapshot.identity.scenario;
      expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(document), 0, expectedIdentity)).toThrow(RangeError);
      const malformed = jsonObject(session.readEnvironmentJson());
      for (const phase of [0, -1, 11, 2.5]) {
        const wrongPhase = { ...malformed, context: { kind: "session", phase_code: phase } };
        expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(wrongPhase), phase)).toThrow(RangeError);
      }
      nested(nested(malformed, "projection"), "metadata").local_frame = { kind: "unavailable", value: { latitude_degrees: 0 } };
      expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(malformed), 3)).toThrow(RangeError);
    } finally { session.dispose(); }
  });

  it("projects the registered venue in Screen/VR and rejects an actual unknown same-ID archive", () => {
    const session = prepared();
    try {
      launch(session);
      session.executeOperation("abort");
      const saved = jsonObject(session.exportRecordJson());
      for (const unknown of [false, true]) {
        if (unknown) { session.executeOperation("leave-replay"); nested(saved, "header").environment_version = 99; }
        session.openArchive(JSON.stringify(saved));
        const data = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
        const environment = parseRuntimeEnvironmentSnapshot(session.readEnvironmentJson(), 9, data.context.scenario);
        expect(environment.kind).toBe(unknown ? "unavailable" : "available");
        if (unknown) expect(environment).toEqual({ kind: "unavailable", reason: "unregistered_environment_identity" });
        else expect(environment).toMatchObject({ kind: "available", value: { sky: { kind: "available", value: {
          condition: { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, visibilityMeters: 25_000 }
        } } } });
        const venue = venueMapForEnvironment(environment);
        expect(venue.kind).toBe(unknown ? "unavailable" : "available");
        const state = updateApp(createInitialAppModel(), { type: "game-session-synced", ...readRuntimeSessionProjection(session) }).model;
        for (const mode of ["screen", "phone-vr", "webxr"] as const) {
          const view = createGameViewModel({ ...state, presentation: { type: "ready", mode }, flightAnalysis: data, replayViewMode: "analysis" },
            null, data, NO_HEAD_HUD_VIEW, NO_ENVIRONMENT_BRIEFING, venue);
          const chart = view.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "chart");
          if (chart?.kind !== "chart") throw new Error("Expected the recorded trajectory chart");
          if (unknown) expect(chart.series).toHaveLength(1);
          else expect(chart.series.length).toBeGreaterThan(1);
        }
      }
    } finally { session.dispose(); }
  });

  it("connects the registered scenario-six waves and full local NED origin", () => {
    const session = prepared();
    try {
      const snapshot = session.readSnapshot();
      if (snapshot.identity.kind !== "prepared") throw new Error("Expected a sealed scenario");
      expect(snapshot.identity.scenario.scenarioId).toBe(6);
      const projection = parseRuntimeEnvironmentSnapshot(session.readEnvironmentJson(), 3, snapshot.identity.scenario);
      expect(projection).toMatchObject({ kind: "available", value: { identity: snapshot.identity.scenario, localFrame: { kind: "available" } } });
      expect(venueMapForEnvironment(projection)).toMatchObject({ kind: "available", value: { origin: "launch-origin-wgs84-35.294075-136.254448" } });
    } finally { session.dispose(); }
  });

  it.each(["identity", "phase", "origin", "datum", "unavailable"] as const)("does not reuse a venue for mismatched %s", (change) => {
    const session = prepared();
    try {
      const snapshot = session.readSnapshot();
      if (snapshot.identity.kind !== "prepared") throw new Error("Expected a sealed scenario");
      const expectedIdentity = snapshot.identity.scenario;
      const document = jsonObject(session.readEnvironmentJson());
      const projection = nested(document, "projection");
      if (change === "identity") nested(projection, "identity").seed_low = expectedIdentity.seedLow + 1;
      if (change === "phase") nested(document, "context").phase_code = 9;
      if (change === "origin") nested(nested(nested(projection, "metadata"), "local_frame"), "value").longitude_degrees = 137;
      if (change === "datum") nested(nested(nested(projection, "metadata"), "local_frame"), "value").water_level_datum = "different water surface";
      if (change === "unavailable") { projection.kind = "unavailable"; delete projection.metadata; }
      const read = () => parseRuntimeEnvironmentSnapshot(JSON.stringify(document), 3, expectedIdentity);
      if (change === "identity" || change === "phase") expect(read).toThrow(RangeError);
      else expect(venueMapForEnvironment(read()).kind).toBe("unavailable");
    } finally { session.dispose(); }
  });
});
