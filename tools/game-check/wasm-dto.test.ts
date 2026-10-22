import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { TailSnapshotDocument } from "../../web/src/generated/tail-snapshot.js";
import type { FlightFrameDocument } from "../../web/src/generated/flight-frame.js";
import type { FlightKeyObservation } from "../../web/src/generated/flight-key.js";
import type { TailControlProfileDocument } from "../../web/src/generated/tail-control-profile.js";
import type { EnvironmentSnapshot } from "../../web/src/generated/runtime-environment.js";
import type { RecordedPlaybackSample } from "../../web/src/generated/record-sample.js";
import type { AnalysisDocument } from "../../web/src/generated/record-analysis.js";
import type { SummaryDocument } from "../../web/src/generated/record-summary.js";
import type { WindDocument } from "../../web/src/generated/record-wind.js";
import type { ReplayCameraView } from "../../web/src/generated/replay-camera.js";
import { projectRuntimeEnvironment } from "../../web/src/game/runtime-environment.js";
import { contractValidator, readContract } from "../game-ui-build/validate.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral = { controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" as const } };
const wind = { north_min_m: -10, east_min_m: -10, altitude_m: 10, spacing_m: 5 };

function fixture(mode = 0) {
  const bridge = new HybridGameSessionBridge(mode, 21, 22);
  return { bridge, session: new TailAppSessionFacade(bridge, physics_hz()) };
}

function launch(session: TailAppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "select_weather", weather: "calm" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  session.observeCountdown(session.captureCountdownToken(), 3, true);
}

