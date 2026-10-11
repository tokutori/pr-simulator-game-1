import { readFileSync } from "node:fs";
import { describe, expect, expectTypeOf, it } from "vitest";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import type { TailPhysicalFlightControls, TailPresentationGeometryAvailability } from "../../web/src/render/contracts/flight-controls.js";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { projectFlightRenderPose, projectRecordedFlightSnapshot, projectTailFlightSnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { parseNamedRecordSample, parseNamedReplayContext } from "../../web/src/game/named-record-query.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function launch(session: HybridGameSessionBridge): void {
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
}

describe("control-layout-aware display snapshots", () => {
  it("requires registered geometry for controlled render poses", () => {
    type TailPose = Extract<FlightRenderPose, { controls: TailPhysicalFlightControls }>;
    expectTypeOf<TailPose["tailGeometry"]>().toEqualTypeOf<TailPresentationGeometryAvailability>();
    expectTypeOf<Omit<TailPose, "tailGeometry">>().not.toExtend<FlightRenderPose>();
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
      if (snapshot.kind !== "tail_flight") throw new Error("Expected live tail flight");
      expect(snapshot.tailGeometry.kind).toBe("available");
      expect(snapshot).not.toHaveProperty("scoreCourseMeters");
      expect(snapshot.progressMeters).toEqual({ kind: "available", value: raw.frame.progressMeters });
      expect(snapshot.progressMeters.value).toBe(raw.frame.progressMeters);
      expect(snapshot.telemetry.value.rollRadians).toBe(raw.frame.telemetry.attitudeEulerRadians[0]);
      session.pause(0);
      const pausedRaw = parseTailSessionSnapshot(session.snapshot_json());
      const paused = projectTailFlightSnapshot(pausedRaw);
      expect(paused).toMatchObject({ kind: "available", value: { kind: "tail_flight", phaseCode: 6 } });
      if (paused.kind !== "available" || paused.value.kind !== "tail_flight") throw new Error("Expected paused tail flight");
      expect(pausedRaw.identity).toEqual(raw.identity);
      expect(paused.value.tailGeometry).toEqual(snapshot.tailGeometry);
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

  it("projects saved controls without a held target or new-model reintegration", () => {
    const source = new HybridGameSessionBridge(0, 21, 22);
    const replay = new HybridGameSessionBridge(2, 31, 32);
    try {
      launch(source);
      source.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: -0.5,
        desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } }));
      source.abort();
      replay.open_archived_flight_record(source.export_flight_record_json());
      const context = parseNamedReplayContext(replay.playback_context_json());
      const sample = parseNamedRecordSample(replay.flight_record_sample_at_seconds(0.005), physics_hz(), context);
      const display = projectRecordedFlightSnapshot(sample, context);
      expect(display).toMatchObject({ kind: "tail_record",
        pilotPositionTargetMeters: { kind: "unavailable", reason: "record_pilot_target_unavailable" },
        stamp: { kind: "exact", tick: sample.tickIndex, fraction: sample.fraction, timeSeconds: sample.timeSeconds }, controls: sample.controls });
      if (display.kind !== "tail_record") throw new Error("Expected saved display");
      expect(display.finalization).toBe(context.finalization);
      expect(display.progressMeters).toEqual({ kind: "unavailable", reason: "record_course_axis_unavailable" });
      expect(sample).not.toHaveProperty("progressMeters");
      expect(display.angularRateBodyRadiansPerSecond).toEqual({ kind: "available", value: { roll: sample.state.angularVelocityBodyRadiansPerSecond[0],
        pitch: sample.state.angularVelocityBodyRadiansPerSecond[1], yaw: sample.state.angularVelocityBodyRadiansPerSecond[2] } });
      expect(display).not.toHaveProperty("terminal");
      expect(display.finalization.terminalTick).toBe(1);
      expect(display.stamp.tick).toBe(0);
      expect(projectFlightRenderPose(display, 0).controls).toBe(sample.controls);
      expect(() => projectFlightRenderPose(display, Number.NaN)).toThrow("finite");
    } finally {
      source.free();
      replay.free();
    }
  });

  it.each([
    ["bpg041-rectangular-hybrid-mock", 1, null],
    ["bpg041-zero-dihedral-oracle", 1, null],
    ["bpg041-playable-hybrid-mock", 2, 3.6],
    ["bpg041-rectangular-hybrid-mock", 2, null],
    ["bpg041-playable-hybrid-mock", 1, null],
    ["bpg041-playable-hybrid-mock", 3, null],
    ["unknown-aircraft", 2, null]
  ] as const)("projects %s model %s from each sealed live phase", (configurationId, modelVersion, expectedArm) => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    try {
      launch(session);
      const phases = [session.snapshot_json()];
      session.pause(0);
      phases.push(session.snapshot_json());
      session.abort();
      phases.push(session.snapshot_json());
      for (const json of phases) {
        const document = JSON.parse(json) as { scenario: { aircraft_model_version: number };
          control_identity: { aircraft_configuration_id: string } };
        document.scenario.aircraft_model_version = modelVersion;
        document.control_identity.aircraft_configuration_id = configurationId;
        const raw = parseTailSessionSnapshot(JSON.stringify(document));
        if (raw.frame.kind === "menu") throw new Error("Expected airborne or Result frame");
        const before = JSON.stringify(raw);
        const projected = projectTailFlightSnapshot(raw);
        if (projected.kind !== "available" || (projected.value.kind !== "tail_flight" && projected.value.kind !== "tail_result")) throw new Error("Expected tail display");
        const display = projected.value;
        expect(display.tailGeometry).toEqual(expectedArm === null
          ? { kind: "unavailable", reason: "unregistered_aircraft_geometry" }
          : { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: expectedArm } });
        expect(Object.isFrozen(display.tailGeometry)).toBe(true);
        if (display.tailGeometry.kind === "available") expect(Object.isFrozen(display.tailGeometry.value)).toBe(true);
        expect(display.controls.physicalIncidence).toBe(raw.frame.state.physicalIncidence);
        expect(projectFlightRenderPose(display, 0).tailGeometry).toBe(display.tailGeometry);
        expect(JSON.stringify(raw)).toBe(before);
      }
    } finally {
      session.free();
    }
  });

  it("projects archived tail geometry from record identity independently of the live default and sample values", () => {
    const source = new HybridGameSessionBridge(0, 21, 22);
    const replay = new HybridGameSessionBridge(2, 31, 32);
    try {
      launch(source);
      source.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: -0.5,
        desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } }));
      source.abort();
      replay.open_archived_flight_record(source.export_flight_record_json());
      const context = parseNamedReplayContext(replay.playback_context_json());
      const sample = parseNamedRecordSample(replay.flight_record_sample_at_seconds(0.005), physics_hz(), context);
      const before = JSON.stringify(sample);
      for (const [configurationId, modelVersion, expectedArm] of [
        ["bpg041-rectangular-hybrid-mock", 1, null],
        ["bpg041-zero-dihedral-oracle", 1, null],
        ["bpg041-playable-hybrid-mock", 2, 3.6],
        ["bpg041-playable-hybrid-mock", 1, null],
        ["unregistered-record-aircraft", 2, null]
      ] as const) {
        const recordContext = Object.freeze({ ...context, scenario: Object.freeze({ ...context.scenario, aircraftModelVersion: modelVersion }),
          controlIdentity: Object.freeze({ ...context.controlIdentity, aircraftConfigurationId: configurationId }) });
        const display = projectRecordedFlightSnapshot(sample, recordContext);
        if (display.kind !== "tail_record") throw new Error("Expected recorded tail display");
        expect(display.tailGeometry).toEqual(expectedArm === null
          ? { kind: "unavailable", reason: "unregistered_aircraft_geometry" }
          : { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: expectedArm } });
        expect(display.controls).toBe(sample.controls);
        expect(display.finalization).toBe(context.finalization);
        expect(display.stamp).toEqual({ kind: "exact", tick: sample.tickIndex, fraction: sample.fraction, timeSeconds: sample.timeSeconds });
        expect(projectFlightRenderPose(display, 0).tailGeometry).toBe(display.tailGeometry);
        expect(JSON.stringify(sample)).toBe(before);
      }
    } finally {
      source.free();
      replay.free();
    }
  });

});
