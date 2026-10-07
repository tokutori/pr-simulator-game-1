import { boundaryInteger, boundaryNumber, boundaryObject, boundaryTag, boundaryTuple } from "./tail-boundary-values.js";
import { decodeRecordedDistanceScore, decodeTailControlIdentity, decodeTailPhysicalIncidence, decodeTailScenarioIdentity, decodeTailTerminalFinalization } from "./tail-session-codec.js";
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
export type LegacyTerminalFinalization = Omit<TailTerminalFinalization, "failure">;
export type RecordQueryContext =
  | Readonly<{ controlLayout: "legacy_three_axis"; finalization: LegacyTerminalFinalization }>
  | Readonly<{ controlLayout: "tail_incidence"; finalization: TailTerminalFinalization }>;
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
export type NamedReplayContext = PlaybackEnvelope & Readonly<{ phase: "replay" }> & (
  | Readonly<{ controlLayout: "legacy_three_axis"; controlIdentity: null; finalization: LegacyTerminalFinalization }>
  | Readonly<{ controlLayout: "tail_incidence"; controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>
);
export type NamedAttractContext = PlaybackEnvelope & Readonly<{ phase: "attract"; controlLayout: "tail_incidence";
  controlIdentity: TailControlIdentity; finalization: TailTerminalFinalization }>;
export type NamedPlaybackContext = NamedReplayContext | NamedAttractContext;
export interface NamedPlaybackClock {
  readonly timeSeconds: number;
  readonly rateCode: 0 | 1 | 2;
  readonly kind: "paused" | "playing";
}
export type NamedReplayClock = NamedPlaybackClock;

export function tailResultRecordContext(snapshot: TailSessionSnapshot): RecordQueryContext {
  if (snapshot.frame.kind !== "result") throw new RangeError("Recorded Result queries require a terminal Rust snapshot");
  return Object.freeze({ controlLayout: "tail_incidence", finalization: snapshot.frame.finalization });
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
  const document = boundaryObject(boundedJson(json, 16_384), ["schema_version", "phase", "scenario", "control_layout", "control_identity", "difficulty", "finalization"]);
  if (document.schema_version !== 2) throw new RangeError("Unsupported named playback context schema");
  const phase = boundaryTag(document.phase, ["replay", "attract"]);
  const layout = boundaryTag(document.control_layout, ["legacy_three_axis", "tail_incidence"]);
  const finalization = boundaryObject(document.finalization, ["layout", "value"]);
  if (finalization.layout !== layout) throw new RangeError("Playback finalization and physical control layout disagree");
  const envelope: PlaybackEnvelope = { schemaVersion: 2, scenario: decodeTailScenarioIdentity(document.scenario),
    difficulty: decodeRecordedDifficulty(document.difficulty) };
  if (layout === "legacy_three_axis") {
    if (phase === "attract") throw new RangeError("Named Attract requires the Rust two-tail demo layout");
    if (document.control_identity !== null) throw new RangeError("Legacy Replay cannot declare a two-tail controller identity");
    return Object.freeze({ ...envelope, phase, controlLayout: layout, controlIdentity: null, finalization: decodeLegacyFinalization(finalization.value) });
  }
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
    if (previous === undefined || current === undefined || current.timeSeconds <= previous.timeSeconds) {
      throw new RangeError("Named Analysis samples must be strictly chronological");
    }
    const contiguous = current.fraction === 0 ? previous.fraction === 0 && current.tickIndex === previous.tickIndex + 1
      : previous.fraction === 0 && current.tickIndex === previous.tickIndex && index === samples.length - 1;
    if (!contiguous) throw new RangeError("Named Analysis must retain every saved integer tick and its optional terminal fraction");
  }
  return Object.freeze(samples);
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
  if (controls.layout !== context.controlLayout) throw new RangeError("Record query controls belong to another layout");
  return Object.freeze({ schemaVersion: 2, tickIndex, fraction, timeSeconds, state: decodeSavedState(sample.state), controls });
}

function decodePhysicalControls(value: unknown): RecordedPhysicalControls {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new RangeError("Record query requires physical controls");
  const layout = boundaryTag((value as Record<string, unknown>).layout, ["legacy_three_axis", "tail_incidence"]);
  if (layout === "legacy_three_axis") {
    const controls = boundaryObject(value, ["layout", "roll_rad", "pitch_rad", "yaw_rad"]);
    return Object.freeze({ layout, rollRadians: boundaryNumber(controls.roll_rad), pitchRadians: boundaryNumber(controls.pitch_rad), yawRadians: boundaryNumber(controls.yaw_rad) });
  }
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

function decodeLegacyFinalization(value: unknown): LegacyTerminalFinalization {
  const terminal = boundaryObject(value, ["reason", "disposition", "terminal_tick", "terminal_fraction", "score_m"]);
  const reason = boundaryTag(terminal.reason, ["water_contact", "time_limit", "manual_abort", "out_of_valid_envelope", "fatal_simulation_error"]);
  const disposition = boundaryTag(terminal.disposition, ["complete", "interrupted", "failed"]);
  const expected = reason === "water_contact" || reason === "time_limit" ? "complete" : reason === "manual_abort" ? "interrupted" : "failed";
  if (disposition !== expected) throw new RangeError("Legacy finalization reason and disposition disagree");
  return Object.freeze({ reason, disposition, terminalTick: boundaryInteger(terminal.terminal_tick), terminalFraction: boundaryNumber(terminal.terminal_fraction, 0, 1),
    scoreMeters: terminal.score_m === null ? null : decodeRecordedDistanceScore(terminal.score_m) });
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