describe("same-build Rust/WASM generated DTO connections", () => {
  it.each([0, 1, 2])("exposes the sealed control profile in prepared and live phases (%s)", (mode) => {
    const { bridge, session } = fixture(mode);
    try {
      expect(() => bridge.control_profile_json()).toThrow();
      session.executeSharedSessionCommand({ kind: "open_setup" });
      session.executeSharedSessionCommand({ kind: "prepare" });
      const profile = readContract<TailControlProfileDocument>("tail_profile", bridge.control_profile_json());
      expect(profile.schema_version).toBe(2);
      session.executeSharedSessionCommand({ kind: "start_countdown" });
      session.observeCountdown(session.captureCountdownToken(), 3, true);
      expect(readContract<TailControlProfileDocument>("tail_profile", bridge.control_profile_json())).toEqual(profile);
      session.executeSharedSessionCommand({ kind: "abort" });
      session.executeSharedSessionCommand({ kind: "return_to_title" });
      expect(() => bridge.control_profile_json()).toThrow();
    } finally { session.dispose(); }
  });

  it("connects shared frame admission, final interpolation interval and key events through the actual WASM ABI", () => {
    const { bridge, session } = fixture();
    const frame = (timestamp: number): FlightFrameDocument => readContract<FlightFrameDocument>(
      "flight_frame", bridge.advance_flight_frame_json(timestamp, "[]", true));
    const key = (pressed: boolean, repeat: boolean): void => {
      const observation: FlightKeyObservation = { control: "pilot_increase", pressed, repeat };
      bridge.observe_flight_key_json(JSON.stringify(observation));
    };
    try {
      launch(session);
      key(true, false);
      expect(frame(1000).advance).toMatchObject({ kind: "ticks", count: 0 });
      const advanced = frame(1035);
      expect(advanced.advance).toEqual({ kind: "ticks", count: 3, interpolation_fraction: 0.5 });
      expect(advanced.render_pose?.simulationTimeSeconds).toBe(0.025);
      expect(advanced.render_pose?.airspeedMetersPerSecond).toEqual(expect.any(Number));
      const idle = frame(1038);
      expect(idle.advance).toMatchObject({ kind: "ticks", count: 0 });
      expect(idle.render_pose?.simulationTimeSeconds).toBeCloseTo(0.028, 12);
      expect(idle.snapshot?.render_pose?.simulationTimeSeconds).toBe(0.03);
      const beforePause = session.readSnapshot();
      if (beforePause.frame.kind !== "flight") throw new Error("Expected a live frame");
      session.executeSharedSessionCommand({ kind: "pause" });
      session.executeSharedSessionCommand({ kind: "resume" });
      key(true, true);
      const initialResume = frame(9000);
      expect(initialResume.advance).toMatchObject({ kind: "ticks", count: 0 });
      expect(initialResume.render_pose).toEqual(initialResume.snapshot?.render_pose);
      const resumed = frame(9010);
      if (resumed.snapshot?.frame.kind !== "flight") throw new Error("Expected a resumed live frame");
      expect(resumed.snapshot.frame.state.pilot_position_target_normalized)
        .toBe(beforePause.frame.state.pilot_position_target_normalized);
      expect(frame(9160).advance).toMatchObject({ kind: "ticks", count: 15 });
      expect(frame(9310.001).advance).toEqual({ kind: "processing_delay", blocked: true });
      expect(session.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: false });
      expect(frame(9320.001).advance).toEqual({ kind: "processing_delay", blocked: false });
      expect(session.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: true });
    } finally { session.dispose(); }
  });

  it("keeps named lifecycle phases and the shared elapsed-time countdown", () => {
    const { bridge, session } = fixture();
    try {
      expect(session.readLifecycle()).toMatchObject({ phase: "title", control_mode_code: 0, can_resume: false });
      const menu = readContract<TailSnapshotDocument>("tail_snapshot", bridge.snapshot_json());
      expect(menu).toMatchObject({ phase: "title", scenario: null, control_identity: null, render_pose: null, frame: { kind: "menu" } });
      const validate = contractValidator<TailSnapshotDocument>("tail_snapshot");
      const omitted: Record<string, unknown> = { ...menu };
      delete omitted.control_identity;
      expect(validate(omitted)).toBe(false);
      expect(validate({ ...menu, surplus: true })).toBe(false);
      session.executeSharedSessionCommand({ kind: "open_setup" });
      expect(session.readLifecycle().phase).toBe("flight_setup");
      session.executeSharedSessionCommand({ kind: "prepare" });
      expect(session.readLifecycle().phase).toBe("briefing_ready");
      session.executeSharedSessionCommand({ kind: "start_countdown" });
      expect(session.observeCountdown(session.captureCountdownToken(), 0.8, true)).toEqual({ kind: "waiting", remainingSeconds: 3 });
      expect(session.observeCountdown(session.captureCountdownToken(), 0.2, true)).toEqual({ kind: "waiting", remainingSeconds: 2 });
      expect(session.observeCountdown(session.captureCountdownToken(), 2, true)).toEqual({ kind: "launched" });
      const live = session.readSnapshot();
      expect(live.phase).toBe("flight_running");
      session.executeSharedSessionCommand({ kind: "pause" });
      expect(session.readSnapshot().frame).toEqual(live.frame);
      expect(session.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: true });
      session.executeSharedSessionCommand({ kind: "resume" });
      expect(session.readSnapshot().phase).toBe("flight_running");
    } finally { session.dispose(); }
  });

  it("transfers original live state, record queries and camera pose without a second numerical model", () => {
    const { bridge, session } = fixture();
    try {
      launch(session);
      session.flightPort.advanceTick(neutral);
      const live = readContract<TailSnapshotDocument>("tail_snapshot", bridge.snapshot_json());
      expect(session.readSnapshot()).toEqual(live);
      session.executeSharedSessionCommand({ kind: "abort" });
      const terminal = session.readSnapshot();
      if (terminal.frame.kind !== "result" || live.frame.kind !== "flight") throw new Error("Expected live and Result frames");
      expect(terminal.frame.state).toEqual(live.frame.state);
      expect(terminal.frame.telemetry).toEqual(live.frame.telemetry);
      const archive = session.exportRecordJson();
      const summary = readContract<SummaryDocument>("record_summary", bridge.flight_record_summary_json());
      const samples = readContract<AnalysisDocument>("record_analysis", bridge.flight_analysis_samples_json());
      const sample = readContract<RecordedPlaybackSample>("record_sample", bridge.flight_record_sample_at_seconds(0.005));
      const queriedWind = readContract<WindDocument>("record_wind", bridge.flight_wind_grid_json(-10, -10, 10, 5));
      expect(session.readRecordSummary()).toEqual(summary);
      expect(session.readAnalysisSamples()).toEqual(samples.samples);
      expect(session.queryRecordSample(0.005)).toEqual(sample);
      expect(session.queryWindGrid(wind)).toEqual(queriedWind);
      expect(summary.context.finalization.value).toEqual(terminal.frame.finalization);
      expect(samples.samples).toHaveLength(2);
      expect(sample).toMatchObject({ tick_index: 0, fraction: 0.5, flight_time_s: 0.005 });
      expect(sample.controls.layout).toBe("tail_incidence");
      expect(sample).not.toHaveProperty("render_pose");
      expect(queriedWind.projection.kind).toBe("available");
      if (queriedWind.projection.kind === "available") expect(queriedWind.projection.samples).toHaveLength(25);
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      const before = session.readPlaybackClock();
      const camera = readContract<ReplayCameraView>("replay_camera", bridge.playback_camera_view_json(0.005, "auto", true));
      const pose = session.queryReplayCamera(0.005, "auto", true);
      expect(pose).toMatchObject(camera.render_pose);
      expect(camera.sample).not.toHaveProperty("render_pose");
      expect(session.readPlaybackClock()).toEqual(before);
      expect(session.exportRecordJson()).toBe(archive);
    } finally { session.dispose(); }
  });

  it.each([0, 1])("keeps zero-duration and fractional saved states in the Rust archive boundary (%s ticks)", (ticks) => {
    const source = fixture();
    const viewer = fixture(2);
    try {
      launch(source.session);
      for (let tick = 0; tick < ticks; tick += 1) source.session.flightPort.advanceTick(neutral);
      source.session.executeSharedSessionCommand({ kind: "abort" });
      const original = source.session.exportRecordJson();
      viewer.session.openArchive(original);
      const document = JSON.parse(original) as { schema_version: number; samples: readonly unknown[]; finalization: { terminal_tick: number } };
      expect(document.schema_version).toBe(7);
      expect(document.samples).toHaveLength(ticks + 1);
      expect(viewer.session.readRecordSummary().summary.sample_count).toBe(ticks + 1);
      expect(viewer.session.queryRecordSample(ticks / 200).flight_time_s).toBe(ticks / 200);
      expect(viewer.session.readFlightLog("json")).toBe(original);
      viewer.session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(() => { viewer.session.openArchive("{malformed archive"); }).toThrow();
      expect(viewer.session.readLifecycle().phase).toBe("title");
    } finally { source.session.dispose(); viewer.session.dispose(); }
  });

  it("retains unavailable sky, wind and saved causes without selecting the current live environment", () => {
    const source = fixture();
    const viewer = fixture();
    try {
      launch(source.session);
      source.session.executeSharedSessionCommand({ kind: "abort" });
      const saved = JSON.parse(source.session.exportRecordJson()) as Record<string, unknown>;
      const header = saved.header as Record<string, unknown>;
      header.scenario_id = 999;
      header.environment_version = 99;
      const cause = { dynamics: { load: { aerodynamic: { hybrid: { site: { proxy: { surface: "horizontal_tail", index: 1 } },
        cause: { wind: "outside_grid" }, limit: null, stage: "second" } } } } };
      Object.assign(saved.finalization as object, { reason: "out_of_valid_envelope", disposition: "failed", failure: cause });
      viewer.session.openArchive(JSON.stringify(saved));
      expect(viewer.session.readRecordSummary().context.finalization.value.failure).toEqual(cause);
      expect(viewer.session.queryWindGrid(wind).projection).toMatchObject({ kind: "unavailable", reason: "unregistered_environment_identity" });
      const environment = readContract<EnvironmentSnapshot>("runtime_environment", viewer.bridge.environment_snapshot_json());
      expect(viewer.session.readEnvironment()).toEqual(environment);
      if (environment.projection.kind !== "unavailable") throw new Error("Expected the original unknown identity");
      expect(projectRuntimeEnvironment(environment)).toEqual({ ...environment.projection, reason: "unregistered_environment_identity" });
    } finally { source.session.dispose(); viewer.session.dispose(); }
  });
});
