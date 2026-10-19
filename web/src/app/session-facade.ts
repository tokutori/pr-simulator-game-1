import { projectTailGameSession } from "./session-snapshot.js";
import type { PlaybackSessionSnapshot, ReplayReturnTarget, SessionLifecycleProjection, TailGameSessionProjection } from "./session-snapshot.js";
import { readFlightLog } from "../game/flight-log-export.js";
import type { FlightLogExportPort, FlightLogFormat } from "../game/flight-log-export.js";
import type { NamedAnalysisCursor, NamedAnalysisDataset } from "../game/flight-analysis-view.js";
import type { TailResultSnapshot, TailSessionPort } from "../game/tail-flight-controller.js";
import { encodeTailLogicalInput } from "../game/tail-input.js";
import type { DisplayAvailability, FlightRenderPose, RecordedFlightRenderPose } from "../render/contracts/runtime.js";
import { projectReplayCamera } from "../render/camera/replay-camera.js";
import { encodeGameUiAction, encodeGameUiPlatformInput } from "./game-ui-codec.js";
import type { GameUiPlatformObservation } from "./game-ui-codec.js";
import type { GameUiReplayCamera, ReplayCameraView } from "../generated/replay-camera.js";
import type { GameUiAction } from "../generated/game-ui-action.js";
import type { GameUiEffect, GameUiUpdate } from "../generated/game-ui-effects.js";
import type { SessionCommand } from "./game-ui-types.js";
import type { GameUiView } from "../generated/game-ui-view.js";
import type { GameUiFrame } from "../generated/game-ui-frame.js";
import type { TailSnapshotDocument } from "../generated/tail-snapshot.js";
import type { TailPreparedLaunchDocument } from "../generated/tail-prepared.js";
import type { TailControlProfileDocument } from "../generated/tail-control-profile.js";
import type { EnvironmentSnapshot } from "../generated/runtime-environment.js";
import type { PlaybackContext } from "../generated/record-context.js";
import type { AnalysisDocument } from "../generated/record-analysis.js";
import type { RecordedPlaybackSample } from "../generated/record-sample.js";
import type { SummaryDocument } from "../generated/record-summary.js";
import type { GridRequest, WindDocument } from "../generated/record-wind.js";
import { mergeGameUiFrame } from "./game-ui-frame.js";

const queryOwner = Symbol("session query owner");
const queryGeneration = Symbol("session query generation");
const recordSourceGeneration = Symbol("record source generation");
const analysisObservation = Symbol("saved analysis observation");
export interface SessionQueryToken {
  readonly [queryOwner]: symbol;
  readonly [queryGeneration]: symbol;
}
export type SessionQueryAcceptance<Value> = Readonly<{ kind: "accepted"; value: Value }> | Readonly<{ kind: "stale" }>;
export interface CountdownObservationToken {
  readonly query: SessionQueryToken;
  readonly attempt: number;
}
export type CountdownProgress = Readonly<{ kind: "ignored" }> | Readonly<{ kind: "cancelled" }>
  | Readonly<{ kind: "waiting"; remainingSeconds: number }> | Readonly<{ kind: "launched" }>;
