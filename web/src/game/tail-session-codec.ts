import type { FlightSnapshot } from "./flight-snapshot.js";
import { boundaryInteger, boundaryNumber, boundaryObject, boundaryTag, boundaryTuple } from "./tail-boundary-values.js";
import { decodeTailTickFailure, tailFailureIsEnvelope } from "./tail-session-failure.js";
import type { TailTickFailure } from "./tail-session-failure.js";

export const TAIL_SESSION_SCHEMA_VERSION = 2;
export type TailPilotPositionCommand = Readonly<{ kind: "hold" }> | Readonly<{ kind: "set"; normalized: number }>;
export interface TailLogicalInput {
  readonly controlLayout: "tail_incidence";
  readonly noseUp: number;
  readonly turnRight: number;
  readonly desiredPitchRateRadiansPerSecond: number;
  readonly desiredYawRateRadiansPerSecond: number;
  readonly pilotPositionCommand: TailPilotPositionCommand;
}
export interface TailScenarioIdentity {
  readonly catalogVersion: number;
  readonly scenarioId: number;
  readonly scenarioVersion: number;
  readonly aircraftModelVersion: number;
  readonly environmentVersion: number;
  readonly controllerProfileVersion: number;
  readonly seedLow: number;
  readonly seedHigh: number;
}
export interface TailControlIdentity {
  readonly aircraftConfigurationId: string;
  readonly controllerProfileId: string;
}
export type TailSessionIdentity =
  | Readonly<{ kind: "unprepared" }>
  | Readonly<{ kind: "prepared"; scenario: TailScenarioIdentity; controls: TailControlIdentity }>;
type PreparedIdentity = Extract<TailSessionIdentity, { kind: "prepared" }>;
type Vector3 = readonly [number, number, number];
export interface TailFlightState {
  readonly tick: number;
  readonly fraction: number;
  readonly flightTimeSeconds: number;
  readonly datumPositionNedMeters: Vector3;
  readonly datumVelocityNedMetersPerSecond: Vector3;
  readonly attitudeBodyToNed: readonly [number, number, number, number];
  readonly angularRateBodyRadiansPerSecond: Vector3;
  readonly pilotPositionMeters: number;
  readonly pilotVelocityMetersPerSecond: number;
  readonly pilotPositionTargetMeters: number;
  readonly pilotPositionTargetNormalized: number;
  readonly physicalIncidence: Readonly<{ horizontalTailRadians: number; verticalTailRadians: number }>;
}
export interface TailFlightTelemetry {
  readonly compositeCgPositionNedMeters: Vector3;
  readonly altitudeMeters: number;
  readonly airspeedMetersPerSecond: number;
  readonly groundspeedMetersPerSecond: number;
  readonly windAtCgNedMetersPerSecond: Vector3;
  readonly angleOfAttackRadians: number | null;
  readonly sideslipAngleRadians: number | null;
  readonly attitudeEulerRadians: Vector3;
}
export type TailTerminalReason = "water_contact" | "time_limit" | "manual_abort" | "out_of_valid_envelope" | "fatal_simulation_error";
export interface TailTerminalFinalization {
  readonly reason: TailTerminalReason;
  readonly disposition: "complete" | "interrupted" | "failed";
  readonly terminalTick: number;
  readonly terminalFraction: number;
  readonly scoreMeters: Vector3 | null;
  readonly failure: TailTickFailure | null;
}
interface TailSessionEnvelope {
  readonly schemaVersion: 2;
  readonly controlLayout: "tail_incidence";
  readonly controlModeCode: 0 | 1 | 2;
}
export type TailSessionSnapshot = TailSessionEnvelope & (
  | Readonly<{ phaseCode: 0 | 1; identity: Extract<TailSessionIdentity, { kind: "unprepared" }>; frame: Readonly<{ kind: "menu" }> }>
  | Readonly<{ phaseCode: 2 | 3 | 4 | 8; identity: PreparedIdentity; frame: Readonly<{ kind: "menu" }> }>
  | Readonly<{ phaseCode: 5 | 6; identity: PreparedIdentity;
      frame: Readonly<{ kind: "flight"; state: TailFlightState; telemetry: TailFlightTelemetry }> }>
  | Readonly<{ phaseCode: 7; identity: PreparedIdentity;
      frame: Readonly<{ kind: "result"; state: TailFlightState; telemetry: TailFlightTelemetry; finalization: TailTerminalFinalization }> }>
);
export type FlightSnapshotBoundary =
  | Readonly<{ schemaVersion: 1; controlLayout: "legacy_three_axis"; snapshot: FlightSnapshot }>
  | TailSessionSnapshot;

