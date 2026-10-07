import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { projectFlightRenderPose, projectLegacyFlightSnapshot, projectRecordedFlightSnapshot, projectTailFlightSnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { parseNamedRecordSample, parseNamedReplayContext } from "../../web/src/game/named-record-query.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function launch(session: GameSessionBridge | HybridGameSessionBridge): void {
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
}

describe("control-layout-aware display snapshots", () => {
  it("retains the old 33-value snapshot and marks unavailable physical fields explicitly", () => {
    const session = new GameSessionBridge(0);
    try {
      launch(session);
      const raw = parseFlightSnapshot(session.advance_tick(0.5, -0.5, 0.25, 0));
      const snapshot = projectLegacyFlightSnapshot(raw);
      expect(snapshot).toMatchObject({ kind: "legacy_live", positionNed: raw.positionNed, velocityNed: raw.velocityNed,
        stamp: { kind: "legacy_projection", tick: raw.tick, contactFraction: { kind: "unavailable", reason: "non_contact_snapshot" }, timeSeconds: raw.flightTimeSeconds },
        controls: { layout: "legacy_three_axis", rollRadians: raw.actuatorDeflectionRadians.roll,
          pitchRadians: raw.actuatorDeflectionRadians.pitch, yawRadians: raw.actuatorDeflectionRadians.yaw },
        angularRateBodyRadiansPerSecond: { kind: "unavailable", reason: "legacy_body_rate_unavailable" },
        compositeCgPositionNedMeters: { kind: "unavailable", reason: "legacy_cg_unavailable" },
        pilotPositionTargetMeters: { kind: "unavailable", reason: "legacy_pilot_target_unavailable" } });
      expect(snapshot.progressMeters).toEqual({ kind: "unavailable", reason: "legacy_progress_unavailable" });
      expect(snapshot.controls).not.toHaveProperty("physicalIncidence");
      expect(snapshot).not.toHaveProperty("finalization");
      if (snapshot.telemetry.kind !== "available") throw new Error("Expected legacy telemetry");
      expect(snapshot.telemetry.value).toMatchObject({ altitudeMeters: raw.telemetry?.altitudeMeters,
        angleOfAttackRadians: { kind: "available", value: raw.telemetry?.angleOfAttackRadians } });
      expect(Object.isFrozen(snapshot)).toBe(true);
      expect(Object.isFrozen(snapshot.controls)).toBe(true);
      const pose = projectFlightRenderPose(snapshot, -0.1);
      expect(pose).toMatchObject({ controls: snapshot.controls, simulationTimeSeconds: raw.flightTimeSeconds, initialPilotPositionMeters: -0.1 });
      expect(pose).not.toHaveProperty("actuatorDeflectionRadians");
    } finally {
      session.free();
    }
  });

  it("projects tail flight and pause without adding a roll control or unavailable score", () => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    try {
      expect(projectTailFlightSnapshot(parseTailSessionSnapshot(session.snapshot_json()))).toEqual({ kind: "unavailable", reason: "menu_phase" });
      launch(session);
      const raw = parseTailSessionSnapshot(session.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence",
        noseUp: 0.5, turnRight: -0.5, desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0,
        pilotPositionCommand: { kind: "set", normalized: 0.1 } })));
      if (raw.frame.kind !== "flight") throw new Error("Expected Rust airborne state");
      const state = raw.frame.state;
      const projected = projectTailFlightSnapshot(raw);
      if (projected.kind !== "available") throw new Error("Expected tail flight display");
      const snapshot = projected.value;
      expect(snapshot).toMatchObject({ kind: "tail_flight", phaseCode: 5,
        stamp: { kind: "exact", tick: state.tick, fraction: state.fraction, timeSeconds: state.flightTimeSeconds },
        pilotPositionMeters: state.pilotPositionMeters, pilotPositionTargetMeters: { kind: "available", value: state.pilotPositionTargetMeters },
        controls: { layout: "tail_incidence", physicalIncidence: state.physicalIncidence },
        angularRateBodyRadiansPerSecond: { kind: "available", value: { roll: state.angularRateBodyRadiansPerSecond[0], pitch: state.angularRateBodyRadiansPerSecond[1], yaw: state.angularRateBodyRadiansPerSecond[2] } } });
      expect(snapshot.controls).not.toHaveProperty("rollRadians");
      expect(snapshot).not.toHaveProperty("scoreCourseMeters");
      expect(snapshot.progressMeters).toEqual({ kind: "available", value: raw.frame.progressMeters });
      if (snapshot.progressMeters.kind !== "available") throw new Error("Expected Rust progress");
      expect(snapshot.progressMeters.value).toBe(raw.frame.progressMeters);
      if (snapshot.telemetry.kind !== "available") throw new Error("Expected tail telemetry");
      expect(snapshot.telemetry.value.rollRadians).toBe(raw.frame.telemetry.attitudeEulerRadians[0]);
      session.pause(0);
      expect(projectTailFlightSnapshot(parseTailSessionSnapshot(session.snapshot_json()))).toMatchObject({ kind: "available", value: { kind: "tail_flight", phaseCode: 6 } });
    } finally {
      session.free();
    }
  });

  it("retains Result finalization, score and original cause with one fractional stamp", () => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    try {
      launch(session);
      session.abort();
      const payload: unknown = JSON.parse(session.snapshot_json());
      const document = payload as { frame: { state: { fraction: number; flight_time_s: number }; finalization: Record<string, unknown> } };
      document.frame.state.fraction = 0.375;
      document.frame.state.flight_time_s = 0.375 / physics_hz();
      Object.assign(document.frame.finalization, { reason: "out_of_valid_envelope", disposition: "failed", terminal_fraction: 0.375,
        failure: { dynamics: { load: { aerodynamic: { hybrid: { site: "datum", cause: { wind: "outside_grid" }, limit: null, stage: "first" } } } } } });
      const raw = parseTailSessionSnapshot(JSON.stringify(document));
      if (raw.frame.kind !== "result") throw new Error("Expected Rust Result frame");
      const projected = projectTailFlightSnapshot(raw);
      if (projected.kind !== "available" || projected.value.kind !== "tail_result") throw new Error("Expected tail Result display");
      const snapshot = projected.value;
      expect(snapshot.finalization).toBe(raw.frame.finalization);
      expect(snapshot.finalization.failure).toBe(raw.frame.finalization.failure);
      expect(snapshot.progressMeters).toEqual({ kind: "unavailable", reason: "terminal_progress_unavailable" });
      expect(snapshot.stamp).toEqual({ kind: "exact", tick: 0, fraction: 0.375, timeSeconds: 0.00375 });
      expect(projectFlightRenderPose(snapshot, 0)).toMatchObject({ simulationTimeSeconds: 0.00375, controls: snapshot.controls });
      expect(snapshot.controls).not.toHaveProperty("pitchRadians");
    } finally {
      session.free();
    }
  });

  it.each(["legacy_three_axis", "tail_incidence"] as const)("projects saved %s without a held target or new-model reintegration", (layout) => {
    const source = layout === "legacy_three_axis" ? new GameSessionBridge(0) : new HybridGameSessionBridge(0, 21, 22);
    const replay = new HybridGameSessionBridge(2, 31, 32);
    try {
      launch(source);
      if (source instanceof GameSessionBridge) source.advance_tick(0.5, -0.5, 0.25, 0);
      else source.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: -0.5,
        desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } }));
      source.abort();
      replay.open_archived_flight_record(source.export_flight_record_json());
      const context = parseNamedReplayContext(replay.playback_context_json());
      const sample = parseNamedRecordSample(replay.flight_record_sample_at_seconds(0.005), physics_hz(), context);
      const display = projectRecordedFlightSnapshot(sample, context);
      expect(display).toMatchObject({ kind: layout === "legacy_three_axis" ? "legacy_record" : "tail_record",
        pilotPositionTargetMeters: { kind: "unavailable", reason: "record_pilot_target_unavailable" },
        stamp: { kind: "exact", tick: sample.tickIndex, fraction: sample.fraction, timeSeconds: sample.timeSeconds }, controls: sample.controls });
      if (display.kind !== "legacy_record" && display.kind !== "tail_record") throw new Error("Expected saved display");
      expect(display.finalization).toBe(context.finalization);
      expect(display.progressMeters).toEqual({ kind: "unavailable", reason: "record_course_axis_unavailable" });
      expect(sample).not.toHaveProperty("progressMeters");
      expect(display.angularRateBodyRadiansPerSecond).toEqual({ kind: "available", value: { roll: sample.state.angularVelocityBodyRadiansPerSecond[0],
        pitch: sample.state.angularVelocityBodyRadiansPerSecond[1], yaw: sample.state.angularVelocityBodyRadiansPerSecond[2] } });
      expect(display).not.toHaveProperty("terminal");
      expect(display.finalization.terminalTick).toBe(1);
      expect(display.stamp.tick).toBe(0);
      expect(projectFlightRenderPose(display, 0).controls).toBe(sample.controls);
      const conflicting = context.controlLayout === "legacy_three_axis"
        ? { controlLayout: "tail_incidence" as const, finalization: { ...context.finalization, failure: null } }
        : { controlLayout: "legacy_three_axis" as const, finalization: context.finalization };
      expect(() => projectRecordedFlightSnapshot(sample, conflicting)).toThrow("same layout");
      expect(() => projectFlightRenderPose(display, Number.NaN)).toThrow("finite");
    } finally {
      source.free();
      replay.free();
    }
  });

  it("normalizes legacy missing telemetry and undefined flow angles at the projection boundary", () => {
    const values = new Array<number>(33).fill(0);
    values[7] = 1;
    values[19] = -1;
    const missing = projectLegacyFlightSnapshot(parseFlightSnapshot(values));
    expect(missing.telemetry).toEqual({ kind: "unavailable", reason: "legacy_telemetry_unavailable" });
    expect(projectFlightRenderPose(missing, 0)).toMatchObject({ airspeedMetersPerSecond: null, windVelocityNedMetersPerSecond: null });
    values[31] = 1;
    const zeroFlow = projectLegacyFlightSnapshot(parseFlightSnapshot(values));
    if (zeroFlow.telemetry.kind !== "available") throw new Error("Expected zero-speed telemetry");
    expect(zeroFlow.telemetry.value.angleOfAttackRadians).toEqual({ kind: "unavailable", reason: "undefined_flow_angle" });
    expect(zeroFlow.telemetry.value.sideslipAngleRadians).toEqual({ kind: "unavailable", reason: "undefined_flow_angle" });
    values[16] = 1;
    values[19] = 0.25;
    expect(projectLegacyFlightSnapshot(parseFlightSnapshot(values)).stamp).toMatchObject({ contactFraction: { kind: "available", value: 0.25 } });
  });
});
