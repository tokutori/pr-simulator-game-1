import { boundaryNumber, boundaryObject, boundaryTuple } from "./tail-boundary-values.js";
import { decodeTailTickFailure, tailFailureIsEnvelope } from "./tail-session-failure.js";
import type { TailTickFailure } from "./tail-session-failure.js";
import type { TailInputDocument } from "../generated/tail-input.js";
import type { TailSnapshotDocument, TailStateDocument, TailTelemetryDocument, TailProgressDocument } from "../generated/tail-snapshot.js";
import type { EnvironmentIdentity } from "../generated/tail-scenario.js";
import type { FlightRecordTailIdentityDocument } from "../generated/tail-identity.js";
import type { TailFlightRecordFinalizationDocument } from "../generated/tail-finalization.js";
import { validateTailInputDocument, validateTailSnapshotDocument, validateTailPreparedLaunchDocument,
  validateEnvironmentIdentity, validateFlightRecordTailIdentityDocument, validateTailIncidenceDocument,
  validateTailFlightRecordFinalizationDocument } from "../generated/game-ui-validators.js";

export const TAIL_SESSION_SCHEMA_VERSION: TailInputDocument["schema_version"] = 2;
export type TailPilotPositionCommand = TailInputDocument["pilot_position_command"];
export interface TailLogicalInput {
  readonly controlLayout: TailInputDocument["control_layout"];
  readonly noseUp: TailInputDocument["nose_up"];
  readonly turnRight: TailInputDocument["turn_right"];
  readonly desiredPitchRateRadiansPerSecond: TailInputDocument["desired_pitch_rate_rad_s"];
  readonly desiredYawRateRadiansPerSecond: TailInputDocument["desired_yaw_rate_rad_s"];
  readonly pilotPositionCommand: TailPilotPositionCommand;
}
export interface TailScenarioIdentity {
  readonly catalogVersion: EnvironmentIdentity["catalog_version"];
  readonly scenarioId: EnvironmentIdentity["scenario_id"];
  readonly scenarioVersion: EnvironmentIdentity["scenario_version"];
  readonly aircraftModelVersion: EnvironmentIdentity["aircraft_model_version"];
  readonly environmentVersion: EnvironmentIdentity["environment_version"];
  readonly controllerProfileVersion: EnvironmentIdentity["controller_profile_version"];
  readonly seedLow: EnvironmentIdentity["seed_low"];
  readonly seedHigh: EnvironmentIdentity["seed_high"];
}
export interface TailControlIdentity {
  readonly aircraftConfigurationId: FlightRecordTailIdentityDocument["aircraft_configuration_id"];
  readonly controllerProfileId: FlightRecordTailIdentityDocument["controller_profile_id"];
}
export type TailSessionIdentity =
  | Readonly<{ kind: "unprepared" }>
  | Readonly<{ kind: "prepared"; scenario: TailScenarioIdentity; controls: TailControlIdentity }>;