export function encodeTailLogicalInput(input: TailLogicalInput): string {
  const object = boundaryObject(input, ["controlLayout", "noseUp", "turnRight", "desiredPitchRateRadiansPerSecond",
    "desiredYawRateRadiansPerSecond", "pilotPositionCommand"]);
  boundaryTag(object.controlLayout, ["tail_incidence"]);
  const command = input.pilotPositionCommand;
  const position = command.kind === "hold"
    ? boundaryObject(command, ["kind"])
    : boundaryObject(command, ["kind", "normalized"]);
  boundaryTag(position.kind, ["hold", "set"]);
  return JSON.stringify({
    schema_version: TAIL_SESSION_SCHEMA_VERSION,
    control_layout: "tail_incidence",
    nose_up: boundaryNumber(input.noseUp, -1, 1),
    turn_right: boundaryNumber(input.turnRight, -1, 1),
    desired_pitch_rate_rad_s: boundaryNumber(input.desiredPitchRateRadiansPerSecond),
    desired_yaw_rate_rad_s: boundaryNumber(input.desiredYawRateRadiansPerSecond),
    pilot_position_command: command.kind === "hold" ? { kind: "hold" }
      : { kind: "set", normalized: boundaryNumber(command.normalized, -1, 1) }
  });
}

export function parseTailSessionSnapshot(json: string, physicsHz = 100): TailSessionSnapshot {
  if (json.length > 16_384) throw new RangeError("Tail snapshot exceeds its bounded envelope");
  boundaryNumber(physicsHz, Number.MIN_VALUE);
  const parsed: unknown = JSON.parse(json);
  const document = boundaryObject(parsed, ["schema_version", "control_layout", "phase_code", "control_mode_code",
    "scenario", "control_identity", "frame"]);
  if (document.schema_version !== TAIL_SESSION_SCHEMA_VERSION || document.control_layout !== "tail_incidence") {
    throw new RangeError("Unsupported tail snapshot schema or control layout");
  }
  const controlModeCode = boundaryInteger(document.control_mode_code, 0, 2) as 0 | 1 | 2;
  const envelope: TailSessionEnvelope = { schemaVersion: TAIL_SESSION_SCHEMA_VERSION, controlLayout: "tail_incidence", controlModeCode };
  const identity = decodeIdentity(document.scenario, document.control_identity);
  const phase = boundaryInteger(document.phase_code, 0, 8);
  const frameTag = boundaryObjectWithKind(document.frame);
  if (frameTag === "menu") {
    boundaryObject(document.frame, ["kind"]);
    const frame = Object.freeze({ kind: "menu" as const });
    if (phase === 0 || phase === 1) {
      if (identity.kind !== "unprepared") throw new RangeError("Title and Setup cannot retain sealed identity");
      return Object.freeze({ ...envelope, phaseCode: phase, identity, frame });
    }
    if (phase !== 2 && phase !== 3 && phase !== 4 && phase !== 8) throw new RangeError("Flight phase requires a flight or terminal frame");
    if (identity.kind !== "prepared") throw new RangeError("Prepared phase requires sealed identity");
    return Object.freeze({ ...envelope, phaseCode: phase, identity, frame });
  }
  if (identity.kind !== "prepared") throw new RangeError("Flight snapshot requires sealed identity");
  const frame = boundaryObject(document.frame, frameTag === "flight"
    ? ["kind", "state", "telemetry"] : ["kind", "state", "telemetry", "finalization"]);
  const state = decodeState(frame.state, physicsHz);
  const telemetry = decodeTelemetry(frame.telemetry);
  if (frameTag === "flight") {
    if ((phase !== 5 && phase !== 6) || state.fraction !== 0) throw new RangeError("Airborne frame requires an integer flight phase");
    return Object.freeze({ ...envelope, phaseCode: phase, identity, frame: Object.freeze({ kind: frameTag, state, telemetry }) });
  }
  if (phase !== 7) throw new RangeError("Terminal frame requires Result phase");
  const finalization = decodeTailTerminalFinalization(frame.finalization);
  if (finalization.terminalTick !== state.tick || finalization.terminalFraction !== state.fraction) {
    throw new RangeError("Terminal state and finalization must share one exact stamp");
  }
  return Object.freeze({ ...envelope, phaseCode: phase, identity, frame: Object.freeze({ kind: frameTag, state, telemetry, finalization }) });
}

