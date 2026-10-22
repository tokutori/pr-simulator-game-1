import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { projectRuntimePlaybackClock, readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { projectRuntimeEnvironment } from "../../web/src/game/runtime-environment.js";
import { projectRuntimeVenue } from "../../web/src/game/runtime-venue.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";
import { selectSharedReplayView, sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function prepared() {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "select_weather", weather: "typical" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  return session;
}

function launch(session: TailAppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  expect(session.observeCountdown(session.captureCountdownToken(), 1, true)).toEqual({ kind: "waiting", remainingSeconds: 2 });
  expect(session.observeCountdown(session.captureCountdownToken(), 1, true)).toEqual({ kind: "waiting", remainingSeconds: 1 });
  expect(session.observeCountdown(session.captureCountdownToken(), 1, true)).toEqual({ kind: "launched" });
}

function jsonObject(json: string): Record<string, unknown> {
  return JSON.parse(json) as Record<string, unknown>;
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

function sessionVenue(session: TailAppSessionFacade) {
  const environment = projectRuntimeEnvironment(session.readEnvironment());
  return { environment, venue: projectRuntimeVenue(environment) };
}

describe("application runtime session projection", () => {
  it("projects Rust menu, live, paused and terminal phases with the original tail identity", () => {
    const session = prepared();
    try {
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phase: "briefing_ready", display: { kind: "unavailable" } });
      launch(session);
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phase: "flight_running", display: { value: { phase: "flight_running" } } });
      session.executeSharedSessionCommand({ kind: "pause" });
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phase: "flight_paused", display: { value: { phase: "flight_paused" } } });
      session.executeSharedSessionCommand({ kind: "abort" });
      const projection = readRuntimeSessionProjection(session);
      expect(projection).toMatchObject({ phase: "result", display: { value: { phase: "result" } } });
      const summary = session.readRecordSummary();
      if (projection.phase !== "result" || projection.display.kind !== "available") throw new Error("Expected the Rust terminal display");
      expect(projection.display.value.frame.finalization).toEqual(summary.context.finalization.value);
    } finally { session.dispose(); }
  });

  it.each(["replay", "attract"] as const)("uses only named %s queries and its Rust clock", (phase) => {
    const session = prepared();
    try {
      if (phase === "replay") { launch(session); session.executeSharedSessionCommand({ kind: "abort" }); session.executeSharedSessionCommand({ kind: "enter_replay" }); }
      else { session.executeSharedSessionCommand({ kind: "cancel_briefing" }); session.executeSharedSessionCommand({ kind: "return_to_title" }); session.executeSharedSessionCommand({ kind: "enter_attract" }); }
      const projection = readRuntimeSessionProjection(session);
      expect(projection).toMatchObject({ phase, display: { value: { controls: { layout: "tail_incidence" } } } });
      const clock = session.setPlaybackPlaying(false);
      const cursor = session.queryRecordSample(clock.timeSeconds);
      expect(cursor.flight_time_s).toBe(clock.timeSeconds);
      expect(session.queryReplayCamera(clock.timeSeconds, "auto", true).controls.layout).toBe("tail_incidence");
      expect(projectRuntimePlaybackClock(clock)).toEqual({ timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: false });
    } finally { session.dispose(); }
  });

  it("preserves current saved controls in an imported archive without resimulation", () => {
    const archived = new HybridGameSessionBridge(0, 21, 22);
    const session = prepared();
    try {
      archived.open_setup(); archived.prepare(); archived.mark_briefing_ready();
      archived.start_countdown(1); archived.observe_countdown(archived.countdown_token(), 1, true);
      archived.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.05, turnRight: -0.05,
        desiredPitchRateRadiansPerSecond: 0.01, desiredYawRateRadiansPerSecond: -0.01,
        pilotPositionCommand: { kind: "set", normalized: 0.25 } }));
      archived.abort();
      session.executeSharedSessionCommand({ kind: "cancel_briefing" }); session.executeSharedSessionCommand({ kind: "return_to_title" });
      const saved = archived.export_flight_record_json();
      session.openArchive(saved);
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "registered_origin" });
      expect(readRuntimeSessionProjection(session)).toMatchObject({ phase: "replay", display: { value: { controls: { layout: "tail_incidence" } } } });
      const samples = session.readAnalysisSamples();
      const clock = session.readPlaybackClock();
      const cursor = session.queryRecordSample(0.005);
      const pose = session.queryReplayCamera(0.005, "auto", true);
      const terminalSample = samples.at(-1);
      if (terminalSample === undefined) throw new Error("Expected the saved terminal control sample");
      expect(terminalSample.controls.physical_incidence.horizontal_tail_rad).not.toBe(0);
      expect(cursor.controls).toEqual(terminalSample.controls);
      expect(pose.controls.physicalIncidence.horizontalTailRadians).toBe(terminalSample.controls.physical_incidence.horizontal_tail_rad);
      expect(cursor).not.toHaveProperty("pilot_position_target_normalized");
      expect(session.readPlaybackClock()).toEqual(clock);
      expect(session.readAnalysisSamples()).toEqual(samples);
      expect(archived.export_flight_record_json()).toBe(saved);
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "title_exhibition" });
      session.executeSharedSessionCommand({ kind: "enter_attract" });
      expect(sessionVenue(session).environment).toMatchObject({ kind: "available", value: { source: "attract" } });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
    } finally { session.dispose(); archived.free(); }
  });
});

