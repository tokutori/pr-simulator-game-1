import type { LegacyPhysicalFlightControls, TailPhysicalFlightControls } from "../render/contracts/flight-controls.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";
import type { FlightSnapshot } from "./flight-snapshot.js";
import type { LegacyTerminalFinalization, NamedRecordSample, RecordQueryContext } from "./named-record-query.js";
import type { TailFlightProgressMeters, TailFlightTelemetry, TailSessionSnapshot, TailTerminalFinalization } from "./tail-session-codec.js";

type NedVector = FlightSnapshot["positionNed"];
type BodyRate = Readonly<{ roll: number; pitch: number; yaw: number }>;
export type DisplayAvailability<Value, Reason extends string> =
  | Readonly<{ kind: "available"; value: Value }>
  | Readonly<{ kind: "unavailable"; reason: Reason }>;
type Available<Value> = Extract<DisplayAvailability<Value, never>, { kind: "available" }>;
type FlowAngle = DisplayAvailability<number, "undefined_flow_angle">;
type ExactStamp = Readonly<{ kind: "exact"; tick: number; fraction: number; timeSeconds: number }>;
type LegacyStamp = Readonly<{ kind: "legacy_projection"; tick: number;
  contactFraction: DisplayAvailability<number, "non_contact_snapshot">; timeSeconds: number }>;
export type FlightDisplayTelemetry = Omit<NonNullable<FlightSnapshot["telemetry"]>, "angleOfAttackRadians" | "sideslipAngleRadians"> & Readonly<{
  angleOfAttackRadians: FlowAngle;
  sideslipAngleRadians: FlowAngle;
}>;

interface DisplayState {
  readonly positionNed: NedVector;
  readonly velocityNed: NedVector;
  readonly attitudeBodyToNed: FlightSnapshot["attitudeBodyToNed"];
  readonly pilotPositionMeters: number;
  readonly pilotVelocityMetersPerSecond: number;
  readonly telemetry: DisplayAvailability<FlightDisplayTelemetry, "legacy_telemetry_unavailable">;
}
interface NamedDisplayState extends DisplayState {
  readonly stamp: ExactStamp;
  readonly angularRateBodyRadiansPerSecond: Available<BodyRate>;
  readonly compositeCgPositionNedMeters: Available<NedVector>;
  readonly telemetry: Available<FlightDisplayTelemetry>;
}
export type FlightDisplaySnapshot =
  | (DisplayState & Readonly<{
      kind: "legacy_live";
      controls: LegacyPhysicalFlightControls;
      stamp: LegacyStamp;
      angularRateBodyRadiansPerSecond: Readonly<{ kind: "unavailable"; reason: "legacy_body_rate_unavailable" }>;
      compositeCgPositionNedMeters: Readonly<{ kind: "unavailable"; reason: "legacy_cg_unavailable" }>;
      pilotPositionTargetMeters: Readonly<{ kind: "unavailable"; reason: "legacy_pilot_target_unavailable" }>;
      pilotPositionTargetNormalized: Readonly<{ kind: "unavailable"; reason: "legacy_pilot_target_unavailable" }>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "legacy_progress_unavailable" }>;
      terminal: FlightSnapshot["terminal"];
      scoreCourseMeters: number;
      crossTrackMeters: number;
    }>)
  | (NamedDisplayState & Readonly<{
      kind: "tail_flight";
      phaseCode: 5 | 6;
      controls: TailPhysicalFlightControls;
      pilotPositionTargetMeters: Available<number>;
      pilotPositionTargetNormalized: Available<number>;
      progressMeters: Available<TailFlightProgressMeters>;
    }>)
  | (NamedDisplayState & Readonly<{
      kind: "tail_result";
      controls: TailPhysicalFlightControls;
      pilotPositionTargetMeters: Available<number>;
      pilotPositionTargetNormalized: Available<number>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "terminal_progress_unavailable" }>;
      finalization: TailTerminalFinalization;
    }>)
  | (NamedDisplayState & Readonly<{
      kind: "legacy_record";
      controls: LegacyPhysicalFlightControls;
      pilotPositionTargetMeters: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      pilotPositionTargetNormalized: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "record_course_axis_unavailable" }>;
      finalization: LegacyTerminalFinalization;
    }>)
  | (NamedDisplayState & Readonly<{
      kind: "tail_record";
      controls: TailPhysicalFlightControls;
      pilotPositionTargetMeters: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      pilotPositionTargetNormalized: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "record_course_axis_unavailable" }>;
      finalization: TailTerminalFinalization;
    }>);

