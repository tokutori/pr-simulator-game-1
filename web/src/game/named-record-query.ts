import { boundaryInteger, boundaryNumber, boundaryObject, boundaryTag, boundaryTuple } from "./tail-boundary-values.js";
import { decodeTailControlIdentity, decodeTailPhysicalIncidence, decodeTailScenarioIdentity, decodeTailTerminalFinalization } from "./tail-session-codec.js";
import type { TailControlIdentity, TailFlightTelemetry, TailScenarioIdentity, TailSessionSnapshot, TailTerminalFinalization } from "./tail-session-codec.js";
import type { PhysicalFlightControls } from "../render/contracts/flight-controls.js";

type Vector3 = readonly [number, number, number];
export type RecordedPhysicalControls = PhysicalFlightControls;
export interface NamedRecordState {
  readonly datumPositionNedMeters: Vector3;
  readonly datumVelocityNedMetersPerSecond: Vector3;
  readonly attitudeBodyToNed: readonly [number, number, number, number];
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
export type RecordQueryContext = Readonly<{ controlLayout: "tail_incidence"; scenario: TailScenarioIdentity;
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
interface RecordedDifficultyAxes {
  readonly preset: "beginner" | "standard" | "expert" | "realistic" | "custom";
  readonly assistance: "strong" | "assisted" | "light" | "manual";
  readonly weather: "calm" | "mild" | "typical" | "challenging" | "near_limit";
}
export interface RecordedHudProfile {
  readonly telemetry: boolean;
  readonly attitude: boolean;
  readonly wind: boolean;
  readonly flightPath: boolean;
  readonly angleOfAttack: boolean;
  readonly warnings: boolean;
}
export type RecordedDifficulty = RecordedDifficultyAxes & (
  | Readonly<{ information: "full" | "standard" | "minimal" | "realistic"; hudProfile: null }>
  | Readonly<{ information: "custom"; hudProfile: RecordedHudProfile }>
);
interface PlaybackEnvelope {
  readonly schemaVersion: 2;
  readonly scenario: TailScenarioIdentity;
  readonly difficulty: RecordedDifficulty;
}
export type NamedReplayContext = PlaybackEnvelope & Readonly<{ phase: "replay"; controlLayout: "tail_incidence";
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedAttractContext = PlaybackEnvelope & Readonly<{ phase: "attract"; controlLayout: "tail_incidence";
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedPlaybackContext = NamedReplayContext | NamedAttractContext;
export type NamedResultContext = PlaybackEnvelope & Readonly<{ phase: "result"; controlLayout: "tail_incidence";
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
  const document = boundaryObject(value, ["schema_version", "phase", "scenario", "control_layout", "control_identity", "difficulty", "finalization"]);
  if (document.schema_version !== 2) throw new RangeError("Unsupported named playback context schema");
  const phase = boundaryTag(document.phase, ["result", "replay", "attract"]);
  const layout = boundaryTag(document.control_layout, ["tail_incidence"]);
  const finalization = boundaryObject(document.finalization, ["layout", "value"]);
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
  const document = boundaryObject(boundedJson(json, 16 * 1024 * 1024), ["schema_version", "samples"]);
  if (document.schema_version !== 2 || !Array.isArray(document.samples) || document.samples.length === 0) {
    throw new RangeError("Named Analysis requires a nonempty schema-two sample batch");
  }
  const samples = document.samples.map((sample: unknown) => decodeNamedRecordSample(sample, physicsHz, context));
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

function decodeNamedRecordSample(value: unknown, physicsHz: number, context: RecordQueryContext): NamedRecordSample {
  const sample = boundaryObject(value, ["schema_version", "tick_index", "fraction", "flight_time_s", "state", "controls"]);
  if (sample.schema_version !== 2) throw new RangeError("Unsupported named record sample schema");
  const tickIndex = boundaryInteger(sample.tick_index);
  const fraction = boundaryNumber(sample.fraction, 0, 1);
  const timeSeconds = boundaryNumber(sample.flight_time_s, 0, terminalTime(physicsHz, context));
  if (Math.abs(timeSeconds - (tickIndex + fraction) / physicsHz) > 1e-10) throw new RangeError("Record query time disagrees with its exact stamp");
  const controls = decodePhysicalControls(sample.controls);
  return Object.freeze({ schemaVersion: 2, tickIndex, fraction, timeSeconds, state: decodeSavedState(sample.state), controls });
}

function decodePhysicalControls(value: unknown): RecordedPhysicalControls {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new RangeError("Record query requires physical controls");
  const layout = boundaryTag((value as Record<string, unknown>).layout, ["tail_incidence"]);

  const controls = boundaryObject(value, ["layout", "physical_incidence"]);
  return Object.freeze({ layout, physicalIncidence: decodeTailPhysicalIncidence(controls.physical_incidence) });
}

function decodeSavedState(value: unknown): NamedRecordState {
  const state = boundaryObject(value, ["datum_position_ned_m", "datum_velocity_ned_mps", "attitude_body_to_ned", "angular_velocity_body_rad_s",
    "pilot_position_m", "pilot_velocity_mps", "wind_at_cg_ned_mps", "telemetry"]);
  const attitude = boundaryTuple(state.attitude_body_to_ned, 4) as readonly [number, number, number, number];
  if (Math.abs(Math.hypot(...attitude) - 1) > 1e-8) throw new RangeError("Saved attitude must be a unit quaternion");
  const telemetry = boundaryObject(state.telemetry, ["composite_cg_position_ned_m", "altitude_m", "airspeed_mps", "groundspeed_mps",
    "angle_of_attack_rad", "sideslip_angle_rad", "attitude_euler_rad"]);
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

function decodeRecordedDifficulty(value: unknown): RecordedDifficulty {
  const hasProfile = typeof value === "object" && value !== null && Object.hasOwn(value, "hud_profile");
  const difficulty = boundaryObject(value, hasProfile ? ["preset", "information", "assistance", "weather", "hud_profile"] : ["preset", "information", "assistance", "weather"]);
  const axes: RecordedDifficultyAxes = { preset: boundaryTag(difficulty.preset, ["beginner", "standard", "expert", "realistic", "custom"]),
    assistance: boundaryTag(difficulty.assistance, ["strong", "assisted", "light", "manual"]),
    weather: boundaryTag(difficulty.weather, ["calm", "mild", "typical", "challenging", "near_limit"]) };
  const information = boundaryTag(difficulty.information, ["full", "standard", "minimal", "realistic", "custom"]);
  if (information !== "custom") {
    if (hasProfile) throw new RangeError("Explicit saved HUD profile requires Custom information");
    return Object.freeze({ ...axes, information, hudProfile: null });
  }
  const profile = boundaryObject(difficulty.hud_profile, ["telemetry", "attitude", "wind", "flight_path", "angle_of_attack", "warnings"]);
  const boolean = (value: unknown) => { if (typeof value !== "boolean") throw new RangeError("Saved HUD cues must be boolean"); return value; };
  return Object.freeze({ ...axes, information, hudProfile: Object.freeze({ telemetry: boolean(profile.telemetry), attitude: boolean(profile.attitude),
    wind: boolean(profile.wind), flightPath: boolean(profile.flight_path), angleOfAttack: boolean(profile.angle_of_attack), warnings: boolean(profile.warnings) }) });
}

function terminalTime(physicsHz: number, context: RecordQueryContext): number {
  boundaryInteger(physicsHz, 1);
  return boundaryNumber((context.finalization.terminalTick + context.finalization.terminalFraction) / physicsHz, 0);
}

function vector3(value: unknown): Vector3 {
  return boundaryTuple(value, 3) as Vector3;
}

function boundedJson(json: string, maximum: number): unknown {
  if (json.length > maximum) throw new RangeError("Named record boundary exceeds its bounded envelope");
  const parsed: unknown = JSON.parse(json);
  return parsed;
}
