import type { GameSessionOperation } from "./app-state.js";
import type { DifficultyUiState, ReplayReturnTarget, TailGameSessionProjection } from "./app-state.js";
import { decodePreparedUiConfiguration, decodeSessionDifficulty } from "./session-selection.js";
import type { PreparedUiConfiguration, SessionSelectionPort } from "./session-selection.js";
import { projectTailGameSession } from "./session-snapshot.js";
import { executeGameSessionOperation } from "./game-session-operation.js";
import type { GameSessionOperationPort } from "./game-session-operation.js";
import type { FlightSessionPort } from "../game/flight-controller.js";
import { readFlightLog } from "../game/flight-log-export.js";
import type { FlightLogExportPort, FlightLogFormat } from "../game/flight-log-export.js";
import { projectPreparedLaunchRenderPose, projectRecordedFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import type { DisplayAvailability } from "../game/flight-display-snapshot.js";
import { validateAnalysisInput } from "../game/flight-analysis-view.js";
import type { NamedAnalysisCursor, NamedAnalysisDataset } from "../game/flight-analysis-view.js";
import { loadFlightAnalysis, queryFlightRecordRenderPoseAt, queryFlightRecordSampleAt } from "../game/flight-record-query.js";
import type { FlightAnalysisData, FlightAnalysisSample, FlightRecordQueryPort } from "../game/flight-record-query.js";
import { parseFlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { parseNamedAnalysisSamples, parseNamedPlaybackClock, parseNamedPlaybackContext, parseNamedRecordSample, tailResultRecordContext } from "../game/named-record-query.js";
import type { NamedAttractContext, NamedPlaybackClock, NamedPlaybackContext, NamedRecordContext, NamedRecordSample, NamedReplayClock, NamedReplayContext, RecordQueryContext } from "../game/named-record-query.js";
import { parseNamedRecordSummary, parseNamedWindGrid, sameNamedRecordContext } from "../game/named-record-analysis.js";
import type { NamedRecordSummary, NamedWindGrid, NamedWindGridRequest } from "../game/named-record-analysis.js";
import type { TailSessionPort } from "../game/tail-flight-controller.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";
import { parseTailPreparedLaunchSnapshot, parseTailSessionSnapshot } from "../game/tail-session-codec.js";
import type { TailSessionSnapshot } from "../game/tail-session-codec.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";

const queryOwner = Symbol("session query owner");
const queryGeneration = Symbol("session query generation");
const recordSourceGeneration = Symbol("record source generation");
const analysisObservation = Symbol("saved analysis observation");
export interface SessionQueryToken {
  readonly [queryOwner]: symbol;
  readonly [queryGeneration]: symbol;
}
export type SessionQueryAcceptance<Value> = Readonly<{ kind: "accepted"; value: Value }> | Readonly<{ kind: "stale" }>;
interface RecordSourceToken {
  readonly [queryOwner]: symbol;
  readonly [recordSourceGeneration]: symbol;
}
interface AnalysisObservationProof {
  readonly source: RecordSourceToken;
  readonly context: NamedRecordContext;
}
export interface ObservedNamedAnalysisDataset extends NamedAnalysisDataset {
  readonly [analysisObservation]: AnalysisObservationProof;
}
export interface SessionResourcePort extends SessionSelectionPort, FlightLogExportPort {
  phase_code(): number;
  control_mode_code(): number;
  countdown_remaining(): number;
  can_resume(): boolean;
  advance_countdown(): number;
  clear_pause_reason(reason: number): void;
  environment_snapshot_json(): string;
  export_flight_record_json(): string;
  open_archived_flight_record(json: string): void;
  is_archived_replay(): boolean;
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
    prepared_launch_snapshot_json(): string;
    playback_context_json(): string;
    flight_analysis_samples_json(): string;
    flight_record_sample_at_seconds(seconds: number): string;
    playback_clock_state(): ArrayLike<number>;
    seek_playback(seconds: number): ArrayLike<number>;
    set_playback_rate_code(code: number): ArrayLike<number>;
    set_playback_playing(playing: boolean): ArrayLike<number>;
    advance_playback(elapsedSeconds: number): ArrayLike<number>;
    flight_record_summary_json(): string;
    flight_wind_grid_json(northMinimumMeters: number, eastMinimumMeters: number, altitudeMeters: number, spacingMeters: number): string;
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
  private sourceGeneration = Symbol("record source generation");
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

  protected captureRecordSourceToken(): RecordSourceToken {
    return this.observe(() => Object.freeze({ [queryOwner]: this.owner, [recordSourceGeneration]: this.sourceGeneration }));
  }

  protected recordSourceIsCurrent(token: RecordSourceToken): boolean {
    return this.resourceState === "alive" && token[queryOwner] === this.owner && token[recordSourceGeneration] === this.sourceGeneration;
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

  readDifficulty(): DifficultyUiState {
    return this.observe(() => decodeSessionDifficulty(this.resource));
  }

  readPreparedConfiguration(): PreparedUiConfiguration {
    return this.observe(() => decodePreparedUiConfiguration(this.readLifecycle().phaseCode, () => this.resource.configuration_metadata()));
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

  readReplayReturnTarget(): ReplayReturnTarget {
    return this.observe(() => {
      const token = this.captureQueryToken();
      if (this.readLifecycle().phaseCode !== 9) throw new RangeError("Replay return target requires the Rust Replay phase");
      const archived = this.resource.is_archived_replay();
      if (typeof archived !== "boolean") throw new RangeError("Rust archive projection requires a boolean");
      const target = archived ? "title" : "result";
      if (this.acceptQuery(token, target).kind === "stale") throw new RangeError("Replay ownership changed during observation");
      return target;
    });
  }

  exportRecordJson(): string {
    return this.observe(() => this.resource.export_flight_record_json());
  }

  readFlightLog(format: FlightLogFormat): string {
    return this.observe(() => {
      const token = this.captureQueryToken();
      const phase = this.readLifecycle().phaseCode;
      if (phase !== 7 && phase !== 9) throw new RangeError("Flight log export requires the Rust Result or Replay phase");
      const text = readFlightLog(this.resource, format);
      if (this.readLifecycle().phaseCode !== phase || this.acceptQuery(token, text).kind === "stale") {
        throw new Error("Flight log record source changed during observation");
      }
      return text;
    });
  }

  openArchive(json: string): void {
    this.change(() => { this.resource.open_archived_flight_record(json); });
  }

  dispose(): void {
    if (this.resourceState === "disposed") return;
    this.resourceState = "disposed";
    this.generation = Symbol("disposed session generation");
    this.sourceGeneration = Symbol("disposed record source generation");
    this.resource.free();
  }

  protected observe<Value>(read: () => Value): Value {
    if (this.resourceState !== "alive") throw new Error("Session resource has been disposed");
    return read();
  }

  protected change<Value>(operation: () => Value, recordPolicy: "replace_record" | "retain_record" = "replace_record"): Value {
    return this.observe(() => {
      this.generation = Symbol("session generation");
      if (recordPolicy === "replace_record") this.sourceGeneration = Symbol("record source generation");
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

  pauseForReason(reason: "page_hidden" | "presentation_transition"): void {
    this.change(() => { this.port.pause(reason === "page_hidden" ? 1 : 2); });
  }

  readSnapshot(): TailSessionSnapshot {
    return this.observe(() => parseTailSessionSnapshot(this.port.snapshot_json(), this.physicsHz));
  }

  readPreparedLaunchPose(): FlightRenderPose | null {
    return this.observe(() => {
      const snapshot = parseTailPreparedLaunchSnapshot(this.port.prepared_launch_snapshot_json(), this.physicsHz);
      if (snapshot.phaseCode !== this.readLifecycle().phaseCode) throw new RangeError("Prepared launch projection disagrees with the Rust phase");
      return projectPreparedLaunchRenderPose(snapshot);
    });
  }

  readGameSessionProjection(): TailGameSessionProjection {
    return this.observe(() => {
      const lifecycle = this.readLifecycle();
      const configuration = this.readPreparedConfiguration();
      return projectTailGameSession(this.readSnapshot(), lifecycle, this.readDifficulty(),
        configuration.kind === "available" ? configuration.value : null);
    });
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

  readAnalysisDataset(wind: DisplayAvailability<NamedWindGridRequest, "not_requested">): ObservedNamedAnalysisDataset {
    return this.observeRecordQuery(() => {
      const source = this.captureRecordSourceToken();
      const summary = this.readRecordSummary();
      const context = summary.context;
      const samples = parseNamedAnalysisSamples(this.port.flight_analysis_samples_json(), this.physicsHz, context);
      const windGrid: NamedAnalysisDataset["windGrid"] = wind.kind === "unavailable" ? wind
        : Object.freeze({ kind: "available", value: this.queryWindGridWithContext(wind.value, context) });
      const dataset: ObservedNamedAnalysisDataset = Object.freeze({ kind: "named_record", context, summary, samples, windGrid,
        [analysisObservation]: Object.freeze({ source, context }) });
      validateAnalysisInput(dataset);
      this.requireRecordContext(context);
      return dataset;
    });
  }

  queryAnalysisCursor(seconds: number, expectedDataset: NamedAnalysisDataset): NamedAnalysisCursor {
    return this.observeRecordQuery(() => {
      if (!(analysisObservation in expectedDataset) || !isAnalysisObservationProof(expectedDataset[analysisObservation])) {
        throw new RangeError("Analysis cursor requires an observed dataset");
      }
      const proof = expectedDataset[analysisObservation];
      if (!this.recordSourceIsCurrent(proof.source)) {
        throw new RangeError("Analysis dataset owner or generation is stale");
      }
      const expectedContext = expectedDataset.context;
      if (!sameNamedRecordContext(expectedContext, proof.context)) throw new RangeError("Analysis dataset differs from its observed record context");
      this.requireCursorRecordContext(expectedContext);
      const sample = parseNamedRecordSample(this.port.flight_record_sample_at_seconds(seconds), this.physicsHz, expectedContext);
      this.requireCursorRecordContext(expectedContext);
      return Object.freeze({ ...sample, kind: "named_record", context: expectedContext });
    });
  }

  readRecordSummary(): NamedRecordSummary {
    return this.observe(() => {
      const summary = parseNamedRecordSummary(this.port.flight_record_summary_json(), this.physicsHz);
      const phase = this.readLifecycle().phaseCode;
      if (phase === 9 || phase === 10) {
        if (!sameNamedRecordContext(summary.context, this.readPlaybackContext())) throw new RangeError("Summary belongs to another Rust playback context");
      } else {
        const snapshot = this.readSnapshot();
        if (snapshot.phaseCode !== 7 || summary.context.phase !== "result"
            || JSON.stringify(summary.context.scenario) !== JSON.stringify(snapshot.identity.scenario)
            || JSON.stringify(summary.context.controlIdentity) !== JSON.stringify(snapshot.identity.controls)
            || JSON.stringify(summary.context.finalization) !== JSON.stringify(snapshot.frame.finalization)) {
          throw new RangeError("Summary belongs to another Rust Result context");
        }
      }
      return summary;
    });
  }

  queryWindGrid(request: NamedWindGridRequest): NamedWindGrid {
    return this.observe(() => this.queryWindGridWithContext(request, this.readRecordSummary().context));
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
    }, "retain_record");
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
    }, "retain_record");
  }

  private queryWindGridWithContext(request: NamedWindGridRequest, context: NamedRecordContext): NamedWindGrid {
    return this.observe(() => {
      const archived = this.port.is_archived_replay();
      if (typeof archived !== "boolean") throw new RangeError("Rust archive projection requires a boolean");
      const source = context.phase === "attract" ? "attract" : archived ? "archive" : "record";
      const json = this.port.flight_wind_grid_json(request.northMinimumMeters, request.eastMinimumMeters, request.altitudeMeters, request.spacingMeters);
      return parseNamedWindGrid(json, context, request, source);
    });
  }

  private requireRecordContext(context: NamedRecordContext): void {
    if (!sameNamedRecordContext(context, this.readRecordSummary().context)) throw new RangeError("Saved query belongs to another record context");
  }

  private requireCursorRecordContext(context: NamedRecordContext): void {
    const phaseCode = this.readLifecycle().phaseCode;
    if (phaseCode === 9 || phaseCode === 10) {
      if (!sameNamedRecordContext(context, this.readPlaybackContext())) throw new RangeError("Saved cursor belongs to another playback context");
      return;
    }
    if (phaseCode !== 7 || context.phase !== "result") throw new RangeError("Saved cursor requires the expected Rust record phase");
    const snapshot = this.readSnapshot();
    if (snapshot.phaseCode !== 7 || JSON.stringify(context.scenario) !== JSON.stringify(snapshot.identity.scenario)
        || JSON.stringify(context.controlIdentity) !== JSON.stringify(snapshot.identity.controls)
        || JSON.stringify(context.finalization) !== JSON.stringify(snapshot.frame.finalization)) {
      throw new RangeError("Saved cursor belongs to another Result context");
    }
  }

  private observeRecordQuery<Value>(read: () => Value): Value {
    const token = this.captureQueryToken();
    const value = this.observe(read);
    if (this.acceptQuery(token, value).kind === "stale") throw new RangeError("Saved record query owner or generation changed during observation");
    return value;
  }

  private recordContext(): RecordQueryContext {
    const phase = this.readLifecycle().phaseCode;
    return phase === 9 || phase === 10 ? this.readPlaybackContext() : tailResultRecordContext(this.readSnapshot());
  }
}

export type AppSessionFacade = LegacyAppSessionFacade | TailAppSessionFacade;

function isAnalysisObservationProof(value: unknown): value is AnalysisObservationProof {
  if (typeof value !== "object" || value === null || !("source" in value) || !("context" in value)) return false;
  const source = value.source;
  return typeof source === "object" && source !== null && queryOwner in source && recordSourceGeneration in source
    && typeof source[queryOwner] === "symbol" && typeof source[recordSourceGeneration] === "symbol"
    && typeof value.context === "object" && value.context !== null;
}

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