export function projectLegacyFlightSnapshot(snapshot: FlightSnapshot): FlightDisplaySnapshot {
  return Object.freeze({ kind: "legacy_live", positionNed: snapshot.positionNed, velocityNed: snapshot.velocityNed,
    attitudeBodyToNed: snapshot.attitudeBodyToNed, pilotPositionMeters: snapshot.pilotPositionMeters,
    pilotVelocityMetersPerSecond: snapshot.pilotVelocityMetersPerSecond,
    telemetry: snapshot.telemetry === null ? unavailable("legacy_telemetry_unavailable")
      : available(Object.freeze({ ...snapshot.telemetry, angleOfAttackRadians: flowAngle(snapshot.telemetry.angleOfAttackRadians),
          sideslipAngleRadians: flowAngle(snapshot.telemetry.sideslipAngleRadians) })),
    controls: Object.freeze({ layout: "legacy_three_axis", rollRadians: snapshot.actuatorDeflectionRadians.roll,
      pitchRadians: snapshot.actuatorDeflectionRadians.pitch, yawRadians: snapshot.actuatorDeflectionRadians.yaw }),
    stamp: Object.freeze({ kind: "legacy_projection", tick: snapshot.tick,
      contactFraction: snapshot.contactFraction === null ? unavailable("non_contact_snapshot") : available(snapshot.contactFraction), timeSeconds: snapshot.flightTimeSeconds }),
    angularRateBodyRadiansPerSecond: unavailable("legacy_body_rate_unavailable"), compositeCgPositionNedMeters: unavailable("legacy_cg_unavailable"),
    pilotPositionTargetMeters: unavailable("legacy_pilot_target_unavailable"),
    pilotPositionTargetNormalized: unavailable("legacy_pilot_target_unavailable"),
    progressMeters: unavailable("legacy_progress_unavailable"),
    terminal: snapshot.terminal, scoreCourseMeters: snapshot.scoreCourseMeters, crossTrackMeters: snapshot.crossTrackMeters });
}

export function projectTailFlightSnapshot(snapshot: TailSessionSnapshot): DisplayAvailability<FlightDisplaySnapshot, "menu_phase"> {
  if (snapshot.frame.kind === "menu") return unavailable("menu_phase");
  const state = snapshot.frame.state;
  const telemetry = snapshot.frame.telemetry;
  const values: NamedDisplayState = { positionNed: ned(state.datumPositionNedMeters), velocityNed: ned(state.datumVelocityNedMetersPerSecond),
    attitudeBodyToNed: attitude(state.attitudeBodyToNed), pilotPositionMeters: state.pilotPositionMeters,
    pilotVelocityMetersPerSecond: state.pilotVelocityMetersPerSecond, telemetry: available(displayTelemetry(telemetry)),
    stamp: Object.freeze({ kind: "exact", tick: state.tick, fraction: state.fraction, timeSeconds: state.flightTimeSeconds }),
    angularRateBodyRadiansPerSecond: available(bodyRate(state.angularRateBodyRadiansPerSecond)), compositeCgPositionNedMeters: available(ned(telemetry.compositeCgPositionNedMeters)) };
  const controls: TailPhysicalFlightControls = Object.freeze({ layout: "tail_incidence", physicalIncidence: state.physicalIncidence });
  if (snapshot.frame.kind === "result") {
    return available(Object.freeze({ ...values, kind: "tail_result", controls, pilotPositionTargetMeters: available(state.pilotPositionTargetMeters),
      pilotPositionTargetNormalized: available(state.pilotPositionTargetNormalized),
      progressMeters: unavailable("terminal_progress_unavailable"),
      finalization: snapshot.frame.finalization }));
  }
  if (snapshot.phaseCode !== 5 && snapshot.phaseCode !== 6) throw new RangeError("Tail flight display requires a Rust flight phase");
  return available(Object.freeze({ ...values, kind: "tail_flight", phaseCode: snapshot.phaseCode, controls, pilotPositionTargetMeters: available(state.pilotPositionTargetMeters),
    pilotPositionTargetNormalized: available(state.pilotPositionTargetNormalized), progressMeters: available(snapshot.frame.progressMeters) }));
}