describe("registered environment and venue projection", () => {
  it.each(["calm", "mild"] as const)("preserves current weather %s venue through flight, Result, Replay and Retry but hides its imported record", (weather) => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "title_exhibition" });
      session.executeSharedSessionCommand({ kind: "open_setup" });
      session.executeSharedSessionCommand({ kind: "select_weather", weather });
      const selected = sessionVenue(session);
      expect(selected.environment).toMatchObject({ kind: "available", value: { source: "selected", localFrame: { kind: "unavailable" } } });
      expect(selected.venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "prepare" });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "start_countdown" });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.observeCountdown(session.captureCountdownToken(), 1, true); session.observeCountdown(session.captureCountdownToken(), 1, true); session.observeCountdown(session.captureCountdownToken(), 1, true);
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "pause" });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "abort" });
      expect(sessionVenue(session).environment).toMatchObject({ kind: "available", value: { source: "sealed" } });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      const saved = session.exportRecordJson();
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      expect(sessionVenue(session).environment).toMatchObject({ kind: "available", value: { source: "record" } });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      session.executeSharedSessionCommand({ kind: "retry" });
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "shared_launch" });
      session.executeSharedSessionCommand({ kind: "cancel_briefing" }); session.executeSharedSessionCommand({ kind: "return_to_title" });
      session.openArchive(saved);
      expect(sessionVenue(session).environment).toMatchObject({ kind: "available", value: { source: "archive", localFrame: { kind: "unavailable" } } });
      expect(sessionVenue(session).venue).toEqual({ kind: "unavailable", reason: "origin_not_recorded" });
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(session.readLifecycle().phase).toBe("title");
      expect(sessionVenue(session).venue).toEqual({ kind: "visible", basis: "title_exhibition" });
    } finally { session.dispose(); }
  });

  it("keeps the independent default Attract venue without inventing a recorded origin", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      session.executeSharedSessionCommand({ kind: "enter_attract" });
      const projected = sessionVenue(session);
      expect(projected.environment).toMatchObject({ kind: "available", value: { source: "attract", localFrame: { kind: "unavailable" } } });
      expect(projected.venue).toEqual({ kind: "visible", basis: "shared_launch" });
    } finally { session.dispose(); }
  });

  it("projects the registered venue in Screen/VR and rejects an actual unknown same-ID archive", () => {
    const session = prepared();
    try {
      launch(session);
      session.executeSharedSessionCommand({ kind: "abort" });
      const saved = jsonObject(session.exportRecordJson());
      for (const unknown of [false, true]) {
        if (unknown) { session.executeSharedSessionCommand({ kind: "leave_replay" }); nested(saved, "header").environment_version = 99; }
        session.openArchive(JSON.stringify(saved));
        const environment = projectRuntimeEnvironment(session.readEnvironment());
        expect(environment.kind).toBe(unknown ? "unavailable" : "available");
        if (unknown) expect(environment).toMatchObject({ kind: "unavailable", reason: "unregistered_environment_identity", source: "archive", identity: { environment_version: 99 } });
        else expect(environment).toMatchObject({ kind: "available", value: { sky: { kind: "available", value: {
          condition: { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, visibilityMeters: 25_000 }
        } } } });
        expect(projectRuntimeVenue(environment)).toEqual(unknown ? { kind: "unavailable", reason: "environment_unavailable" }
          : { kind: "visible", basis: "registered_origin" });
        const platform = sharedGameUiPlatform();
        selectSharedReplayView(session, platform, "analysis");
        for (const mode of ["screen", "phone-vr", "webxr"] as const) {
          const view = createGameViewModel(session.readSharedUi({ ...platform,
            presentation_mode: mode === "phone-vr" ? "phone_vr" : mode === "webxr" ? "hmd" : mode }));
          const notice = view.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-state");
          if (unknown) {
            if (notice?.kind !== "status") throw new Error("Expected the environment notice");
            expect(notice.value).toContain("直前の背景");
          }
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
      expect(snapshot.scenario?.scenario_id).toBe(6);
      const projection = projectRuntimeEnvironment(session.readEnvironment());
      expect(projection).toMatchObject({ kind: "available", value: { identity: snapshot.scenario, localFrame: { kind: "defined" } } });
      expect(projectRuntimeVenue(projection)).toEqual({ kind: "visible", basis: "shared_launch" });
    } finally { session.dispose(); }
  });

  it.each(["origin_not_recorded", "unregistered_origin"] as const)("projects shared venue rejection %s", (reason) => {
    const session = prepared();
    try {
      launch(session); session.executeSharedSessionCommand({ kind: "abort" }); session.openArchive(session.exportRecordJson());
      const document = session.readEnvironment();
      if (document.projection.kind !== "available") throw new Error("Expected the registered environment");
      const environment = projectRuntimeEnvironment({ ...document, projection: { ...document.projection, venue: { kind: reason } } });
      expect(projectRuntimeVenue(environment)).toEqual({ kind: "unavailable", reason });
    } finally { session.dispose(); }
  });

});