function boundaryObjectWithKind(value: unknown): "menu" | "flight" | "result" {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new RangeError("Missing tail frame");
  return boundaryTag((value as Record<string, unknown>).kind, ["menu", "flight", "result"]);
}

function decodeIdentity(scenarioValue: unknown, controlsValue: unknown): TailSessionIdentity {
  if (scenarioValue === null && controlsValue === null) return Object.freeze({ kind: "unprepared" });
  return Object.freeze({ kind: "prepared", scenario: decodeTailScenarioIdentity(scenarioValue), controls: decodeTailControlIdentity(controlsValue) });
}

export function decodeTailScenarioIdentity(value: unknown): TailScenarioIdentity {
  const scenario = boundaryObject(value, ["catalog_version", "scenario_id", "scenario_version", "aircraft_model_version",
    "environment_version", "controller_profile_version", "seed_low", "seed_high"]);
  const version = (value: unknown) => boundaryInteger(value, 1, 0xffff_ffff);
  return Object.freeze({
    catalogVersion: version(scenario.catalog_version), scenarioId: boundaryInteger(scenario.scenario_id, 0, 0xffff_ffff),
    scenarioVersion: version(scenario.scenario_version), aircraftModelVersion: version(scenario.aircraft_model_version),
    environmentVersion: version(scenario.environment_version), controllerProfileVersion: version(scenario.controller_profile_version),
    seedLow: boundaryInteger(scenario.seed_low, 0, 0xffff_ffff), seedHigh: boundaryInteger(scenario.seed_high, 0, 0xffff_ffff)
  });
}

export function decodeTailControlIdentity(value: unknown): TailControlIdentity {
  const controls = boundaryObject(value, ["aircraft_configuration_id", "controller_profile_id"]);
  return Object.freeze({ aircraftConfigurationId: identityName(controls.aircraft_configuration_id),
    controllerProfileId: identityName(controls.controller_profile_id) });
}

function identityName(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0 || new TextEncoder().encode(value).length > 128 || /\p{Cc}/u.test(value)) {
    throw new RangeError("Invalid tail model or controller identity");
  }
  return value;
}

function vector3(value: unknown): Vector3 {
  return boundaryTuple(value, 3) as Vector3;
}

function decodeState(value: unknown, physicsHz: number): TailFlightState {
  const state = boundaryObject(value, ["tick", "fraction", "flight_time_s", "datum_position_ned_m", "datum_velocity_ned_mps",
    "attitude_body_to_ned", "angular_rate_body_rad_s", "pilot_position_m", "pilot_velocity_mps", "pilot_position_target_m", "pilot_position_target_normalized", "physical_incidence"]);
  const tick = boundaryInteger(state.tick);
  const fraction = boundaryNumber(state.fraction, 0, 1);
  const flightTimeSeconds = boundaryNumber(state.flight_time_s, 0);
  if (Math.abs(flightTimeSeconds - (tick + fraction) / physicsHz) > 1e-10) throw new RangeError("Tail flight time disagrees with its stamp");
  const attitude = boundaryTuple(state.attitude_body_to_ned, 4) as readonly [number, number, number, number];
  if (Math.abs(Math.hypot(...attitude) - 1) > 1e-8) throw new RangeError("Tail flight attitude must be a unit quaternion");
  return Object.freeze({ tick, fraction, flightTimeSeconds,
    datumPositionNedMeters: vector3(state.datum_position_ned_m), datumVelocityNedMetersPerSecond: vector3(state.datum_velocity_ned_mps),
    attitudeBodyToNed: attitude, angularRateBodyRadiansPerSecond: vector3(state.angular_rate_body_rad_s),
    pilotPositionMeters: boundaryNumber(state.pilot_position_m), pilotVelocityMetersPerSecond: boundaryNumber(state.pilot_velocity_mps),
    pilotPositionTargetMeters: boundaryNumber(state.pilot_position_target_m), pilotPositionTargetNormalized: boundaryNumber(state.pilot_position_target_normalized, -1, 1),
    physicalIncidence: decodeTailPhysicalIncidence(state.physical_incidence) });
}

