import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import type { TailLogicalInput } from "../../web/src/game/tail-session-codec.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
const neutral: TailLogicalInput = Object.freeze({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: Object.freeze({ kind: "hold" }) });

function launch(mode = 0): HybridGameSessionBridge {
  const session = new HybridGameSessionBridge(mode, 0xffff_ffff, 0xffff_ffff);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
  return session;
}

function snapshotDocument() {
  const session = launch();
  try {
    const parsed: unknown = JSON.parse(session.snapshot_json());
    return parsed as Record<string, unknown>;
  } finally {
    session.free();
  }
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

function terminalDocument(failure: unknown = null, reason = "manual_abort") {
  const document = snapshotDocument();
  const frame = nested(document, "frame");
  document.phase_code = 7;
  frame.kind = "result";
  delete frame.progress_m;
  frame.finalization = { reason, disposition: reason === "manual_abort" ? "interrupted" : "failed",
    terminal_tick: 0, terminal_fraction: 0, score_m: [0, 0, 0], failure };
  return document;
}

describe("versioned two-tail TypeScript boundary", () => {
  it("retains Rust live progress for Flight and Pause and keeps terminal score separate", () => {
    const session = launch();
    try {
      const initial = parseTailSessionSnapshot(session.snapshot_json());
      if (initial.frame.kind !== "flight") throw new Error("Expected flight frame");
      expect(initial.frame.progressMeters).toEqual({ courseParallelMeters: 0, crossTrackMeters: 0, netHorizontalMeters: 0 });
      const next = parseTailSessionSnapshot(session.advance_tick_json(encodeTailLogicalInput(neutral)));
      const payload: unknown = JSON.parse(session.snapshot_json());
      const progress = nested(nested(payload as Record<string, unknown>, "frame"), "progress_m");
      if (next.frame.kind !== "flight") throw new Error("Expected flight frame");
      expect(next.frame.progressMeters).toEqual({ courseParallelMeters: progress.course_parallel_m,
        crossTrackMeters: progress.cross_track_m, netHorizontalMeters: progress.net_horizontal_m });
      expect(Object.isFrozen(next.frame.progressMeters)).toBe(true);
      session.pause(0);
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 6, frame: { progressMeters: next.frame.progressMeters } });
      session.clear_pause_reason(0);
      session.resume();
      const result = parseTailSessionSnapshot(session.abort());
      if (result.frame.kind !== "result") throw new Error("Expected Result frame");
      expect(result.frame).not.toHaveProperty("progressMeters");
      expect(result.frame.finalization.scoreMeters).toEqual([next.frame.progressMeters.courseParallelMeters,
        next.frame.progressMeters.crossTrackMeters, next.frame.progressMeters.netHorizontalMeters]);
    } finally {
      session.free();
    }
  });

  it("rejects missing, non-finite, inconsistent and surplus live progress", () => {
    const corruptions: readonly ((frame: Record<string, unknown>) => void)[] = [
      (frame) => { delete frame.progress_m; },
      (frame) => { frame.progress_m = null; },
      (frame) => { nested(frame, "progress_m").course_parallel_m = Infinity; },
      (frame) => { nested(frame, "progress_m").cross_track_m = null; },
      (frame) => { nested(frame, "progress_m").net_horizontal_m = -1; },
      (frame) => { frame.progress_m = { course_parallel_m: 3, cross_track_m: 4, net_horizontal_m: 0 }; },
      (frame) => { nested(frame, "progress_m").extra = 0; }
    ];
    for (const corrupt of corruptions) {
      const document = snapshotDocument();
      corrupt(nested(document, "frame"));
      expect(() => parseTailSessionSnapshot(JSON.stringify(document))).toThrow(RangeError);
    }
    const result = terminalDocument();
    nested(result, "frame").progress_m = { course_parallel_m: 0, cross_track_m: 0, net_horizontal_m: 0 };
    expect(() => parseTailSessionSnapshot(JSON.stringify(result))).toThrow(RangeError);
    const menu = snapshotDocument();
    menu.phase_code = 3;
    menu.frame = { kind: "menu", progress_m: { course_parallel_m: 0, cross_track_m: 0, net_horizontal_m: 0 } };
    expect(() => parseTailSessionSnapshot(JSON.stringify(menu))).toThrow(RangeError);
    const legal = snapshotDocument();
    nested(legal, "frame").progress_m = { course_parallel_m: -3, cross_track_m: 4, net_horizontal_m: 5 };
    expect(parseTailSessionSnapshot(JSON.stringify(legal))).toMatchObject({ frame: { progressMeters: {
      courseParallelMeters: -3, crossTrackMeters: 4, netHorizontalMeters: 5 } } });
  });
  it("requires sealed identity for every Briefing and Countdown phase and clears it for Title and Setup", () => {
    const session = new HybridGameSessionBridge(0, 1, 2);
    try {
      const title = parseTailSessionSnapshot(session.snapshot_json());
      expect(title).toMatchObject({ phaseCode: 0, identity: { kind: "unprepared" }, frame: { kind: "menu" } });
      session.open_setup();
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 1, identity: { kind: "unprepared" } });
      session.prepare();
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 2, identity: { kind: "prepared" } });
      session.mark_briefing_ready();
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 3, identity: { kind: "prepared" } });
      session.start_countdown(1);
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 4, identity: { kind: "prepared" } });
      session.open_setup();
      expect(parseTailSessionSnapshot(session.snapshot_json())).toMatchObject({ phaseCode: 1, identity: { kind: "unprepared" } });
    } finally {
      session.free();
    }
    for (const phase of [0, 1, 2, 3, 4, 8]) {
      const prepared = snapshotDocument();
      prepared.phase_code = phase;
      prepared.frame = { kind: "menu" };
      const unprepared = { ...prepared, scenario: null, control_identity: null };
      if (phase === 0 || phase === 1) {
        expect(() => parseTailSessionSnapshot(JSON.stringify(prepared))).toThrow(RangeError);
        expect(parseTailSessionSnapshot(JSON.stringify(unprepared)).identity.kind).toBe("unprepared");
      } else {
        expect(() => parseTailSessionSnapshot(JSON.stringify(unprepared))).toThrow(RangeError);
        expect(parseTailSessionSnapshot(JSON.stringify(prepared)).identity.kind).toBe("prepared");
      }
    }
  });

  it("reads generated WASM identities and lifecycle without changing the legacy factory", () => {
    for (const mode of [0, 1, 2]) {
      const session = launch(mode);
      const legacy = new GameSessionBridge(0);
      try {
        const initial = parseTailSessionSnapshot(session.snapshot_json(), physics_hz());
        expect(initial.phaseCode).toBe(5);
        expect(initial.controlLayout).toBe("tail_incidence");
        expect(initial.controlModeCode).toBe(mode);
        expect(initial.identity).toMatchObject({ kind: "prepared",
          scenario: { catalogVersion: 3, scenarioId: 6, scenarioVersion: 3, aircraftModelVersion: 2,
            environmentVersion: 6, controllerProfileVersion: 3, seedLow: 0xffff_ffff, seedHigh: 0xffff_ffff },
          controls: { aircraftConfigurationId: "bpg041-playable-hybrid-mock", controllerProfileId: "bpg040-tail-rate-feedback" } });
        if (initial.frame.kind !== "flight") throw new Error("Expected flight frame");
        expect(Object.isFrozen(initial.frame.state.physicalIncidence)).toBe(true);
        expect(initial.frame.state.physicalIncidence).not.toHaveProperty("roll");
        expect(initial.frame.state.angularRateBodyRadiansPerSecond).toHaveLength(3);
        expect(initial.frame.telemetry.altitudeMeters).toBe(10.5);
        session.pause(0);
        expect(parseTailSessionSnapshot(session.snapshot_json()).phaseCode).toBe(6);
        session.resume();
        const next = parseTailSessionSnapshot(session.advance_tick_json(encodeTailLogicalInput(neutral)));
        if (next.frame.kind !== "flight") throw new Error("Expected flight frame");
        expect(next.frame.state.tick).toBe(1);
        expect(next.frame.state.flightTimeSeconds).toBe(0.01);
        const result = parseTailSessionSnapshot(session.abort());
        if (result.frame.kind !== "result") throw new Error("Expected terminal frame");
        expect(result.frame.finalization.reason).toBe("manual_abort");
        expect(result.frame.finalization.terminalTick).toBe(result.frame.state.tick);
        session.retry();
        expect(parseTailSessionSnapshot(session.snapshot_json()).phaseCode).toBe(3);
        legacy.open_setup();
        legacy.prepare();
        legacy.mark_briefing_ready();
        legacy.start_countdown(1);
        legacy.advance_countdown();
        expect(parseFlightSnapshot(legacy.launch()).tick).toBe(0);
        expect(legacy.snapshot()).toHaveLength(33);
      } finally {
        session.free();
        legacy.free();
      }
    }
  });

  it("encodes nose-up/right-turn intent and preserves Hold/Set separately", () => {
    const session = launch();
    try {
      const input: TailLogicalInput = { ...neutral, noseUp: 1, turnRight: 1,
        desiredPitchRateRadiansPerSecond: 0.2, desiredYawRateRadiansPerSecond: 0.2,
        pilotPositionCommand: { kind: "set", normalized: 1 } };
      const json = encodeTailLogicalInput(input);
      expect(JSON.parse(json)).toEqual({ schema_version: 2, control_layout: "tail_incidence", nose_up: 1, turn_right: 1,
        desired_pitch_rate_rad_s: 0.2, desired_yaw_rate_rad_s: 0.2, pilot_position_command: { kind: "set", normalized: 1 } });
      const moved = parseTailSessionSnapshot(session.advance_tick_json(json));
      if (moved.frame.kind !== "flight") throw new Error("Expected flight frame");
      expect(moved.frame.state.physicalIncidence).toEqual({ horizontalTailRadians: -0.01, verticalTailRadians: -0.01 });
      expect(moved.frame.state.pilotPositionTargetMeters).toBe(0.4);
      const held = parseTailSessionSnapshot(session.advance_tick_json(encodeTailLogicalInput(neutral)));
      if (held.frame.kind !== "flight") throw new Error("Expected flight frame");
      expect(held.frame.state.pilotPositionTargetMeters).toBe(0.4);
      const surplus = { ...neutral, roll: 0 };
      expect(() => encodeTailLogicalInput(surplus)).toThrow(RangeError);
      for (const invalid of [
        { ...neutral, noseUp: 1.01 }, { ...neutral, turnRight: NaN },
        { ...neutral, desiredPitchRateRadiansPerSecond: NaN }, { ...neutral, desiredYawRateRadiansPerSecond: Infinity },
        { ...neutral, pilotPositionCommand: { kind: "set" as const, normalized: -1.01 } },
        { ...neutral, pilotPositionCommand: { kind: "hold" as const, normalized: 0 } }
      ]) expect(() => encodeTailLogicalInput(invalid)).toThrow(RangeError);
      const explicitRates = { ...neutral, desiredPitchRateRadiansPerSecond: 0.201 };
      expect(JSON.parse(encodeTailLogicalInput(explicitRates))).toHaveProperty("desired_pitch_rate_rad_s", 0.201);
      const before = session.snapshot_json();
      expect(() => session.advance_tick_json(encodeTailLogicalInput(explicitRates))).toThrow();
      expect(session.snapshot_json()).toBe(before);
    } finally {
      session.free();
    }
  });

  it("keeps one fractional terminal time and explicit absent telemetry angles", () => {
    const document = terminalDocument(null, "water_contact");
    const frame = nested(document, "frame");
    const state = nested(frame, "state");
    Object.assign(state, { tick: 8, fraction: 0.375, flight_time_s: 0.08375 });
    Object.assign(nested(frame, "finalization"), { disposition: "complete", terminal_tick: 8, terminal_fraction: 0.375 });
    Object.assign(nested(frame, "telemetry"), { angle_of_attack_rad: null, sideslip_angle_rad: null });
    const parsed = parseTailSessionSnapshot(JSON.stringify(document));
    if (parsed.frame.kind !== "result") throw new Error("Expected terminal frame");
    expect(parsed.frame.state.fraction).toBe(0.375);
    expect(parsed.frame.state.flightTimeSeconds).toBe(0.08375);
    expect(parsed.frame.telemetry.angleOfAttackRadians).toBeNull();
    expect(Object.isFrozen(parsed.frame.state.attitudeBodyToNed)).toBe(true);
  });

  it("rejects negative or inconsistent net distance and preserves signed valid score components", () => {
    for (const score of [[0, 0, -1], [3, 4, 0], [Number.MAX_VALUE, Number.MAX_VALUE, 1], [3, 4, 5 + 6e-9]]) {
      const document = terminalDocument();
      nested(nested(document, "frame"), "finalization").score_m = score;
      expect(() => parseTailSessionSnapshot(JSON.stringify(document))).toThrow(RangeError);
    }
    for (const score of [[0, 0, 0], [3, 4, 5], [-3, -4, 5], [3, 4, 5 + 4e-9]]) {
      const document = terminalDocument();
      nested(nested(document, "frame"), "finalization").score_m = score;
      const snapshot = parseTailSessionSnapshot(JSON.stringify(document));
      if (snapshot.frame.kind !== "result") throw new Error("Expected terminal frame");
      expect(snapshot.frame.finalization.scoreMeters).toEqual(score);
    }
    const document = terminalDocument();
    nested(nested(document, "frame"), "finalization").score_m = null;
    const snapshot = parseTailSessionSnapshot(JSON.stringify(document));
    if (snapshot.frame.kind !== "result") throw new Error("Expected terminal frame");
    expect(snapshot.frame.finalization.scoreMeters).toBeNull();
  });

  it("rejects wrong schema/layout, unsafe seed, surplus roll, phase/frame and stamp conflicts", () => {
    const mutations: readonly ((document: Record<string, unknown>) => void)[] = [
      (document) => { document.schema_version = 1; }, (document) => { document.control_layout = "legacy_three_axis"; },
      (document) => { document.phase_code = 9; }, (document) => { document.phase_code = 3; },
      (document) => { document.control_mode_code = 3; }, (document) => { document.control_identity = null; },
      (document) => { nested(document, "scenario").seed_high = 0x1_0000_0000; },
      (document) => { nested(document, "control_identity").controller_profile_id = ""; },
      (document) => { nested(nested(nested(document, "frame"), "state"), "physical_incidence").roll_rad = 0; },
      (document) => { nested(nested(document, "frame"), "state").fraction = 0.5; },
      (document) => { nested(nested(document, "frame"), "state").flight_time_s = 1; },
      (document) => { nested(nested(document, "frame"), "state").attitude_body_to_ned = [2, 0, 0, 0]; },
      (document) => { nested(nested(document, "frame"), "state").angular_rate_body_rad_s = [0, 0]; },
      (document) => { nested(nested(document, "frame"), "telemetry").airspeed_mps = -1; },
      (document) => { document.unexpected = true; }
    ];
    for (const mutate of mutations) {
      const document = snapshotDocument();
      mutate(document);
      expect(() => parseTailSessionSnapshot(JSON.stringify(document))).toThrow(RangeError);
    }
    const terminal = terminalDocument();
    nested(nested(terminal, "frame"), "finalization").terminal_tick = 1;
    expect(() => parseTailSessionSnapshot(JSON.stringify(terminal))).toThrow(RangeError);
    expect(() => parseTailSessionSnapshot(" ".repeat(16_385))).toThrow(RangeError);
  });

  it("retains nested original causes and rejects unknown or contradictory diagnostics", () => {
    const hybrid = { site: { proxy: { surface: "horizontal_tail", index: 1 } }, cause: { wind: "outside_grid" },
      limit: null, stage: "second" };
    const document = terminalDocument({ dynamics: { load: { aerodynamic: { hybrid } } } }, "out_of_valid_envelope");
    const parsed = parseTailSessionSnapshot(JSON.stringify(document));
    if (parsed.frame.kind !== "result") throw new Error("Expected terminal frame");
    expect(parsed.frame.finalization.failure).toEqual({ kind: "dynamics", cause: { kind: "load", cause: { kind: "aerodynamic",
      cause: { kind: "hybrid", cause: { site: { kind: "proxy", surface: "horizontal_tail", index: 1 },
        cause: { kind: "wind", cause: "outside_grid" }, limit: null, stage: "second" } } } } });
    const causes: readonly unknown[] = ["unknown", { control: "unknown" }, { dynamics: { load: "unknown" } },
      { control: { incidence: { site: "static_polar", cause: "outside_envelope", limit: "global_beta", stage: "fourth" } } },
      { control: { incidence: { site: "tail_incidence", cause: "non_finite", limit: null, stage: "first" } } },
      { dynamics: { load: { aerodynamic: { hybrid: { ...hybrid, cause: "non_finite", limit: "local_speed" } } } } },
      { tick_overflow: null }, { contact: { math: "unknown" } }];
    for (const cause of causes) {
      expect(() => parseTailSessionSnapshot(JSON.stringify(terminalDocument(cause, "fatal_simulation_error")))).toThrow(RangeError);
    }
    expect(() => parseTailSessionSnapshot(JSON.stringify(terminalDocument("tick_overflow", "out_of_valid_envelope")))).toThrow(RangeError);
    expect(() => parseTailSessionSnapshot(JSON.stringify(terminalDocument(null, "out_of_valid_envelope")))).toThrow(RangeError);
    const missing = terminalDocument();
    delete nested(nested(missing, "frame"), "finalization").failure;
    expect(() => parseTailSessionSnapshot(JSON.stringify(missing))).toThrow(RangeError);
  });
});