type PreparedIdentity = Extract<TailSessionIdentity, { kind: "prepared" }>;
type Vector3 = TailStateDocument["datum_position_ned_m"];
export interface TailFlightState {
  readonly tick: TailStateDocument["tick"];
  readonly fraction: TailStateDocument["fraction"];
  readonly flightTimeSeconds: TailStateDocument["flight_time_s"];
  readonly datumPositionNedMeters: TailStateDocument["datum_position_ned_m"];
  readonly datumVelocityNedMetersPerSecond: TailStateDocument["datum_velocity_ned_mps"];
  readonly attitudeBodyToNed: TailStateDocument["attitude_body_to_ned"];
  readonly angularRateBodyRadiansPerSecond: TailStateDocument["angular_rate_body_rad_s"];
  readonly pilotPositionMeters: TailStateDocument["pilot_position_m"];
  readonly pilotVelocityMetersPerSecond: TailStateDocument["pilot_velocity_mps"];
  readonly pilotPositionTargetMeters: TailStateDocument["pilot_position_target_m"];
  readonly pilotPositionTargetNormalized: TailStateDocument["pilot_position_target_normalized"];
  readonly physicalIncidence: Readonly<{
    horizontalTailRadians: TailStateDocument["physical_incidence"]["horizontal_tail_rad"];
    verticalTailRadians: TailStateDocument["physical_incidence"]["vertical_tail_rad"];
  }>;
}
export interface TailFlightTelemetry {
  readonly compositeCgPositionNedMeters: TailTelemetryDocument["composite_cg_position_ned_m"];
  readonly altitudeMeters: TailTelemetryDocument["altitude_m"];
  readonly airspeedMetersPerSecond: TailTelemetryDocument["airspeed_mps"];
  readonly groundspeedMetersPerSecond: TailTelemetryDocument["groundspeed_mps"];
  readonly windAtCgNedMetersPerSecond: TailTelemetryDocument["wind_at_cg_ned_mps"];
  readonly angleOfAttackRadians: TailTelemetryDocument["angle_of_attack_rad"];
  readonly sideslipAngleRadians: TailTelemetryDocument["sideslip_angle_rad"];
  readonly attitudeEulerRadians: TailTelemetryDocument["attitude_euler_rad"];
}
export interface TailFlightProgressMeters {
  readonly courseParallelMeters: TailProgressDocument["course_parallel_m"];
  readonly crossTrackMeters: TailProgressDocument["cross_track_m"];
  readonly netHorizontalMeters: TailProgressDocument["net_horizontal_m"];
}
export type TailTerminalReason = TailFlightRecordFinalizationDocument["reason"];
export interface TailTerminalFinalization {
  readonly reason: TailTerminalReason;
  readonly disposition: TailFlightRecordFinalizationDocument["disposition"];
  readonly terminalTick: TailFlightRecordFinalizationDocument["terminal_tick"];
  readonly terminalFraction: TailFlightRecordFinalizationDocument["terminal_fraction"];
  readonly scoreMeters: TailFlightRecordFinalizationDocument["score_m"];
  readonly failure: TailTickFailure | null;
}
interface TailSessionEnvelope {
  readonly schemaVersion: TailSnapshotDocument["schema_version"];
  readonly controlLayout: TailSnapshotDocument["control_layout"];
  readonly controlModeCode: 0 | 1 | 2;
}
export type TailSessionSnapshot = TailSessionEnvelope & (
  | Readonly<{ phaseCode: 0 | 1; identity: Extract<TailSessionIdentity, { kind: "unprepared" }>; frame: Readonly<{ kind: "menu" }> }>
  | Readonly<{ phaseCode: 2 | 3 | 4 | 8; identity: PreparedIdentity; frame: Readonly<{ kind: "menu" }> }>
  | Readonly<{ phaseCode: 5 | 6; identity: PreparedIdentity;
      frame: Readonly<{ kind: "flight"; state: TailFlightState; telemetry: TailFlightTelemetry; progressMeters: TailFlightProgressMeters }> }>
  | Readonly<{ phaseCode: 7; identity: PreparedIdentity;
      frame: Readonly<{ kind: "result"; state: TailFlightState; telemetry: TailFlightTelemetry; finalization: TailTerminalFinalization }> }>
);
export type TailPreparedLaunchSnapshot =
  | Readonly<{ kind: "unavailable"; phaseCode: number }>
  | Readonly<{ kind: "prepared"; phaseCode: 2 | 3 | 4 | 8; identity: PreparedIdentity; state: TailFlightState }>;

export function parseTailPreparedLaunchSnapshot(json: string, physicsHz: number): TailPreparedLaunchSnapshot {
  if (json.length > 16_384) throw new RangeError("Prepared launch snapshot exceeds its bounded envelope");
  boundaryNumber(physicsHz, Number.MIN_VALUE);
  const document = parseBoundaryJson(json);
  if (!validateTailPreparedLaunchDocument(document)) {
    throw new RangeError("Unsupported prepared launch schema or control layout");
  }
  const phaseCode = document.phase_code;
  const preparedPhase = phaseCode === 2 || phaseCode === 3 || phaseCode === 4 || phaseCode === 8;
  const preview = document.preview;
  if (preview.kind === "unavailable") {
    if (preparedPhase) throw new RangeError("Prepared phase requires the sealed initial state");
    return Object.freeze({ kind: preview.kind, phaseCode });
  }
  if (!preparedPhase) throw new RangeError("Initial launch projection requires a preparation phase");
  const identity = decodeIdentity(preview.scenario, preview.control_identity);
  if (identity.kind !== "prepared") throw new RangeError("Initial launch projection requires sealed identity");
  const state = decodeState(preview.state, physicsHz);
  if (state.tick !== 0 || state.fraction !== 0 || state.flightTimeSeconds !== 0) {
    throw new RangeError("Initial launch projection must retain its zero tick");
  }
  return Object.freeze({ kind: preview.kind, phaseCode, identity, state });
}

