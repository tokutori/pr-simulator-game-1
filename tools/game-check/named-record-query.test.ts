import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { parseNamedAnalysisSamples, parseNamedRecordSample, parseNamedReplayClock, parseNamedReplayContext,
  tailResultRecordContext } from "../../web/src/game/named-record-query.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function jsonObject(json: string): Record<string, unknown> {
  const parsed: unknown = JSON.parse(json);
  return parsed as Record<string, unknown>;
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

function tailResult(ticks = 2): HybridGameSessionBridge {
  const session = new HybridGameSessionBridge(0, 11, 12);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
  for (let index = 0; index < ticks; index += 1) session.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence",
    noseUp: 0.5, turnRight: -0.5, desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } }));
  session.abort();
  return session;
}

function legacyRecord(): Record<string, unknown> {
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.set_difficulty_preset(1);
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    session.advance_tick(0.5, -0.5, 0.25, 0);
    session.abort();
    return jsonObject(session.export_flight_record_json());
  } finally {
    session.free();
  }
}

describe("named saved-record query boundary", () => {
  it("shares tail Result, Analysis and Replay snapshots without a legacy roll slot or cursor side effect", () => {
    const session = tailResult();
    try {
      const result = parseTailSessionSnapshot(session.snapshot_json());
      const context = tailResultRecordContext(result);
      const analysis = parseNamedAnalysisSamples(session.flight_analysis_samples_json(), physics_hz(), context);
      expect(analysis.map((sample) => sample.timeSeconds)).toEqual([0, 0.01, 0.02]);
      expect(analysis.at(-1)?.controls).toMatchObject({ layout: "tail_incidence" });
      expect(analysis.at(-1)?.controls).not.toHaveProperty("rollRadians");
      expect(analysis.at(-1)?.state.angularVelocityBodyRadiansPerSecond).toHaveLength(3);
      const archive = session.export_flight_record_json();
      session.enter_replay();
      expect(() => session.snapshot_json()).toThrow();
      const replay = parseNamedReplayContext(session.playback_context_json());
      expect(replay.controlLayout).toBe("tail_incidence");
      expect(replay.finalization).toEqual(context.finalization);
      const clock = parseNamedReplayClock(session.seek_playback(0.005), physics_hz(), replay);
      expect(clock).toEqual({ timeSeconds: 0.005, rateCode: 1, kind: "paused" });
      const sample = parseNamedRecordSample(session.flight_record_sample_at_seconds(clock.timeSeconds), physics_hz(), replay);
      expect(sample).toMatchObject({ tickIndex: 0, fraction: 0.5, timeSeconds: 0.005 });
      expect(sample.controls).toEqual(analysis[1]?.controls);
      parseNamedRecordSample(session.flight_record_sample_at_seconds(0.015), physics_hz(), replay);
      expect(parseNamedReplayClock(session.playback_clock_state(), physics_hz(), replay)).toEqual(clock);
      session.set_playback_rate_code(2);
      session.set_playback_playing(true);
      expect(parseNamedReplayClock(session.advance_playback(0.001), physics_hz(), replay)).toEqual({ timeSeconds: 0.007, rateCode: 2, kind: "playing" });
      expect(parseNamedReplayClock(session.seek_playback(0.02), physics_hz(), replay).kind).toBe("paused");
      expect(parseNamedReplayClock(session.set_playback_playing(true), physics_hz(), replay).timeSeconds).toBe(0);
      expect(session.export_flight_record_json()).toBe(archive);
      session.leave_replay();
      expect(parseTailSessionSnapshot(session.snapshot_json())).toEqual(result);
    } finally {
      session.free();
    }
  });

  it.each([1, 2, 3, 4, 5])("views saved v%d legacy controls and unknown environments without reintegration", (version) => {
    const saved = legacyRecord();
    saved.schema_version = version;
    const header = nested(saved, "header");
    header.environment_version = 99;
    if (version < 5) delete header.personal_best_key;
    if (version < 4) delete header.physics_model_version;
    if (version < 3) delete header.score_definition_version;
    if (version < 2) delete nested(header, "difficulty").hud_profile;
    const session = new HybridGameSessionBridge(2, 19, 20);
    try {
      const selected = session.control_mode_code();
      session.open_archived_flight_record(JSON.stringify(saved));
      const context = parseNamedReplayContext(session.playback_context_json());
      expect(context).toMatchObject({ controlLayout: "legacy_three_axis", controlIdentity: null,
        scenario: { environmentVersion: 99 }, difficulty: { preset: "standard", information: "standard", hudProfile: null } });
      expect(context.finalization).not.toHaveProperty("failure");
      const analysis = parseNamedAnalysisSamples(session.flight_analysis_samples_json(), physics_hz(), context);
      const sample = parseNamedRecordSample(session.flight_record_sample_at_seconds(0.005), physics_hz(), context);
      expect(sample.controls.layout).toBe("legacy_three_axis");
      expect(sample.controls).toEqual(analysis[1]?.controls);
      expect(sample.controls).toHaveProperty("rollRadians");
      expect(sample.controls).not.toHaveProperty("physicalIncidence");
      expect(() => session.control_profile_json()).toThrow();
      expect(() => session.export_flight_record_json()).toThrow();
      session.leave_replay();
      expect(session.phase_code()).toBe(0);
      expect(session.control_mode_code()).toBe(selected);
    } finally {
      session.free();
    }
  });

  it("views v6 archives from their saved context while retaining original nested failure causes", () => {
    const source = tailResult();
    const session = new HybridGameSessionBridge(2, 99, 100);
    try {
      const saved = jsonObject(source.export_flight_record_json());
      nested(saved, "header").environment_version = 99;
      session.open_archived_flight_record(JSON.stringify(saved));
      const context = parseNamedReplayContext(session.playback_context_json());
      expect(context.scenario.environmentVersion).toBe(99);
      const query = parseNamedRecordSample(session.flight_record_sample_at_seconds(0.01), physics_hz(), context);
      expect(query.controls).toMatchObject({ layout: "tail_incidence" });
      expect(query).toEqual(parseNamedAnalysisSamples(session.flight_analysis_samples_json(), physics_hz(), context)[1]);
      expect(() => session.control_profile_json()).toThrow();
      const document = jsonObject(session.playback_context_json());
      Object.assign(nested(nested(document, "finalization"), "value"), { reason: "out_of_valid_envelope", disposition: "failed", failure: {
        dynamics: { load: { aerodynamic: { hybrid: { site: { proxy: { surface: "horizontal_tail", index: 1 } },
          cause: { wind: "outside_grid" }, limit: null, stage: "second" } } } }
      } });
      const failed = parseNamedReplayContext(JSON.stringify(document));
      if (failed.controlLayout !== "tail_incidence") throw new Error("Expected two-tail finalization");
      expect(failed.finalization.failure).toMatchObject({ kind: "dynamics", cause: { kind: "load", cause: { kind: "aerodynamic",
        cause: { kind: "hybrid", cause: { stage: "second", site: { kind: "proxy", index: 1 }, cause: { kind: "wind", cause: "outside_grid" } } } } } });
      const terminal = nested(nested(document, "finalization"), "value");
      terminal.failure = { tick_overflow: "unknown" };
      expect(() => parseNamedReplayContext(JSON.stringify(document))).toThrow(RangeError);
      terminal.failure = null;
      expect(() => parseNamedReplayContext(JSON.stringify(document))).toThrow(RangeError);
    } finally {
      source.free();
      session.free();
    }
  });

  it("accepts zero-duration and fractional terminal batches and rejects malformed state/layout/stamps", () => {
    const empty = tailResult(0);
    const session = tailResult();
    try {
      empty.enter_replay();
      const zeroContext = parseNamedReplayContext(empty.playback_context_json());
      expect(parseNamedAnalysisSamples(empty.flight_analysis_samples_json(), physics_hz(), zeroContext)).toHaveLength(1);
      expect(parseNamedReplayClock(empty.set_playback_playing(true), physics_hz(), zeroContext)).toEqual({ timeSeconds: 0, rateCode: 1, kind: "paused" });
      session.enter_replay();
      const document = jsonObject(session.playback_context_json());
      Object.assign(nested(nested(document, "finalization"), "value"), { terminal_tick: 1, terminal_fraction: 0.375 });
      const context = parseNamedReplayContext(JSON.stringify(document));
      const batch = jsonObject(session.flight_analysis_samples_json());
      const samples = batch.samples as Record<string, unknown>[];
      Object.assign(samples[2] ?? {}, { tick_index: 1, fraction: 0.375, flight_time_s: 0.01375 });
      expect(parseNamedAnalysisSamples(JSON.stringify(batch), physics_hz(), context).at(-1)?.fraction).toBe(0.375);
      const original = session.flight_record_sample_at_seconds(0.005);
      const mutations: readonly ((sample: Record<string, unknown>) => void)[] = [
        (sample) => { sample.schema_version = 1; }, (sample) => { sample.tick_index = 0.5; },
        (sample) => { sample.fraction = -1; }, (sample) => { sample.flight_time_s = 0.5; },
        (sample) => { sample.flight_time_s = 0.006; }, (sample) => { nested(sample, "controls").roll_rad = 0; },
        (sample) => { nested(sample, "controls").layout = "legacy_three_axis"; },
        (sample) => { nested(nested(sample, "controls"), "physical_incidence").horizontal_tail_rad = 1; },
        (sample) => { nested(sample, "state").attitude_body_to_ned = [2, 0, 0, 0]; },
        (sample) => { nested(sample, "state").angular_velocity_body_rad_s = [0, 0]; },
        (sample) => { nested(nested(sample, "state"), "telemetry").angle_of_attack_rad = "undefined"; }
      ];
      for (const mutate of mutations) {
        const sample = jsonObject(original);
        mutate(sample);
        expect(() => parseNamedRecordSample(JSON.stringify(sample), physics_hz(), context)).toThrow(RangeError);
      }
      const duplicate = { ...batch, samples: [samples[0], samples[0], samples[2]] };
      expect(() => parseNamedAnalysisSamples(JSON.stringify(duplicate), physics_hz(), context)).toThrow(RangeError);
      expect(() => parseNamedAnalysisSamples(JSON.stringify({ ...batch, samples: [] }), physics_hz(), context)).toThrow(RangeError);
      expect(() => parseNamedAnalysisSamples(session.flight_analysis_samples_json(), physics_hz(), context)).toThrow(RangeError);
      expect(() => parseNamedRecordSample(original, 0, context)).toThrow(RangeError);
    } finally {
      empty.free();
      session.free();
    }
  });

  it("rejects contradictory Replay context and invalid core clock projections", () => {
    const session = tailResult();
    try {
      session.enter_replay();
      const json = session.playback_context_json();
      const context = parseNamedReplayContext(json);
      const mutations: readonly ((document: Record<string, unknown>) => void)[] = [
        (document) => { document.schema_version = 1; }, (document) => { document.phase = "result"; },
        (document) => { document.control_identity = null; }, (document) => { document.control_layout = "legacy_three_axis"; },
        (document) => { nested(document, "finalization").layout = "legacy_three_axis"; },
        (document) => { nested(document, "scenario").seed_high = 0x1_0000_0000; },
        (document) => { nested(nested(document, "finalization"), "value").disposition = "complete"; },
        (document) => { nested(nested(document, "finalization"), "value").score_m = [3, 4, 0]; },
        (document) => { nested(document, "difficulty").information = "custom"; },
        (document) => { nested(document, "difficulty").hud_profile = { wind: true }; }
      ];
      for (const mutate of mutations) {
        const document = jsonObject(json);
        mutate(document);
        expect(() => parseNamedReplayContext(JSON.stringify(document))).toThrow(RangeError);
      }
      for (const clock of [[0, 1], [-1, 1, 0], [NaN, 1, 0], [0, 3, 0], [0, 1, 2], [0.03, 1, 0], [0.02, 1, 1]]) {
        expect(() => parseNamedReplayClock(clock, physics_hz(), context)).toThrow(RangeError);
      }
      expect(() => parseNamedReplayContext(" ".repeat(16_385))).toThrow(RangeError);
    } finally {
      session.free();
    }
  });
});
