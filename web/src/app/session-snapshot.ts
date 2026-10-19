import type { TailSnapshotDocument } from "../generated/tail-snapshot.js";
import type { RecordedPlaybackSample } from "../generated/record-sample.js";
import type { PlaybackContext } from "../generated/record-context.js";
import type { SessionLifecycleDocument } from "../generated/session-lifecycle.js";
import type { DisplayAvailability, FlightRenderPose } from "../render/contracts/runtime.js";

export type SessionLifecycleProjection = SessionLifecycleDocument;
export type SessionPhase = SessionLifecycleDocument["phase"];
export type GamePhase = SessionPhase | "boot";
export type ReplayReturnTarget = "title" | "result";
export type PlaybackSessionSnapshot = RecordedPlaybackSample & Readonly<{ context: PlaybackContext }>;
export type FlightSnapshotInput = TailSnapshotDocument | PlaybackSessionSnapshot;
export type LiveSessionSnapshot<Phase extends "flight_running" | "flight_paused" = "flight_running" | "flight_paused"> =
  TailSnapshotDocument & Readonly<{ phase: Phase; render_pose: FlightRenderPose; frame: Extract<TailSnapshotDocument["frame"], { kind: "flight" }> }>;
type TerminalSessionSnapshot = TailSnapshotDocument & Readonly<{ phase: "result"; render_pose: FlightRenderPose; frame: Extract<TailSnapshotDocument["frame"], { kind: "result" }> }>;
type RecordDisplay<Value> = DisplayAvailability<Value, "record_not_loaded">;
export type TailGameSessionProjection = SessionLifecycleDocument & (
  | Readonly<{ phase: Exclude<SessionPhase, "flight_running" | "flight_paused" | "result" | "replay" | "attract">;
      display: DisplayAvailability<never, "menu_phase"> }>
  | Readonly<{ phase: "flight_running"; display: Readonly<{ kind: "available"; value: LiveSessionSnapshot<"flight_running"> }> }>
  | Readonly<{ phase: "flight_paused"; display: Readonly<{ kind: "available"; value: LiveSessionSnapshot<"flight_paused"> }> }>
  | Readonly<{ phase: "result"; display: RecordDisplay<TerminalSessionSnapshot> }>
  | Readonly<{ phase: "replay"; returnTarget: ReplayReturnTarget; display: RecordDisplay<PlaybackSessionSnapshot> }>
  | Readonly<{ phase: "attract"; display: RecordDisplay<PlaybackSessionSnapshot> }>
);
export type GameSessionProjection = TailGameSessionProjection;

export function isRetainedLiveSnapshot(snapshot: FlightSnapshotInput): snapshot is LiveSessionSnapshot {
  return "frame" in snapshot && (snapshot.phase === "flight_running" || snapshot.phase === "flight_paused") && snapshot.frame.kind === "flight";
}

export function projectTailGameSession(snapshot: TailSnapshotDocument, lifecycle: SessionLifecycleDocument): TailGameSessionProjection {
  const display = snapshot.frame.kind === "menu" ? { kind: "unavailable" as const, reason: "menu_phase" as const }
    : { kind: "available" as const, value: snapshot };
  return { ...lifecycle, display } as TailGameSessionProjection;
}