export function encodeTailLogicalInput(input: TailLogicalInput): string {
  const object = boundaryObject(input, ["controlLayout", "noseUp", "turnRight", "desiredPitchRateRadiansPerSecond",
    "desiredYawRateRadiansPerSecond", "pilotPositionCommand"]);
  const command = object.pilotPositionCommand;
  const pilotPositionCommand = typeof command === "object" && command !== null && !Array.isArray(command)
    ? Object.fromEntries(Object.entries(command)) : command;
  const document = {
    schema_version: TAIL_SESSION_SCHEMA_VERSION,
    control_layout: object.controlLayout,
    nose_up: object.noseUp,
    turn_right: object.turnRight,
    desired_pitch_rate_rad_s: object.desiredPitchRateRadiansPerSecond,
    desired_yaw_rate_rad_s: object.desiredYawRateRadiansPerSecond,
    pilot_position_command: pilotPositionCommand
  };
  if (!validateTailInputDocument(document)) {
    throw new RangeError(`Invalid tail input: ${(validateTailInputDocument.errors ?? [])
      .map((error) => `${error.instancePath || "/"} ${error.message ?? error.keyword}`).join("; ")}`);
  }
  return JSON.stringify(document);
}

export function parseTailSessionSnapshot(json: string, physicsHz = 100): TailSessionSnapshot {
  if (json.length > 16_384) throw new RangeError("Tail snapshot exceeds its bounded envelope");
  boundaryNumber(physicsHz, Number.MIN_VALUE);
  const document = parseBoundaryJson(json);
  if (!validateTailSnapshotDocument(document)) {
    throw new RangeError("Unsupported tail snapshot schema or control layout");
  }
  const controlModeCode = document.control_mode_code as 0 | 1 | 2;
  const envelope: TailSessionEnvelope = { schemaVersion: TAIL_SESSION_SCHEMA_VERSION, controlLayout: "tail_incidence", controlModeCode };
  const identity = decodeIdentity(document.scenario, document.control_identity);
  const phase = document.phase_code;
  const frame = document.frame;
  if (frame.kind === "menu") {
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
  const state = decodeState(frame.state, physicsHz);
  const telemetry = decodeTelemetry(frame.telemetry);
  if (frame.kind === "flight") {
    if ((phase !== 5 && phase !== 6) || state.fraction !== 0) throw new RangeError("Airborne frame requires an integer flight phase");
    return Object.freeze({ ...envelope, phaseCode: phase, identity, frame: Object.freeze({ kind: frame.kind, state, telemetry,
      progressMeters: decodeTailFlightProgress(frame.progress_m) }) });
  }
  if (phase !== 7) throw new RangeError("Terminal frame requires Result phase");
  const finalization = decodeTailTerminalFinalization(frame.finalization);
  if (finalization.terminalTick !== state.tick || finalization.terminalFraction !== state.fraction) {
    throw new RangeError("Terminal state and finalization must share one exact stamp");
  }
  return Object.freeze({ ...envelope, phaseCode: phase, identity, frame: Object.freeze({ kind: frame.kind, state, telemetry, finalization }) });
}

function parseBoundaryJson(json: string): unknown {
  return JSON.parse(json, (_key, value: unknown) => typeof value === "object" && value !== null && !Array.isArray(value)
    ? Object.setPrototypeOf(value, null) as Record<string, unknown> : value) as unknown;
}

function ownBoundaryValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(ownBoundaryValue);
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? Object.setPrototypeOf(Object.fromEntries(Object.entries(value).map(([key, field]) => [key, ownBoundaryValue(field)])), null) as Record<string, unknown>
    : value;
}

function decodeIdentity(scenarioValue: TailSnapshotDocument["scenario"], controlsValue: TailSnapshotDocument["control_identity"]): TailSessionIdentity {
  if (scenarioValue === null && controlsValue === null) return Object.freeze({ kind: "unprepared" });
  return Object.freeze({ kind: "prepared", scenario: decodeTailScenarioIdentity(scenarioValue), controls: decodeTailControlIdentity(controlsValue) });
}

export function decodeTailScenarioIdentity(value: unknown): TailScenarioIdentity {
  const scenario = ownBoundaryValue(value);
  if (!validateEnvironmentIdentity(scenario)) throw new RangeError("Invalid tail scenario identity");
  return Object.freeze({
    catalogVersion: scenario.catalog_version, scenarioId: scenario.scenario_id,
    scenarioVersion: scenario.scenario_version, aircraftModelVersion: scenario.aircraft_model_version,
    environmentVersion: scenario.environment_version, controllerProfileVersion: scenario.controller_profile_version,
    seedLow: scenario.seed_low, seedHigh: scenario.seed_high
  });
}

export function decodeTailControlIdentity(value: unknown): TailControlIdentity {
  const controls = ownBoundaryValue(value);
  if (!validateFlightRecordTailIdentityDocument(controls)) throw new RangeError("Invalid tail control identity");
  return Object.freeze({ aircraftConfigurationId: identityName(controls.aircraft_configuration_id),
    controllerProfileId: identityName(controls.controller_profile_id) });
}

function identityName(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0 || new TextEncoder().encode(value).length > 128 || /\p{Cc}/u.test(value)) {
    throw new RangeError("Invalid tail model or controller identity");
  }
  return value;
}