interface RecordSourceToken {
  readonly [queryOwner]: symbol;
  readonly [recordSourceGeneration]: symbol;
}
export interface ObservedNamedAnalysisDataset extends NamedAnalysisDataset {
  readonly [analysisObservation]: RecordSourceToken;
}
export type WindGridRequest = Pick<GridRequest, "north_min_m" | "east_min_m" | "altitude_m" | "spacing_m">;
export interface PlaybackClock {
  readonly timeSeconds: number;
  readonly rateCode: 0 | 1 | 2;
  readonly kind: "paused" | "playing";
}
export interface SessionResourcePort extends FlightLogExportPort {
  lifecycle_snapshot_json(): string;
  countdown_remaining_seconds(): number;
  countdown_token(): number;
  observe_countdown(token: number, elapsedSeconds: number, frameAvailable: boolean): number;
  clear_pause_reason(reason: number): void;
  environment_snapshot_json(): string;
  export_flight_record_json(): string;
  open_archived_flight_record(json: string): void;
  is_archived_replay(): boolean;
  free(): void;
}
const ownedSessionResources = new WeakSet<SessionResourcePort>();
export interface TailAppSessionPort extends SessionResourcePort {
  snapshot_json(): string;
  control_profile_json(): string;
  advance_tick_json(json: string): string;
  pause(reason: number): void;
  prepared_launch_snapshot_json(): string;
  game_ui_view_revision(platformJson: string): number;
  game_ui_view_json(platformJson: string): string;
  game_ui_frame_json(expectedRevision: number, platformJson: string): string;
  game_ui_action_json(actionJson: string, platformJson: string): string;
  apply_game_ui_session_command_json(commandJson: string): void;
  apply_game_ui_record_action_json(actionJson: string): void;
  playback_context_json(): string;
  flight_analysis_samples_json(): string;
  flight_record_sample_at_seconds(seconds: number): string;
  playback_camera_view_json(seconds: number, selection: string, screen: boolean): string;
  playback_clock_state(): ArrayLike<number>;
  seek_playback(seconds: number): ArrayLike<number>;
  set_playback_rate_code(code: number): ArrayLike<number>;
  set_playback_playing(playing: boolean): ArrayLike<number>;
  advance_playback(elapsedSeconds: number): ArrayLike<number>;
  flight_record_summary_json(): string;
  flight_wind_grid_json(northMinimumMeters: number, eastMinimumMeters: number, altitudeMeters: number, spacingMeters: number): string;
}
type CompletedOperation = Readonly<{ kind: "completed" }> | Readonly<{ kind: "countdown-started" }>;
export type TailAppOperationResult = CompletedOperation | Readonly<{ kind: "aborted"; terminalSnapshot: TailResultSnapshot }>;

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
    return this.observe(() => ({ [queryOwner]: this.owner, [queryGeneration]: this.generation }));
  }

  acceptQuery<Value>(token: SessionQueryToken, value: Value): SessionQueryAcceptance<Value> {
    return this.resourceState === "alive" && token[queryOwner] === this.owner && token[queryGeneration] === this.generation
      ? { kind: "accepted", value } : { kind: "stale" };
  }

  protected captureRecordSourceToken(): RecordSourceToken {
    return this.observe(() => ({ [queryOwner]: this.owner, [recordSourceGeneration]: this.sourceGeneration }));
  }

  protected recordSourceIsCurrent(token: RecordSourceToken): boolean {
    return this.resourceState === "alive" && token[queryOwner] === this.owner && token[recordSourceGeneration] === this.sourceGeneration;
  }

  readLifecycle(): SessionLifecycleProjection {
    return this.readOutput(() => this.resource.lifecycle_snapshot_json());
  }

  captureCountdownToken(): CountdownObservationToken | null {
    return this.observe(() => {
      const attempt = this.resource.countdown_token();
      return attempt === 0 ? null : { query: this.captureQueryToken(), attempt };
    });
  }

  observeCountdown(token: CountdownObservationToken | null, elapsedSeconds: number, frameAvailable: boolean): CountdownProgress {
    if (token === null || this.acceptQuery(token.query, token.attempt).kind === "stale") return { kind: "ignored" };
    return this.change(() => {
      switch (this.resource.observe_countdown(token.attempt, elapsedSeconds, frameAvailable)) {
        case 0: return { kind: "ignored" };
        case 1: return { kind: "cancelled" };
        case 2: return { kind: "waiting", remainingSeconds: this.resource.countdown_remaining_seconds() };
        case 3: return { kind: "launched" };
        default: throw new RangeError("Rust Countdown progress is unsupported");
      }
    });
  }

  clearPauseReason(reason: number): void {
    this.change(() => { this.resource.clear_pause_reason(reason); });
  }

  readEnvironment(): EnvironmentSnapshot {
    return this.readOutput(() => this.resource.environment_snapshot_json());
  }

  readReplayReturnTarget(): ReplayReturnTarget {
    return this.observe(() => this.resource.is_archived_replay() ? "title" : "result");
  }

  exportRecordJson(): string {
    return this.observe(() => this.resource.export_flight_record_json());
  }

  readFlightLog(format: FlightLogFormat): string {
    return this.observe(() => readFlightLog(this.resource, format));
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

  protected readOutput<Value>(read: () => string): Value {
    return this.observe(() => JSON.parse(read()) as Value);
  }
}

