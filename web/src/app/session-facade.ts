import type { GameSessionOperation } from "./app-state.js";
import { executeGameSessionOperation } from "./game-session-operation.js";
import type { GameSessionOperationPort } from "./game-session-operation.js";
import type { FlightSessionPort } from "../game/flight-controller.js";
import { projectRecordedFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import { loadFlightAnalysis, queryFlightRecordRenderPoseAt, queryFlightRecordSampleAt } from "../game/flight-record-query.js";
import type { FlightAnalysisData, FlightAnalysisSample, FlightRecordQueryPort } from "../game/flight-record-query.js";
import { parseFlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { parseNamedAnalysisSamples, parseNamedPlaybackClock, parseNamedPlaybackContext, parseNamedRecordSample, tailResultRecordContext } from "../game/named-record-query.js";
import type { NamedAttractContext, NamedPlaybackClock, NamedPlaybackContext, NamedRecordSample, NamedReplayClock, NamedReplayContext, RecordQueryContext } from "../game/named-record-query.js";
import type { TailSessionPort } from "../game/tail-flight-controller.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";
import { parseTailSessionSnapshot } from "../game/tail-session-codec.js";
import type { TailSessionSnapshot } from "../game/tail-session-codec.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";

const queryOwner = Symbol("session query owner");
const queryGeneration = Symbol("session query generation");
export interface SessionQueryToken {
  readonly [queryOwner]: symbol;
  readonly [queryGeneration]: symbol;
}
export type SessionQueryAcceptance<Value> = Readonly<{ kind: "accepted"; value: Value }> | Readonly<{ kind: "stale" }>;
export interface SessionResourcePort {
  phase_code(): number;
  control_mode_code(): number;
  countdown_remaining(): number;
  can_resume(): boolean;
  advance_countdown(): number;
  clear_pause_reason(reason: number): void;
  environment_snapshot_json(): string;
  export_flight_record_json(): string;
  open_archived_flight_record(json: string): void;
  free(): void;
}
const ownedSessionResources = new WeakSet<SessionResourcePort>();
export type LegacyAppSessionPort = SessionResourcePort & GameSessionOperationPort & FlightSessionPort & FlightRecordQueryPort & {
  launch(): ArrayLike<number>;
};
type PendingTailOperation = "cycle-difficulty-preset" | "cycle-information-level"
  | "cycle-assistance-level" | "cycle-weather-class";
export type TailAppSessionOperation = Exclude<GameSessionOperation, PendingTailOperation>;
export type TailAppSessionPort = SessionResourcePort & TailSessionPort
  & Omit<GameSessionOperationPort, "abort" | "cycle_difficulty_preset"
    | "cycle_information_level" | "cycle_assistance_level" | "cycle_weather_class"> & {
    abort(): string;
    launch(): string;
    playback_context_json(): string;
    flight_analysis_samples_json(): string;
    flight_record_sample_at_seconds(seconds: number): string;
    playback_clock_state(): ArrayLike<number>;
    seek_playback(seconds: number): ArrayLike<number>;
    set_playback_rate_code(code: number): ArrayLike<number>;
    set_playback_playing(playing: boolean): ArrayLike<number>;
    advance_playback(elapsedSeconds: number): ArrayLike<number>;
  };
type CompletedOperation = Readonly<{ kind: "completed" }> | Readonly<{ kind: "countdown-started" }>;
export type LegacyAppOperationResult = CompletedOperation | Readonly<{ kind: "aborted"; terminalSnapshot: FlightSnapshot }>;
export type TailAppOperationResult = CompletedOperation
  | Readonly<{ kind: "aborted"; terminalSnapshot: Extract<TailSessionSnapshot, { phaseCode: 7 }> }>;
export interface SessionLifecycleProjection {
  readonly phaseCode: number;
  readonly controlModeCode: 0 | 1 | 2;
  readonly countdownRemaining: number;
  readonly canResume: boolean;
}
export type TailTitleProjection = Readonly<{ kind: "idle"; phaseCode: 0 }>
  | Readonly<{ kind: "attract"; phaseCode: 10; context: NamedAttractContext; clock: NamedPlaybackClock }>;

abstract class SessionResourceOwner {
  private readonly owner = Symbol("session owner");
  private generation = Symbol("session generation");
  private resourceState: "alive" | "disposed" = "alive";

  protected constructor(private readonly resource: SessionResourcePort, readonly physicsHz: number) {
    if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0) throw new RangeError("Session physics frequency must be a positive integer");
    if (ownedSessionResources.has(resource)) throw new Error("Session resource already has an application owner");
    ownedSessionResources.add(resource);
  }

  captureQueryToken(): SessionQueryToken {
    return this.observe(() => Object.freeze({ [queryOwner]: this.owner, [queryGeneration]: this.generation }));
  }

  acceptQuery<Value>(token: SessionQueryToken, value: Value): SessionQueryAcceptance<Value> {
    return this.resourceState === "alive" && token[queryOwner] === this.owner && token[queryGeneration] === this.generation
      ? Object.freeze({ kind: "accepted", value }) : Object.freeze({ kind: "stale" });
  }

  readLifecycle(): SessionLifecycleProjection {
    return this.observe(() => {
      const canResume = this.resource.can_resume();
      if (typeof canResume !== "boolean") throw new RangeError("Rust resume projection requires a boolean");
      return Object.freeze({ phaseCode: boundaryInteger(this.resource.phase_code(), 0, 10),
        controlModeCode: boundaryInteger(this.resource.control_mode_code(), 0, 2) as 0 | 1 | 2,
        countdownRemaining: boundaryInteger(this.resource.countdown_remaining()), canResume });
    });
  }

  advanceCountdown(): number {
    return this.change(() => boundaryInteger(this.resource.advance_countdown()));
  }

  clearPauseReason(reason: number): void {
    this.change(() => { this.resource.clear_pause_reason(reason); });
  }

  readEnvironmentJson(): string {
    return this.observe(() => this.resource.environment_snapshot_json());
  }

  exportRecordJson(): string {
    return this.observe(() => this.resource.export_flight_record_json());
  }

  openArchive(json: string): void {
    this.change(() => { this.resource.open_archived_flight_record(json); });
  }

  dispose(): void {
    if (this.resourceState === "disposed") return;
    this.resourceState = "disposed";
    this.generation = Symbol("disposed session generation");
    this.resource.free();
  }

  protected observe<Value>(read: () => Value): Value {
    if (this.resourceState !== "alive") throw new Error("Session resource has been disposed");
    return read();
  }

  protected change<Value>(operation: () => Value): Value {
    return this.observe(() => {
      this.generation = Symbol("session generation");
      return operation();
    });
  }
}

export class LegacyAppSessionFacade extends SessionResourceOwner {
  readonly controlLayout = "legacy_three_axis";
  readonly flightPort: FlightSessionPort;

  constructor(private readonly port: LegacyAppSessionPort, physicsHz: number) {
    super(port, physicsHz);
    this.flightPort = Object.freeze({ snapshot: () => this.observe(() => port.snapshot()),
      advance_tick: (roll: number, pitch: number, yaw: number, pilot: number) => this.change(() => port.advance_tick(roll, pitch, yaw, pilot)),
      free: () => { this.dispose(); } });
  }

  executeOperation(operation: GameSessionOperation): LegacyAppOperationResult {
    return this.change(() => {
      const result = executeGameSessionOperation(this.port, operation);
      return result.kind === "aborted" ? Object.freeze({ kind: "aborted", terminalSnapshot: parseFlightSnapshot(result.terminalSnapshot) })
        : Object.freeze(result);
    });
  }

  readSnapshot(): FlightSnapshot {
    return this.observe(() => parseFlightSnapshot(this.port.snapshot()));
  }

  launch(): FlightSnapshot {
    return this.change(() => parseFlightSnapshot(this.port.launch()));
  }

  readAnalysis(scenarioId: number | null = null): FlightAnalysisData {
    return this.observe(() => loadFlightAnalysis(this.port, this.physicsHz, scenarioId));
  }

  queryRecordSample(seconds: number): FlightAnalysisSample {
    return this.observe(() => queryFlightRecordSampleAt(this.port, this.physicsHz, seconds));
  }

  queryRecordRenderPose(seconds: number, initialPilotPositionMeters: number): FlightRenderPose {
    return this.observe(() => queryFlightRecordRenderPoseAt(this.port, this.physicsHz, seconds, initialPilotPositionMeters));
  }
}

export class TailAppSessionFacade extends SessionResourceOwner {
  readonly controlLayout = "tail_incidence";
  readonly flightPort: TailSessionPort;

  constructor(private readonly port: TailAppSessionPort, physicsHz: number) {
    super(port, physicsHz);
    this.flightPort = Object.freeze({ snapshot_json: () => this.observe(() => port.snapshot_json()),
      control_profile_json: () => this.observe(() => port.control_profile_json()),
      advance_tick_json: (json: string) => this.change(() => port.advance_tick_json(json)), free: () => { this.dispose(); } });
  }

  executeOperation(operation: TailAppSessionOperation): TailAppOperationResult {
    return this.change(() => executeTailOperation(this.port, operation, this.physicsHz));
  }

  readSnapshot(): TailSessionSnapshot {
    return this.observe(() => parseTailSessionSnapshot(this.port.snapshot_json(), this.physicsHz));
  }

  launch(): TailSessionSnapshot {
    return this.change(() => parseTailSessionSnapshot(this.port.launch(), this.physicsHz));
  }

  readReplayContext(): NamedReplayContext {
    const context = this.readPlaybackContext();
    if (context.phase !== "replay") throw new RangeError("Replay projection requires the Rust Replay phase");
    return context;
  }

  readAttractContext(): NamedAttractContext {
    const context = this.readPlaybackContext();
    if (context.phase !== "attract") throw new RangeError("Attract projection requires the Rust Attract phase");
    return context;
  }

  readPlaybackContext(): NamedPlaybackContext {
    return this.observe(() => {
      const phaseCode = this.readLifecycle().phaseCode;
      const context = parseNamedPlaybackContext(this.port.playback_context_json());
      if (phaseCode !== (context.phase === "replay" ? 9 : 10)) throw new RangeError("Named playback context disagrees with the Rust phase");
      return context;
    });
  }

  readTitleProjection(): TailTitleProjection {
    return this.observe(() => {
      const phaseCode = this.readLifecycle().phaseCode;
      if (phaseCode === 0) return Object.freeze({ kind: "idle", phaseCode });
      const context = this.readAttractContext();
      return Object.freeze({ kind: "attract", phaseCode: 10, context,
        clock: parseNamedPlaybackClock(this.port.playback_clock_state(), this.physicsHz, context) });
    });
  }

  readAnalysisSamples(): readonly NamedRecordSample[] {
    return this.observe(() => parseNamedAnalysisSamples(this.port.flight_analysis_samples_json(), this.physicsHz, this.recordContext()));
  }

  queryRecordSample(seconds: number): NamedRecordSample {
    return this.observe(() => parseNamedRecordSample(this.port.flight_record_sample_at_seconds(seconds), this.physicsHz, this.recordContext()));
  }

  queryRecordDisplay(seconds: number): FlightDisplaySnapshot {
    return this.observe(() => {
      const context = this.recordContext();
      const sample = parseNamedRecordSample(this.port.flight_record_sample_at_seconds(seconds), this.physicsHz, context);
      return projectRecordedFlightSnapshot(sample, context);
    });
  }

  readReplayClock(): NamedReplayClock {
    return this.observe(() => parseNamedPlaybackClock(this.port.playback_clock_state(), this.physicsHz, this.readReplayContext()));
  }

  seekReplay(seconds: number): NamedReplayClock {
    return this.change(() => {
      const context = this.readReplayContext();
      return parseNamedPlaybackClock(this.port.seek_playback(seconds), this.physicsHz, context);
    });
  }

  readPlaybackClock(): NamedPlaybackClock {
    return this.observe(() => parseNamedPlaybackClock(this.port.playback_clock_state(), this.physicsHz, this.readPlaybackContext()));
  }

  seekPlayback(seconds: number): NamedPlaybackClock {
    return this.changePlaybackClock(() => this.port.seek_playback(seconds));
  }

  setPlaybackRate(code: 0 | 1 | 2): NamedPlaybackClock {
    return this.changePlaybackClock(() => this.port.set_playback_rate_code(code));
  }

  setPlaybackPlaying(playing: boolean): NamedPlaybackClock {
    return this.changePlaybackClock(() => this.port.set_playback_playing(playing));
  }

  advancePlayback(elapsedSeconds: number): NamedPlaybackClock {
    return this.changePlaybackClock(() => this.port.advance_playback(elapsedSeconds));
  }

  private changePlaybackClock(operation: () => ArrayLike<number>): NamedPlaybackClock {
    return this.change(() => {
      const context = this.readPlaybackContext();
      return parseNamedPlaybackClock(operation(), this.physicsHz, context);
    });
  }

  private recordContext(): RecordQueryContext {
    const phase = this.readLifecycle().phaseCode;
    return phase === 9 || phase === 10 ? this.readPlaybackContext() : tailResultRecordContext(this.readSnapshot());
  }
}

export type AppSessionFacade = LegacyAppSessionFacade | TailAppSessionFacade;

function executeTailOperation(port: TailAppSessionPort, operation: TailAppSessionOperation, physicsHz: number): TailAppOperationResult {
  if (typeof operation !== "string") {
    if (operation.kind === "set-information-cue") port.set_information_cue(operation.cueCode, operation.visible);
    else {
      switch (operation.axis) {
        case "preset": port.set_difficulty_preset(operation.code); break;
        case "information": port.set_information_level(operation.code); break;
        case "assistance": port.set_assistance_level(operation.code); break;
        case "weather": port.set_weather_class(operation.code); break;
      }
    }
    return Object.freeze({ kind: "completed" });
  }
  switch (operation) {
    case "open-setup": port.open_setup(); break;
    case "set-control-manual": port.set_control_mode(0); break;
    case "set-control-shared": port.set_control_mode(1); break;
    case "set-control-automatic": port.set_control_mode(2); break;
    case "return-to-title": port.return_to_title(); break;
    case "prepare": port.prepare(); port.mark_briefing_ready(); break;
    case "cancel-briefing": port.cancel_briefing(); break;
    case "start-flight": port.start_countdown(3); return Object.freeze({ kind: "countdown-started" });
    case "cancel-countdown": port.cancel_countdown(); break;
    case "pause": port.pause(0); break;
    case "resume": port.resume(); break;
    case "abort": {
      const snapshot = parseTailSessionSnapshot(port.abort(), physicsHz);
      if (snapshot.phaseCode !== 7) throw new RangeError("Tail abort must return a Rust Result snapshot");
      return Object.freeze({ kind: "aborted", terminalSnapshot: snapshot });
    }
    case "retry": port.retry(); break;
    case "retry-briefing": port.retry_briefing(); port.mark_briefing_ready(); break;
    case "enter-replay": port.enter_replay(); break;
    case "leave-replay": port.leave_replay(); break;
    case "enter-attract": port.enter_attract(); break;
    case "leave-attract": port.leave_attract(); break;
    default: return unknownOperation(operation);
  }
  return Object.freeze({ kind: "completed" });
}

function unknownOperation(operation: never): never {
  throw new RangeError(`Unsupported tail session operation: ${String(operation)}`);
}