function vector3(value: Vector3): Vector3 {
  return Object.freeze([...value]);
}

function decodeState(state: TailStateDocument, physicsHz: number): TailFlightState {
  const tick = state.tick;
  const fraction = state.fraction;
  const flightTimeSeconds = state.flight_time_s;
  if (Math.abs(flightTimeSeconds - (tick + fraction) / physicsHz) > 1e-10) throw new RangeError("Tail flight time disagrees with its stamp");
  const attitude: TailStateDocument["attitude_body_to_ned"] = Object.freeze([...state.attitude_body_to_ned]);
  if (Math.abs(Math.hypot(...attitude) - 1) > 1e-8) throw new RangeError("Tail flight attitude must be a unit quaternion");
  return Object.freeze({ tick, fraction, flightTimeSeconds,
    datumPositionNedMeters: vector3(state.datum_position_ned_m), datumVelocityNedMetersPerSecond: vector3(state.datum_velocity_ned_mps),
    attitudeBodyToNed: attitude, angularRateBodyRadiansPerSecond: vector3(state.angular_rate_body_rad_s),
    pilotPositionMeters: state.pilot_position_m, pilotVelocityMetersPerSecond: state.pilot_velocity_mps,
    pilotPositionTargetMeters: state.pilot_position_target_m, pilotPositionTargetNormalized: state.pilot_position_target_normalized,
    physicalIncidence: decodeTailPhysicalIncidence(state.physical_incidence) });
}

export function decodeTailPhysicalIncidence(value: unknown): TailFlightState["physicalIncidence"] {
  const incidence = ownBoundaryValue(value);
  if (!validateTailIncidenceDocument(incidence)) throw new RangeError("Invalid tail physical incidence");
  return Object.freeze({ horizontalTailRadians: incidence.horizontal_tail_rad,
    verticalTailRadians: incidence.vertical_tail_rad });
}

function decodeTelemetry(telemetry: TailTelemetryDocument): TailFlightTelemetry {
  return Object.freeze({ compositeCgPositionNedMeters: vector3(telemetry.composite_cg_position_ned_m),
    altitudeMeters: telemetry.altitude_m, airspeedMetersPerSecond: telemetry.airspeed_mps,
    groundspeedMetersPerSecond: telemetry.groundspeed_mps, windAtCgNedMetersPerSecond: vector3(telemetry.wind_at_cg_ned_mps),
    angleOfAttackRadians: telemetry.angle_of_attack_rad, sideslipAngleRadians: telemetry.sideslip_angle_rad,
    attitudeEulerRadians: vector3(telemetry.attitude_euler_rad) });
}

export function decodeTailTerminalFinalization(value: unknown): TailTerminalFinalization {
  const terminal = ownBoundaryValue(value);
  if (!validateTailFlightRecordFinalizationDocument(terminal)) throw new RangeError("Invalid tail finalization");
  const reason = terminal.reason;
  const disposition = terminal.disposition;
  const expected = reason === "water_contact" || reason === "time_limit" ? "complete" : reason === "manual_abort" ? "interrupted" : "failed";
  const failure = terminal.failure === null ? null : decodeTailTickFailure(terminal.failure);
  if (disposition !== expected || (failure === null && reason === "out_of_valid_envelope")
      || (failure !== null && reason !== (tailFailureIsEnvelope(failure) ? "out_of_valid_envelope" : "fatal_simulation_error"))) {
    throw new RangeError("Tail finalization reason, disposition or cause is contradictory");
  }
  return Object.freeze({ reason, disposition, terminalTick: terminal.terminal_tick,
    terminalFraction: terminal.terminal_fraction, scoreMeters: terminal.score_m === null ? null : decodeRecordedDistanceScore(terminal.score_m), failure });
}

export function decodeRecordedDistanceScore(value: unknown): Vector3 {
  const score = boundaryTuple(value, 3) as Vector3;
  const expectedNet = boundaryNumber(Math.hypot(score[0], score[1]), 0);
  if (score[2] < 0 || Math.abs(expectedNet - score[2]) > 1e-9 * Math.max(1, expectedNet)) {
    throw new RangeError("Tail distance score violates its horizontal displacement invariant");
  }
  return score;
}

function decodeTailFlightProgress(progress: TailProgressDocument): TailFlightProgressMeters {
  const course = progress.course_parallel_m;
  const crossTrack = progress.cross_track_m;
  const net = progress.net_horizontal_m;
  decodeRecordedDistanceScore([course, crossTrack, net]);
  return Object.freeze({ courseParallelMeters: course, crossTrackMeters: crossTrack, netHorizontalMeters: net });
}
