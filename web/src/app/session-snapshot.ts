import { projectLegacyFlightSnapshot, projectTailFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { DisplayAvailability, FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { TailSessionSnapshot } from "../game/tail-session-codec.js";
import type { ConfigurationMetadataUiState, DifficultyUiState, GameSessionProjection, TailGameSessionProjection } from "./app-state.js";
import type { SessionLifecycleProjection } from "./session-facade.js";

export type SessionControlLayout = "legacy_three_axis" | "tail_incidence";
export type FlightSnapshotInput = FlightSnapshot | FlightDisplaySnapshot;
type LegacyLiveSnapshot = Extract<FlightDisplaySnapshot, { kind: "legacy_live" }>;
type LegacyTerminalSnapshot = LegacyLiveSnapshot & Readonly<{ terminal: Exclude<FlightSnapshot["terminal"], "airborne"> }>;
export type LiveSessionSnapshot<Phase extends 5 | 6 = 5 | 6> = (LegacyLiveSnapshot & Readonly<{ terminal: "airborne" }>)
  | (Extract<FlightDisplaySnapshot, { kind: "tail_flight" }> & Readonly<{ phaseCode: Phase }>);
export type LiveSessionProjection<Phase extends 5 | 6> =
  | Readonly<{ controlLayout: "legacy_three_axis"; snapshot: Extract<LiveSessionSnapshot<Phase>, { kind: "legacy_live" }> }>
  | Readonly<{ controlLayout: "tail_incidence"; snapshot: Extract<LiveSessionSnapshot<Phase>, { kind: "tail_flight" }> }>;
declare const terminalRecordStamp: unique symbol;
type TerminalRecordSnapshot = Extract<FlightDisplaySnapshot, { kind: "legacy_record" | "tail_record" }>
  & Readonly<{ [terminalRecordStamp]: true }>;
export type TerminalSessionSnapshot = LegacyTerminalSnapshot | Extract<FlightDisplaySnapshot, { kind: "tail_result" }> | TerminalRecordSnapshot;
export type PlaybackSessionSnapshot = LegacyTerminalSnapshot | Extract<FlightDisplaySnapshot, { kind: "legacy_record" | "tail_record" }>;
export type RecordDisplay<Value> = DisplayAvailability<Value, "record_not_loaded">;
export type TerminalSessionProjection =
  | Readonly<{ controlLayout: "legacy_three_axis"; display: RecordDisplay<Extract<TerminalSessionSnapshot, { kind: "legacy_live" | "legacy_record" }>> }>
  | Readonly<{ controlLayout: "tail_incidence"; display: RecordDisplay<Extract<TerminalSessionSnapshot, { kind: "tail_result" | "tail_record" }>> }>;
export type AttractSessionProjection =
  | Readonly<{ controlLayout: "legacy_three_axis"; display: RecordDisplay<Extract<PlaybackSessionSnapshot, { kind: "legacy_live" | "legacy_record" }>> }>
  | Readonly<{ controlLayout: "tail_incidence"; display: RecordDisplay<Extract<PlaybackSessionSnapshot, { kind: "tail_record" }>> }>;

export function normalizeFlightSnapshot(snapshot: FlightSnapshotInput): FlightDisplaySnapshot {
  return "kind" in snapshot ? snapshot : projectLegacyFlightSnapshot(snapshot);
}

export function isLiveSessionSnapshot<Phase extends 5 | 6>(snapshot: FlightDisplaySnapshot, phaseCode: Phase, layout: SessionControlLayout): snapshot is LiveSessionSnapshot<Phase> {
  return snapshot.kind === "legacy_live" ? layout === "legacy_three_axis" && snapshot.terminal === "airborne"
    : snapshot.kind === "tail_flight" && layout === "tail_incidence" && snapshot.phaseCode === phaseCode;
}

export function isRetainedLiveSnapshot(snapshot: FlightDisplaySnapshot, layout: SessionControlLayout): snapshot is LiveSessionSnapshot {
  return isLiveSessionSnapshot(snapshot, 5, layout) || isLiveSessionSnapshot(snapshot, 6, layout);
}

export function liveSessionProjection<Phase extends 5 | 6>(snapshot: FlightDisplaySnapshot, phaseCode: Phase,
  layout: SessionControlLayout): LiveSessionProjection<Phase> | null {
  if (!isLiveSessionSnapshot(snapshot, phaseCode, layout)) return null;
  return snapshot.kind === "legacy_live" ? { controlLayout: "legacy_three_axis", snapshot }
    : { controlLayout: "tail_incidence", snapshot };
}

export function isTerminalSessionSnapshot(snapshot: FlightDisplaySnapshot, layout: SessionControlLayout): snapshot is TerminalSessionSnapshot {
  if (snapshot.kind === "legacy_live") return layout === "legacy_three_axis" && snapshot.terminal !== "airborne";
  if (snapshot.controls.layout !== layout) return false;
  return (snapshot.kind === "tail_result" || snapshot.kind === "legacy_record" || snapshot.kind === "tail_record")
    && snapshot.stamp.tick === snapshot.finalization.terminalTick
    && snapshot.stamp.fraction === snapshot.finalization.terminalFraction;
}

export function terminalSessionProjection(snapshot: FlightDisplaySnapshot | null, layout: SessionControlLayout): TerminalSessionProjection | null {
  if (snapshot === null) {
    const display = Object.freeze({ kind: "unavailable" as const, reason: "record_not_loaded" as const });
    return layout === "legacy_three_axis" ? { controlLayout: layout, display } : { controlLayout: layout, display };
  }
  if (!isTerminalSessionSnapshot(snapshot, layout)) return null;
  return snapshot.kind === "legacy_live" || snapshot.kind === "legacy_record"
    ? { controlLayout: "legacy_three_axis", display: Object.freeze({ kind: "available", value: snapshot }) }
    : { controlLayout: "tail_incidence", display: Object.freeze({ kind: "available", value: snapshot }) };
}

export function isPlaybackSessionSnapshot(snapshot: FlightDisplaySnapshot): snapshot is PlaybackSessionSnapshot {
  return snapshot.kind === "legacy_record" || snapshot.kind === "tail_record"
    || (snapshot.kind === "legacy_live" && snapshot.terminal !== "airborne");
}

export function attractSessionProjection(snapshot: FlightDisplaySnapshot | null, layout: SessionControlLayout): AttractSessionProjection | null {
  if (snapshot === null) {
    const display = Object.freeze({ kind: "unavailable" as const, reason: "record_not_loaded" as const });
    return layout === "legacy_three_axis" ? { controlLayout: layout, display } : { controlLayout: layout, display };
  }
  if (!isPlaybackSessionSnapshot(snapshot)) return null;
  if (layout === "legacy_three_axis" && (snapshot.kind === "legacy_live" || snapshot.kind === "legacy_record")) {
    return { controlLayout: layout, display: Object.freeze({ kind: "available", value: snapshot }) };
  }
  return layout === "tail_incidence" && snapshot.kind === "tail_record"
    ? { controlLayout: layout, display: Object.freeze({ kind: "available", value: snapshot }) } : null;
}

export function projectionSnapshot(projection: GameSessionProjection): FlightSnapshotInput | null {
  return "display" in projection ? projection.display.kind === "available" ? projection.display.value : null : projection.snapshot;
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