export class TailAppSessionFacade extends SessionResourceOwner {
  readonly flightPort: TailSessionPort;
  private sharedUiCache: GameUiView | null = null;

  constructor(private readonly port: TailAppSessionPort, physicsHz: number) {
    super(port, physicsHz);
    this.flightPort = {
      readSnapshot: () => this.readSnapshot(),
      readControlProfile: () => this.readOutput<TailControlProfileDocument>(() => port.control_profile_json()),
      advanceTick: (input) => this.change(() => this.readOutput<TailSnapshotDocument>(() => port.advance_tick_json(encodeTailLogicalInput(input)))),
      pause: (reason) => { this.change(() => { port.pause(reason); }, "retain_record"); },
      clear_pause_reason: (reason) => { this.clearPauseReason(reason); },
      free: () => { this.dispose(); }
    };
  }

  readSharedUi(platform: GameUiPlatformObservation): GameUiView {
    return this.observe(() => {
      const platformJson = encodeGameUiPlatformInput(platform);
      const revision = this.port.game_ui_view_revision(platformJson);
      this.sharedUiCache = this.sharedUiCache !== null && this.sharedUiCache.view_revision === revision
        ? mergeGameUiFrame(this.sharedUiCache, this.readOutput<GameUiFrame>(() => this.port.game_ui_frame_json(revision, platformJson)))
        : this.readOutput<GameUiView>(() => this.port.game_ui_view_json(platformJson));
      return this.sharedUiCache;
    });
  }

  updateSharedUi(action: GameUiAction, platform: GameUiPlatformObservation): GameUiUpdate {
    return this.change(() => this.readOutput(() => this.port.game_ui_action_json(encodeGameUiAction(action), encodeGameUiPlatformInput(platform))), "retain_record");
  }

  executeSharedSessionCommand(command: SessionCommand): TailAppOperationResult {
    return this.change(() => {
      this.port.apply_game_ui_session_command_json(JSON.stringify(command));
      if (command.kind === "start_countdown") return { kind: "countdown-started" };
      if (command.kind === "abort") return { kind: "aborted", terminalSnapshot: this.readSnapshot() as TailResultSnapshot };
      return { kind: "completed" };
    });
  }

  executeSharedRecordAction(action: Extract<GameUiEffect, { kind: "record" }>["action"]): void {
    this.change(() => { this.port.apply_game_ui_record_action_json(encodeGameUiAction(action)); }, "retain_record");
  }

  pauseForReason(reason: "page_hidden" | "presentation_transition"): void {
    this.change(() => { this.port.pause(reason === "page_hidden" ? 1 : 2); });
  }

  readSnapshot(): TailSnapshotDocument {
    return this.readOutput(() => this.port.snapshot_json());
  }

  readPreparedLaunchPose(): FlightRenderPose | null {
    const snapshot = this.readOutput<TailPreparedLaunchDocument>(() => this.port.prepared_launch_snapshot_json());
    return snapshot.preview.kind === "unavailable" ? null : snapshot.preview.render_pose;
  }

  readGameSessionProjection(): TailGameSessionProjection {
    return projectTailGameSession(this.readSnapshot(), this.readLifecycle());
  }

  readPlaybackContext(): PlaybackContext {
    return this.readOutput(() => this.port.playback_context_json());
  }

