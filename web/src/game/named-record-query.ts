import type { PlaybackContext } from "../generated/record-context.js";
import type { RecordedPlaybackSample } from "../generated/record-sample.js";
import { validatePlaybackContext, validateRecordedPlaybackSample, validateAnalysisDocument } from "../generated/game-ui-validators.js";
import { boundaryInteger, boundaryNumber, ownBoundaryValue } from "./tail-boundary-values.js";
import { decodeRecordedDifficulty, decodeTailControlIdentity, decodeTailPhysicalIncidence, decodeTailScenarioIdentity, decodeTailTerminalFinalization } from "./tail-session-codec.js";
import type { RecordedDifficulty, TailControlIdentity, TailFlightTelemetry, TailScenarioIdentity, TailSessionSnapshot, TailTerminalFinalization } from "./tail-session-codec.js";
import type { PhysicalFlightControls } from "../render/contracts/flight-controls.js";

export type { RecordedDifficulty, RecordedHudProfile } from "./tail-session-codec.js";

type Vector3 = RecordedPlaybackSample["state"]["datum_position_ned_m"];
export type RecordedPhysicalControls = PhysicalFlightControls;
export interface NamedRecordState {
  readonly datumPositionNedMeters: Vector3;
  readonly datumVelocityNedMetersPerSecond: Vector3;
  readonly attitudeBodyToNed: RecordedPlaybackSample["state"]["attitude_body_to_ned"];
  readonly angularVelocityBodyRadiansPerSecond: Vector3;
  readonly pilotPositionMeters: number;
  readonly pilotVelocityMetersPerSecond: number;
  readonly windAtCgNedMetersPerSecond: Vector3;
  readonly telemetry: Omit<TailFlightTelemetry, "windAtCgNedMetersPerSecond">;
}
export interface NamedRecordSample {
  readonly schemaVersion: 2;
  readonly tickIndex: number;
  readonly fraction: number;
  readonly timeSeconds: number;
  readonly state: NamedRecordState;
  readonly controls: RecordedPhysicalControls;
}
export type RecordQueryContext = Readonly<{ controlLayout: PlaybackContext["control_layout"]; scenario: TailScenarioIdentity;
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
interface PlaybackEnvelope {
  readonly schemaVersion: 2;
  readonly scenario: TailScenarioIdentity;
  readonly difficulty: RecordedDifficulty;
}
export type NamedReplayContext = PlaybackEnvelope & Readonly<{ phase: "replay"; controlLayout: PlaybackContext["control_layout"];
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedAttractContext = PlaybackEnvelope & Readonly<{ phase: "attract"; controlLayout: PlaybackContext["control_layout"];
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedPlaybackContext = NamedReplayContext | NamedAttractContext;
export type NamedResultContext = PlaybackEnvelope & Readonly<{ phase: "result"; controlLayout: PlaybackContext["control_layout"];
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedRecordContext = NamedResultContext | NamedPlaybackContext;
export interface NamedPlaybackClock {
  readonly timeSeconds: number;
  readonly rateCode: 0 | 1 | 2;
  readonly kind: "paused" | "playing";
}
export type NamedReplayClock = NamedPlaybackClock;

export function tailResultRecordContext(snapshot: TailSessionSnapshot): RecordQueryContext {
  if (snapshot.frame.kind !== "result" || snapshot.identity.kind !== "prepared") throw new RangeError("Recorded Result queries require a terminal Rust snapshot");
  return Object.freeze({ controlLayout: "tail_incidence", scenario: snapshot.identity.scenario,
    controlIdentity: snapshot.identity.controls, finalization: snapshot.frame.finalization });
}

export function parseNamedReplayContext(json: string): NamedReplayContext {
  const context = parseNamedPlaybackContext(json);
  if (context.phase !== "replay") throw new RangeError("Named Replay requires a Replay context");
  return context;
}

export function parseNamedAttractContext(json: string): NamedAttractContext {
  const context = parseNamedPlaybackContext(json);
  if (context.phase !== "attract") throw new RangeError("Named Attract requires an Attract context");
  return context;
}

export function parseNamedPlaybackContext(json: string): NamedPlaybackContext {
  const context = decodeNamedRecordContext(boundedJson(json, 16_384));
  if (context.phase === "result") throw new RangeError("Named playback requires Replay or Attract");
  return context;
}

export function decodeNamedRecordContext(value: unknown): NamedRecordContext {
  const document = ownBoundaryValue(value);
  if (!validatePlaybackContext(document)) throw new RangeError("Invalid named playback context structure");
  const phase = document.phase;
  const layout = document.control_layout;
  const finalization = document.finalization;
  if (finalization.layout !== layout) throw new RangeError("Playback finalization and physical control layout disagree");
  const envelope: PlaybackEnvelope = { schemaVersion: 2, scenario: decodeTailScenarioIdentity(document.scenario),
    difficulty: decodeRecordedDifficulty(document.difficulty) };

  return Object.freeze({ ...envelope, phase, controlLayout: layout, controlIdentity: decodeTailControlIdentity(document.control_identity),
    finalization: decodeTailTerminalFinalization(finalization.value) });
}

export function parseNamedRecordSample(json: string, physicsHz: number, context: RecordQueryContext): NamedRecordSample {
  return decodeNamedRecordSample(boundedJson(json, 16_384), physicsHz, context);
}

export function parseNamedAnalysisSamples(json: string, physicsHz: number, context: RecordQueryContext): readonly NamedRecordSample[] {
  const document = ownBoundaryValue(boundedJson(json, 16 * 1024 * 1024));
  if (!validateAnalysisDocument(document)) throw new RangeError("Invalid named Analysis sample batch structure");
  const samples = document.samples.map((sample) => projectRecordSample(sample, physicsHz, context));
  const first = samples[0];
  const last = samples.at(-1);
  if (first?.tickIndex !== 0 || first.fraction !== 0 || last?.tickIndex !== context.finalization.terminalTick
      || last.fraction !== context.finalization.terminalFraction) throw new RangeError("Analysis endpoints disagree with the recorded terminal stamp");
  for (let index = 1; index < samples.length; index += 1) {
    const previous = samples[index - 1];
    const current = samples[index];
    if (previous === undefined || current === undefined || current.timeSeconds < previous.timeSeconds
        || !namedRecordStampPrecedes(previous, current)) {
      throw new RangeError("Named Analysis samples must be strictly chronological");
    }
    const contiguous = current.fraction === 0 ? previous.fraction === 0 && current.tickIndex === previous.tickIndex + 1
      : previous.fraction === 0 && current.tickIndex === previous.tickIndex && index === samples.length - 1;
    if (!contiguous) throw new RangeError("Named Analysis must retain every saved integer tick and its optional terminal fraction");
  }
  return Object.freeze(samples);
}

export function namedRecordStampPrecedes(previous: NamedRecordSample, current: NamedRecordSample): boolean {
  const previousTick = previous.tickIndex + (previous.fraction === 1 ? 1 : 0);
  const currentTick = current.tickIndex + (current.fraction === 1 ? 1 : 0);
  const previousFraction = previous.fraction === 1 ? 0 : previous.fraction;
  const currentFraction = current.fraction === 1 ? 0 : current.fraction;
  return currentTick > previousTick || (currentTick === previousTick && currentFraction > previousFraction);
}

export function parseNamedReplayClock(values: ArrayLike<number>, physicsHz: number, context: NamedReplayContext): NamedReplayClock {
  return parseNamedPlaybackClock(values, physicsHz, context);
}

export function parseNamedPlaybackClock(values: ArrayLike<number>, physicsHz: number, context: NamedPlaybackContext): NamedPlaybackClock {
  if (values.length !== 3) throw new RangeError("Named playback clock requires exactly three values");
  const duration = terminalTime(physicsHz, context);
  const timeSeconds = boundaryNumber(values[0], 0, duration);
  const rateCode = boundaryInteger(values[1], 0, 2) as 0 | 1 | 2;
  const playing = boundaryInteger(values[2], 0, 1);
  if (timeSeconds === duration && playing === 1) throw new RangeError("Terminal playback cursor must be paused");
  return Object.freeze({ timeSeconds, rateCode, kind: playing === 0 ? "paused" : "playing" });
}

export function decodeNamedRecordSample(value: unknown, physicsHz: number, context: RecordQueryContext): NamedRecordSample {
  const sample = ownBoundaryValue(value);
  if (!validateRecordedPlaybackSample(sample)) throw new RangeError("Invalid named record sample structure");
  return projectRecordSample(sample, physicsHz, context);
}

function projectRecordSample(sample: RecordedPlaybackSample, physicsHz: number, context: RecordQueryContext): NamedRecordSample {
  const tickIndex = boundaryInteger(sample.tick_index);
  const fraction = boundaryNumber(sample.fraction, 0, 1);
  const timeSeconds = boundaryNumber(sample.flight_time_s, 0, terminalTime(physicsHz, context));
  if (Math.abs(timeSeconds - (tickIndex + fraction) / physicsHz) > 1e-10) throw new RangeError("Record query time disagrees with its exact stamp");
  const controls = decodePhysicalControls(sample.controls);
  return Object.freeze({ schemaVersion: 2, tickIndex, fraction, timeSeconds, state: decodeSavedState(sample.state), controls });
}

function decodePhysicalControls(controls: RecordedPlaybackSample["controls"]): RecordedPhysicalControls {
  return Object.freeze({ layout: controls.layout, physicalIncidence: decodeTailPhysicalIncidence(controls.physical_incidence) });
}

function decodeSavedState(state: RecordedPlaybackSample["state"]): NamedRecordState {
  const attitude = Object.freeze([...state.attitude_body_to_ned]) as NamedRecordState["attitudeBodyToNed"];
  if (Math.abs(Math.hypot(...attitude) - 1) > 1e-8) throw new RangeError("Saved attitude must be a unit quaternion");
  const telemetry = state.telemetry;
  const angle = (value: unknown) => value === null ? null : boundaryNumber(value);
  return Object.freeze({ datumPositionNedMeters: vector3(state.datum_position_ned_m), datumVelocityNedMetersPerSecond: vector3(state.datum_velocity_ned_mps),
    attitudeBodyToNed: attitude, angularVelocityBodyRadiansPerSecond: vector3(state.angular_velocity_body_rad_s),
    pilotPositionMeters: boundaryNumber(state.pilot_position_m), pilotVelocityMetersPerSecond: boundaryNumber(state.pilot_velocity_mps),
    windAtCgNedMetersPerSecond: vector3(state.wind_at_cg_ned_mps), telemetry: Object.freeze({
      compositeCgPositionNedMeters: vector3(telemetry.composite_cg_position_ned_m), altitudeMeters: boundaryNumber(telemetry.altitude_m),
      airspeedMetersPerSecond: boundaryNumber(telemetry.airspeed_mps, 0), groundspeedMetersPerSecond: boundaryNumber(telemetry.groundspeed_mps, 0),
      angleOfAttackRadians: angle(telemetry.angle_of_attack_rad), sideslipAngleRadians: angle(telemetry.sideslip_angle_rad),
      attitudeEulerRadians: vector3(telemetry.attitude_euler_rad)
    }) });
}

function terminalTime(physicsHz: number, context: RecordQueryContext): number {
  boundaryInteger(physicsHz, 1);
  return boundaryNumber((context.finalization.terminalTick + context.finalization.terminalFraction) / physicsHz, 0);
}

function vector3(value: Vector3): Vector3 {
  return Object.freeze([...value]);
}

function boundedJson(json: string, maximum: number): unknown {
  if (json.length > maximum) throw new RangeError("Named record boundary exceeds its bounded envelope");
  const parsed: unknown = JSON.parse(json);
  return parsed;
}
