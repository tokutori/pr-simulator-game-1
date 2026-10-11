import { projectTailFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { DisplayAvailability, FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import type { TailSessionSnapshot } from "../game/tail-session-codec.js";
import type { ConfigurationMetadataUiState, DifficultyUiState, GameSessionProjection, TailGameSessionProjection } from "./app-state.js";
import type { SessionLifecycleProjection } from "./session-facade.js";

export type SessionControlLayout = "tail_incidence";
export type FlightSnapshotInput = FlightDisplaySnapshot;
export type LiveSessionSnapshot<Phase extends 5 | 6 = 5 | 6> =
  Extract<FlightDisplaySnapshot, { kind: "tail_flight" }> & Readonly<{ phaseCode: Phase }>;
export type LiveSessionProjection<Phase extends 5 | 6> =
  Readonly<{ controlLayout: "tail_incidence"; snapshot: LiveSessionSnapshot<Phase> }>;

declare const terminalRecordStamp: unique symbol;
type TerminalRecordSnapshot = Extract<FlightDisplaySnapshot, { kind: "tail_record" }>
  & Readonly<{ [terminalRecordStamp]: true }>;
export type TerminalSessionSnapshot = Extract<FlightDisplaySnapshot, { kind: "tail_result" }> | TerminalRecordSnapshot;
export type PlaybackSessionSnapshot = Extract<FlightDisplaySnapshot, { kind: "tail_record" }>;
export type RecordDisplay<Value> = DisplayAvailability<Value, "record_not_loaded">;
export type TerminalSessionProjection =
  Readonly<{ controlLayout: "tail_incidence"; display: RecordDisplay<TerminalSessionSnapshot> }>;
export type AttractSessionProjection =
  Readonly<{ controlLayout: "tail_incidence"; display: RecordDisplay<PlaybackSessionSnapshot> }>;

export function isLiveSessionSnapshot<Phase extends 5 | 6>(snapshot: FlightDisplaySnapshot, phaseCode: Phase): snapshot is LiveSessionSnapshot<Phase> {
  return snapshot.kind === "tail_flight" && snapshot.phaseCode === phaseCode;
}

export function isRetainedLiveSnapshot(snapshot: FlightDisplaySnapshot): snapshot is LiveSessionSnapshot {
  return isLiveSessionSnapshot(snapshot, 5) || isLiveSessionSnapshot(snapshot, 6);
}

export function liveSessionProjection<Phase extends 5 | 6>(snapshot: FlightDisplaySnapshot, phaseCode: Phase): LiveSessionProjection<Phase> | null {
  return isLiveSessionSnapshot(snapshot, phaseCode) ? { controlLayout: "tail_incidence", snapshot } : null;
}

export function isTerminalSessionSnapshot(snapshot: FlightDisplaySnapshot): snapshot is TerminalSessionSnapshot {
  return (snapshot.kind === "tail_result" || snapshot.kind === "tail_record")
    && snapshot.stamp.tick === snapshot.finalization.terminalTick
    && snapshot.stamp.fraction === snapshot.finalization.terminalFraction;
}

export function terminalSessionProjection(snapshot: FlightDisplaySnapshot | null): TerminalSessionProjection | null {
  if (snapshot === null) return { controlLayout: "tail_incidence", display: Object.freeze({ kind: "unavailable", reason: "record_not_loaded" }) };
  return isTerminalSessionSnapshot(snapshot)
    ? { controlLayout: "tail_incidence", display: Object.freeze({ kind: "available", value: snapshot }) } : null;
}

export function isPlaybackSessionSnapshot(snapshot: FlightDisplaySnapshot): snapshot is PlaybackSessionSnapshot {
  return snapshot.kind === "tail_record";
}

export function attractSessionProjection(snapshot: FlightDisplaySnapshot | null): AttractSessionProjection | null {
  if (snapshot === null) return { controlLayout: "tail_incidence", display: Object.freeze({ kind: "unavailable", reason: "record_not_loaded" }) };
  return isPlaybackSessionSnapshot(snapshot)
    ? { controlLayout: "tail_incidence", display: Object.freeze({ kind: "available", value: snapshot }) } : null;
}

export function projectionSnapshot(projection: GameSessionProjection): FlightDisplaySnapshot | null {
  return projection.display.kind === "available" ? projection.display.value : null;
}

export function projectTailGameSession(snapshot: TailSessionSnapshot, lifecycle: SessionLifecycleProjection,
  difficulty: DifficultyUiState, configurationMetadata: ConfigurationMetadataUiState | null): TailGameSessionProjection {
  if (snapshot.phaseCode !== lifecycle.phaseCode || snapshot.controlModeCode !== lifecycle.controlModeCode) {
    throw new RangeError("Rust session lifecycle and snapshot disagree");
  }
  const common = { controlLayout: "tail_incidence" as const, controlModeCode: lifecycle.controlModeCode,
    difficulty, configurationMetadata, countdownRemaining: lifecycle.countdownRemaining, canResume: lifecycle.canResume };
  const display = projectTailFlightSnapshot(snapshot);
  if (snapshot.phaseCode === 5 || snapshot.phaseCode === 6) {
    if (display.kind !== "available" || display.value.kind !== "tail_flight" || display.value.phaseCode !== snapshot.phaseCode) throw new RangeError("Rust flight projection requires a live tail snapshot");
    return snapshot.phaseCode === 5
      ? Object.freeze({ ...common, phaseCode: 5, display: Object.freeze({ kind: "available", value: Object.freeze({ ...display.value, phaseCode: 5 }) }) })
      : Object.freeze({ ...common, phaseCode: 6, display: Object.freeze({ kind: "available", value: Object.freeze({ ...display.value, phaseCode: 6 }) }) });
  }
  if (snapshot.phaseCode === 7) {
    if (display.kind !== "available" || display.value.kind !== "tail_result") throw new RangeError("Rust Result projection requires a terminal tail snapshot");
    return Object.freeze({ ...common, phaseCode: 7, display: Object.freeze({ kind: "available", value: display.value }) });
  }
  if (display.kind !== "unavailable") throw new RangeError("Rust menu projection cannot retain a flight snapshot");
  return Object.freeze({ ...common, phaseCode: snapshot.phaseCode, display });
}