export function projectRecordedFlightSnapshot(sample: NamedRecordSample, context: RecordQueryContext): FlightDisplaySnapshot {
  const state = sample.state;
  const values: NamedDisplayState = { positionNed: ned(state.datumPositionNedMeters), velocityNed: ned(state.datumVelocityNedMetersPerSecond),
    attitudeBodyToNed: attitude(state.attitudeBodyToNed), pilotPositionMeters: state.pilotPositionMeters,
    pilotVelocityMetersPerSecond: state.pilotVelocityMetersPerSecond, telemetry: available(displayTelemetry({ ...state.telemetry, windAtCgNedMetersPerSecond: state.windAtCgNedMetersPerSecond })),
    stamp: Object.freeze({ kind: "exact", tick: sample.tickIndex, fraction: sample.fraction, timeSeconds: sample.timeSeconds }),
    angularRateBodyRadiansPerSecond: available(bodyRate(state.angularVelocityBodyRadiansPerSecond)), compositeCgPositionNedMeters: available(ned(state.telemetry.compositeCgPositionNedMeters)) };
  if (sample.controls.layout === "legacy_three_axis" && context.controlLayout === "legacy_three_axis") {
    return Object.freeze({ ...values, kind: "legacy_record", controls: sample.controls, pilotPositionTargetMeters: unavailable("record_pilot_target_unavailable"),
      pilotPositionTargetNormalized: unavailable("record_pilot_target_unavailable"), progressMeters: unavailable("record_course_axis_unavailable"), finalization: context.finalization });
  }
  if (sample.controls.layout === "tail_incidence" && context.controlLayout === "tail_incidence") {
    return Object.freeze({ ...values, kind: "tail_record", controls: sample.controls, pilotPositionTargetMeters: unavailable("record_pilot_target_unavailable"),
      pilotPositionTargetNormalized: unavailable("record_pilot_target_unavailable"), progressMeters: unavailable("record_course_axis_unavailable"), finalization: context.finalization });
  }
  throw new RangeError("Recorded display controls and finalization require the same layout");
}

export function projectFlightRenderPose(snapshot: FlightDisplaySnapshot, initialPilotPositionMeters: number): FlightRenderPose {
  if (!Number.isFinite(initialPilotPositionMeters)) throw new RangeError("Initial pilot position must be finite");
  return Object.freeze({ datumPositionNed: snapshot.positionNed, attitudeBodyToNed: snapshot.attitudeBodyToNed,
    pilotPositionMeters: snapshot.pilotPositionMeters, initialPilotPositionMeters, simulationTimeSeconds: snapshot.stamp.timeSeconds,
    airspeedMetersPerSecond: snapshot.telemetry.kind === "available" ? snapshot.telemetry.value.airspeedMetersPerSecond : null, controls: snapshot.controls,
    windVelocityNedMetersPerSecond: snapshot.telemetry.kind === "available" ? snapshot.telemetry.value.windVelocityNedMetersPerSecond : null });
}

function ned(values: readonly [number, number, number]): NedVector {
  return Object.freeze({ north: values[0], east: values[1], down: values[2] });
}

function attitude(values: readonly [number, number, number, number]): FlightSnapshot["attitudeBodyToNed"] {
  return Object.freeze({ w: values[0], x: values[1], y: values[2], z: values[3] });
}

function bodyRate(values: readonly [number, number, number]): BodyRate {
  return Object.freeze({ roll: values[0], pitch: values[1], yaw: values[2] });
}

function displayTelemetry(telemetry: TailFlightTelemetry): FlightDisplayTelemetry {
  return Object.freeze({ altitudeMeters: telemetry.altitudeMeters, airspeedMetersPerSecond: telemetry.airspeedMetersPerSecond,
    groundspeedMetersPerSecond: telemetry.groundspeedMetersPerSecond, windVelocityNedMetersPerSecond: ned(telemetry.windAtCgNedMetersPerSecond),
    angleOfAttackRadians: flowAngle(telemetry.angleOfAttackRadians), sideslipAngleRadians: flowAngle(telemetry.sideslipAngleRadians),
    rollRadians: telemetry.attitudeEulerRadians[0], pitchRadians: telemetry.attitudeEulerRadians[1], headingRadians: telemetry.attitudeEulerRadians[2] });
}

function flowAngle(value: number | null): FlowAngle {
  return value === null ? unavailable("undefined_flow_angle") : available(value);
}

function available<Value>(value: Value): Available<Value> {
  return Object.freeze({ kind: "available", value });
}

function unavailable<Reason extends string>(reason: Reason): Readonly<{ kind: "unavailable"; reason: Reason }> {
  return Object.freeze({ kind: "unavailable", reason });
}