export function decodeTailPhysicalIncidence(value: unknown): TailFlightState["physicalIncidence"] {
  const incidence = boundaryObject(value, ["horizontal_tail_rad", "vertical_tail_rad"]);
  return Object.freeze({ horizontalTailRadians: boundaryNumber(incidence.horizontal_tail_rad, -0.2, 0.2),
    verticalTailRadians: boundaryNumber(incidence.vertical_tail_rad, -0.2, 0.2) });
}

function decodeTelemetry(value: unknown): TailFlightTelemetry {
  const telemetry = boundaryObject(value, ["composite_cg_position_ned_m", "altitude_m", "airspeed_mps", "groundspeed_mps",
    "wind_at_cg_ned_mps", "angle_of_attack_rad", "sideslip_angle_rad", "attitude_euler_rad"]);
  const angle = (value: unknown) => value === null ? null : boundaryNumber(value);
  return Object.freeze({ compositeCgPositionNedMeters: vector3(telemetry.composite_cg_position_ned_m),
    altitudeMeters: boundaryNumber(telemetry.altitude_m), airspeedMetersPerSecond: boundaryNumber(telemetry.airspeed_mps, 0),
    groundspeedMetersPerSecond: boundaryNumber(telemetry.groundspeed_mps, 0), windAtCgNedMetersPerSecond: vector3(telemetry.wind_at_cg_ned_mps),
    angleOfAttackRadians: angle(telemetry.angle_of_attack_rad), sideslipAngleRadians: angle(telemetry.sideslip_angle_rad),
    attitudeEulerRadians: vector3(telemetry.attitude_euler_rad) });
}

export function decodeTailTerminalFinalization(value: unknown): TailTerminalFinalization {
  const terminal = boundaryObject(value, ["reason", "disposition", "terminal_tick", "terminal_fraction", "score_m", "failure"]);
  const reason = boundaryTag(terminal.reason, ["water_contact", "time_limit", "manual_abort", "out_of_valid_envelope", "fatal_simulation_error"]);
  const disposition = boundaryTag(terminal.disposition, ["complete", "interrupted", "failed"]);
  const expected = reason === "water_contact" || reason === "time_limit" ? "complete" : reason === "manual_abort" ? "interrupted" : "failed";
  const failure = terminal.failure === null ? null : decodeTailTickFailure(terminal.failure);
  if (disposition !== expected || (failure === null && reason === "out_of_valid_envelope")
      || (failure !== null && reason !== (tailFailureIsEnvelope(failure) ? "out_of_valid_envelope" : "fatal_simulation_error"))) {
    throw new RangeError("Tail finalization reason, disposition or cause is contradictory");
  }
  return Object.freeze({ reason, disposition, terminalTick: boundaryInteger(terminal.terminal_tick),
    terminalFraction: boundaryNumber(terminal.terminal_fraction, 0, 1), scoreMeters: terminal.score_m === null ? null : decodeRecordedDistanceScore(terminal.score_m), failure });
}

export function decodeRecordedDistanceScore(value: unknown): Vector3 {
  const score = vector3(value);
  const expectedNet = boundaryNumber(Math.hypot(score[0], score[1]), 0);
  if (score[2] < 0 || Math.abs(expectedNet - score[2]) > 1e-9 * Math.max(1, expectedNet)) {
    throw new RangeError("Tail distance score violates its horizontal displacement invariant");
  }
  return score;
}