  readAnalysisSamples(): readonly RecordedPlaybackSample[] {
    return this.readOutput<AnalysisDocument>(() => this.port.flight_analysis_samples_json()).samples;
  }

  readAnalysisDataset(wind: DisplayAvailability<WindGridRequest, "not_requested">): ObservedNamedAnalysisDataset {
    const source = this.captureRecordSourceToken();
    const summary = this.readRecordSummary();
    return { kind: "named_record", context: summary.context, summary, samples: this.readAnalysisSamples(),
      windGrid: wind.kind === "unavailable" ? wind : { kind: "available", value: this.queryWindGrid(wind.value) },
      [analysisObservation]: source };
  }

  isAnalysisDatasetCurrent(dataset: NamedAnalysisDataset): boolean {
    return analysisObservation in dataset && isRecordSourceToken(dataset[analysisObservation])
      && this.recordSourceIsCurrent(dataset[analysisObservation]);
  }

  queryAnalysisCursor(seconds: number, expectedDataset: NamedAnalysisDataset): NamedAnalysisCursor {
    if (!this.isAnalysisDatasetCurrent(expectedDataset)) throw new RangeError("Analysis dataset owner or generation is stale");
    return { ...this.queryRecordSample(seconds), kind: "named_record", context: expectedDataset.context };
  }

  readRecordSummary(): SummaryDocument {
    return this.readOutput(() => this.port.flight_record_summary_json());
  }

  queryWindGrid(request: WindGridRequest): WindDocument {
    return this.readOutput(() => this.port.flight_wind_grid_json(request.north_min_m, request.east_min_m, request.altitude_m, request.spacing_m));
  }

  queryRecordSample(seconds: number): RecordedPlaybackSample {
    return this.readOutput(() => this.port.flight_record_sample_at_seconds(seconds));
  }

  queryRecordDisplay(seconds: number): PlaybackSessionSnapshot {
    const context = this.readRecordSummary().context;
    return { ...this.queryRecordSample(seconds), context };
  }

  queryReplayCamera(seconds: number, selection: GameUiReplayCamera, screen: boolean, dataset: NamedAnalysisDataset): RecordedFlightRenderPose {
    if (!this.isAnalysisDatasetCurrent(dataset)) throw new RangeError("Camera analysis record owner or generation is stale");
    const view = this.readOutput<ReplayCameraView>(() => this.port.playback_camera_view_json(seconds, selection, screen));
    return { ...view.render_pose, replayCamera: projectReplayCamera(view) };
  }

  readPlaybackClock(): PlaybackClock {
    return this.observe(() => playbackClock(this.port.playback_clock_state()));
  }

  seekPlayback(seconds: number): PlaybackClock {
    return this.change(() => playbackClock(this.port.seek_playback(seconds)), "retain_record");
  }

  setPlaybackRate(code: 0 | 1 | 2): PlaybackClock {
    return this.change(() => playbackClock(this.port.set_playback_rate_code(code)), "retain_record");
  }

  setPlaybackPlaying(playing: boolean): PlaybackClock {
    return this.change(() => playbackClock(this.port.set_playback_playing(playing)), "retain_record");
  }

  advancePlayback(elapsedSeconds: number): PlaybackClock {
    return this.change(() => playbackClock(this.port.advance_playback(elapsedSeconds)), "retain_record");
  }
}

export type AppSessionFacade = TailAppSessionFacade;

function playbackClock(values: ArrayLike<number>): PlaybackClock {
  return { timeSeconds: values[0]!, rateCode: values[1]! as 0 | 1 | 2, kind: values[2] === 0 ? "paused" : "playing" };
}

function isRecordSourceToken(value: unknown): value is RecordSourceToken {
  return typeof value === "object" && value !== null && queryOwner in value && recordSourceGeneration in value
    && typeof value[queryOwner] === "symbol" && typeof value[recordSourceGeneration] === "symbol";
}
