import type { FlightCameraMode, FlightRenderPose, PresentationMode } from "../render/contracts/runtime.js";
import type { UiAction } from "../render/contracts/ui.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightAnalysisData, FlightAnalysisSample } from "../game/flight-record-query.js";
import { acceptsFlightActivation, failFlightLaunch, flightRuntimeNotice, stopFlightRuntime } from "./flight-runtime-ui.js";
import type { FlightLaunchFailure, FlightRuntimeUiState } from "./flight-runtime-ui.js";

export interface StoredFlightRecordUiEntry {
  readonly id: number;
  readonly savedAt: string;
  readonly personalBest: boolean;
}

export type NamedGameSessionOperation =
  | "open-setup"
  | "set-control-manual"
  | "set-control-shared"
  | "set-control-automatic"
  | "cycle-difficulty-preset"
  | "cycle-information-level"
  | "cycle-assistance-level"
  | "cycle-weather-class"
  | "return-to-title"
  | "prepare"
  | "cancel-briefing"
  | "start-flight"
  | "cancel-countdown"
  | "pause"
  | "resume"
  | "abort"
  | "retry"
  | "retry-briefing"
  | "enter-replay"
  | "leave-replay"
  | "enter-attract"
  | "leave-attract";

export type GameSessionOperation = NamedGameSessionOperation
  | { readonly kind: "set-information-cue"; readonly cueCode: number; readonly visible: boolean };

export interface ReplayClockState {
  readonly timeSeconds: number;
  readonly rateCode: 0 | 1 | 2;
  readonly playing: boolean;
}

export type ReplayClockCommand =
  | { readonly kind: "synchronize"; readonly seekTimeSeconds: number | null }
  | { readonly kind: "play" }
  | { readonly kind: "pause" }
  | { readonly kind: "seek"; readonly timeSeconds: number }
  | { readonly kind: "rate"; readonly rateCode: 0 | 1 | 2 }
  | { readonly kind: "advance"; readonly elapsedSeconds: number };

type ScreenRecoveryOrigin =
  | { readonly origin: "page-restoration"; readonly from: null }
  | { readonly origin: "backend-fault"; readonly from: "webxr" | "phone-vr"; readonly cause: string };

export type LivePresentationUiState =
  | { readonly type: "uninitialized" }
  | { readonly type: "initializing"; readonly requestId: number }
  | { readonly type: "ready"; readonly mode: PresentationMode }
  | ({
      readonly type: "transitioning";
      readonly requestId: number;
    } & (
      | {
          readonly origin: "user-request";
          readonly from: PresentationMode | null;
          readonly to: PresentationMode;
          readonly phase: "requesting" | "stopping" | "starting";
        }
      | (ScreenRecoveryOrigin & { readonly to: "screen"; readonly phase: "stopping" })
    ))
  | { readonly type: "failed"; readonly message: string };

export type PresentationUiState = LivePresentationUiState
  | { readonly type: "cached"; readonly retained: LivePresentationUiState }
  | { readonly type: "hidden" };

export type GameSessionUiState =
  | { readonly kind: "boot"; readonly phaseCode: -1 }
  | { readonly kind: "title"; readonly phaseCode: 0 }
  | { readonly kind: "setup"; readonly phaseCode: 1 }
  | { readonly kind: "briefing-preparing"; readonly phaseCode: 2 }
  | { readonly kind: "briefing-ready"; readonly phaseCode: 3 }
  | { readonly kind: "countdown"; readonly phaseCode: 4; readonly countdownRemaining: number }
  | { readonly kind: "flight"; readonly phaseCode: 5; readonly snapshot: FlightSnapshot }
  | {
      readonly kind: "paused-flight";
      readonly phaseCode: 6;
      readonly snapshot: FlightSnapshot;
      readonly canResume: boolean;
      readonly overlay: PauseOverlayState;
    }
  | { readonly kind: "result"; readonly phaseCode: 7; readonly snapshot: FlightSnapshot | null }
  | { readonly kind: "briefing-failed"; readonly phaseCode: 8 }
  | { readonly kind: "replay"; readonly phaseCode: 9; readonly snapshot: FlightSnapshot | null }
  | { readonly kind: "attract"; readonly phaseCode: 10; readonly snapshot: null };

export type PauseOverlayState =
  | { readonly kind: "menu" }
  | { readonly kind: "settings" }
  | { readonly kind: "help" };

export interface AppModel {
  readonly status: string;
  readonly webXrAvailable: boolean;
  readonly phoneVrAvailable: boolean;
  readonly presentation: PresentationUiState;
  readonly gameSession: GameSessionUiState;
  readonly controlModeCode: number;
  readonly difficulty: DifficultyUiState;
  readonly configurationMetadata: ConfigurationMetadataUiState | null;
  readonly flightAnalysis: FlightAnalysisData | null;
  readonly pendingAnalysisRequestId: number | null;
  readonly nextAnalysisRequestId: number;
  readonly resultTab: "summary" | "analysis";
  readonly analysisChart: "map" | "altitude" | "speed";
  readonly analysisCursorTimeSeconds: number;
  readonly analysisCursorSample: FlightAnalysisSample | null;
  readonly pendingAnalysisCursorRequestId: number | null;
  readonly nextAnalysisCursorRequestId: number;
  readonly replayPose: FlightRenderPose | null;
  readonly pendingReplayPoseRequestId: number | null;
  readonly nextReplayPoseRequestId: number;
  readonly replayPlaying: boolean;
  readonly replayViewMode: "cinematic" | "telemetry" | "analysis";
  readonly replaySpeed: 0.5 | 1 | 2;
  readonly replayClockGeneration: number;
  readonly pendingReplayClockRequestId: number | null;
  readonly nextReplayClockRequestId: number;
  readonly replayCameraMode: "auto" | FlightCameraMode;
  readonly storedFlightRecords: readonly StoredFlightRecordUiEntry[];
  readonly storedFlightRecordsStatus: string;
  readonly pendingRecordListRequestId: number | null;
  readonly nextRecordListRequestId: number;
  readonly pendingGameRequestId: number | null;
  readonly expectedLaunchRequestId: number | null;
  readonly flightRuntime: FlightRuntimeUiState;
  readonly nextRequestId: number;
}

export interface DifficultyUiState {
  readonly presetCode: number;
  readonly informationCode: number;
  readonly hudProfile: HudProfileUiState;
  readonly assistanceCode: number;
  readonly weatherCode: number;
}

export interface HudProfileUiState {
  readonly telemetry: boolean;
  readonly attitude: boolean;
  readonly wind: boolean;
  readonly flightPath: boolean;
  readonly angleOfAttack: boolean;
  readonly warnings: boolean;
}

export interface ConfigurationMetadataUiState {
  readonly presetCode: number;
  readonly informationCode: number;
  readonly hudProfile: HudProfileUiState;
  readonly assistanceCode: number;
  readonly weatherCode: number;
  readonly catalogVersion: number;
  readonly scenarioId: number;
  readonly scenarioVersion: number;
  readonly aircraftModelVersion: number;
  readonly environmentVersion: number;
  readonly controllerProfileVersion: number;
  readonly seedLow: number;
  readonly seedHigh: number;
}

export interface GameSessionProjection {
  readonly phaseCode: number;
  readonly controlModeCode: number;
  readonly difficulty: DifficultyUiState;
  readonly configurationMetadata: ConfigurationMetadataUiState | null;
  readonly countdownRemaining: number;
  readonly snapshot: FlightSnapshot | null;
  readonly canResume: boolean;
}

export type AppMessage =
  | { readonly type: "initialize" }
  | {
      readonly type: "presentation-initialized";
      readonly requestId: number;
      readonly activeMode: PresentationMode | null;
      readonly webXrAvailable: boolean;
      readonly phoneVrAvailable: boolean;
      readonly status: string;
    }
  | { readonly type: "presentation-initialization-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "ui-action"; readonly action: UiAction }
  | { readonly type: "refresh-stored-flight-records" }
  | { readonly type: "stored-flight-records-loaded"; readonly requestId: number; readonly records: readonly StoredFlightRecordUiEntry[] }
  | { readonly type: "stored-flight-records-failed"; readonly requestId: number; readonly message: string }
  | {
      readonly type: "permission-completed";
      readonly requestId: number;
      readonly mode: "webxr" | "phone-vr";
      readonly ok: boolean;
      readonly message: string;
    }
  | {
      readonly type: "backend-transition-completed";
      readonly requestId: number;
      readonly requestedMode: PresentationMode;
      readonly activeMode: PresentationMode | null;
      readonly ok: boolean;
      readonly message: string;
      readonly successStatus: string;
    }
  | { readonly type: "backend-ended"; readonly mode: "webxr" | "phone-vr"; readonly message: string }
  | { readonly type: "page-hidden" }
  | { readonly type: "page-suspended" }
  | { readonly type: "page-restored" }
  | {
      readonly type: "game-session-synced";
      readonly phaseCode: number;
      readonly controlModeCode: number;
      readonly difficulty: DifficultyUiState;
      readonly configurationMetadata: ConfigurationMetadataUiState | null;
      readonly countdownRemaining: number;
      readonly snapshot: FlightSnapshot | null;
      readonly canResume?: boolean;
    }
  | {
      readonly type: "game-operation-completed";
      readonly requestId: number;
      readonly phaseCode: number;
      readonly controlModeCode: number;
      readonly difficulty: DifficultyUiState;
      readonly configurationMetadata: ConfigurationMetadataUiState | null;
      readonly countdownRemaining: number;
      readonly snapshot: FlightSnapshot | null;
      readonly canResume?: boolean;
    }
  | { readonly type: "flight-controller-activated"; readonly launchRequestId: number; readonly projection: GameSessionProjection }
  | { readonly type: "flight-controller-stopped"; readonly launchRequestId: number; readonly cause: string; readonly snapshot: FlightSnapshot }
  | {
      readonly type: "flight-launch-failed";
      readonly launchRequestId: number;
      readonly cause: string;
      readonly outcome: FlightLaunchFailure<GameSessionProjection>;
    }
  | { readonly type: "flight-recovery-completed"; readonly launchRequestId: number; readonly requestId: number; readonly projection: GameSessionProjection }
  | { readonly type: "flight-recovery-failed"; readonly launchRequestId: number; readonly requestId: number; readonly cause: string }
  | { readonly type: "game-session-status"; readonly message: string }
  | { readonly type: "flight-analysis-loaded"; readonly requestId: number; readonly data: FlightAnalysisData }
  | { readonly type: "flight-analysis-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-analysis-cursor-loaded"; readonly requestId: number; readonly sample: FlightAnalysisSample }
  | { readonly type: "flight-analysis-cursor-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-replay-pose-loaded"; readonly requestId: number; readonly pose: FlightRenderPose }
  | { readonly type: "flight-replay-pose-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "replay-clock-tick"; readonly generation: number; readonly elapsedSeconds: number }
  | {
      readonly type: "replay-clock-command-completed";
      readonly requestId: number;
      readonly generation: number;
      readonly state: ReplayClockState;
    }
  | {
      readonly type: "replay-clock-command-failed";
      readonly requestId: number;
      readonly generation: number;
      readonly message: string;
      readonly state?: ReplayClockState;
    }
  | {
      readonly type: "game-operation-failed";
      readonly requestId: number;
      readonly message: string;
      readonly currentSession?: GameSessionProjection;
    };

export type AppEffect =
  | { readonly type: "initialize-presentation"; readonly requestId: number }
  | { readonly type: "request-permission"; readonly mode: "webxr" | "phone-vr"; readonly requestId: number }
  | { readonly type: "switch-backend"; readonly mode: PresentationMode; readonly requestId: number }
  | { readonly type: "cancel-pending-request"; readonly mode: "webxr" | "phone-vr" }
  | { readonly type: "recenter-tracking" }
  | { readonly type: "recenter-menu" }
  | { readonly type: "dispose-presentation" }
  | { readonly type: "suspend-page-flight" }
  | { readonly type: "restore-page-flight" }
  | { readonly type: "persist-flight-record" }
  | { readonly type: "load-stored-flight-records"; readonly requestId: number }
  | { readonly type: "open-stored-flight-record"; readonly id: number; readonly requestId: number }
  | { readonly type: "load-flight-analysis"; readonly requestId: number }
  | { readonly type: "load-flight-analysis-cursor"; readonly requestId: number; readonly timeSeconds: number }
  | { readonly type: "load-flight-replay-pose"; readonly requestId: number; readonly timeSeconds: number }
  | { readonly type: "schedule-replay-clock-tick"; readonly generation: number; readonly delayMilliseconds: number }
  | {
      readonly type: "control-replay-clock";
      readonly requestId: number;
      readonly generation: number;
      readonly command: ReplayClockCommand;
    }
  | { readonly type: "game-session-operation"; readonly operation: GameSessionOperation; readonly requestId: number }
  | { readonly type: "recover-stopped-flight"; readonly launchRequestId: number; readonly requestId: number };

export interface AppTransition {
  readonly model: AppModel;
  readonly effects: readonly AppEffect[];
}

export function gameSessionPhaseCode(session: GameSessionUiState): number {
  return session.phaseCode;
}

export function isStaleGameFlowActivation(
  action: UiAction,
  displayedPhaseCode: number,
  actualPhaseCode: number
): boolean {
  return isGameFlowActivation(action) && displayedPhaseCode !== actualPhaseCode;
}

export function isGameFlowActivation(action: UiAction): boolean {
  return (action.type === "activate" || action.type === "set-toggle") && action.controlId.startsWith("game-");
}

export function gameSessionSnapshot(session: GameSessionUiState): FlightSnapshot | null {
  return "snapshot" in session ? session.snapshot : null;
}

export function gameSessionCountdown(session: GameSessionUiState): number {
  return session.kind === "countdown" ? session.countdownRemaining : 0;
}

export function flightDiagnosticNotice(model: AppModel): string | null {
  return flightRuntimeNotice(model, gameSessionPhaseCode(model.gameSession));
}

export function flightRecoveryControlId(model: AppModel): string | null {
  const diagnostic = model.flightRuntime;
  return diagnostic.kind === "projection-unavailable" || diagnostic.kind === "stopped" && (model.gameSession.kind === "flight" || model.gameSession.kind === "paused-flight")
    ? `flight-recovery-abort-${String(diagnostic.launchRequestId)}` : null;
}

export function gameSessionState(
  phaseCode: number,
  countdownRemaining: number,
  snapshot: FlightSnapshot | null,
  canResume = false,
  previous: GameSessionUiState | null = null
): GameSessionUiState | null {
  switch (phaseCode) {
    case -1: return { kind: "boot", phaseCode: -1 };
    case 0: return { kind: "title", phaseCode: 0 };
    case 1: return { kind: "setup", phaseCode: 1 };
    case 2: return { kind: "briefing-preparing", phaseCode: 2 };
    case 3: return { kind: "briefing-ready", phaseCode: 3 };
    case 4:
      return Number.isSafeInteger(countdownRemaining) && countdownRemaining >= 0
        ? { kind: "countdown", phaseCode: 4, countdownRemaining }
        : null;
    case 5:
      return snapshot === null ? null : { kind: "flight", phaseCode, snapshot };
    case 6:
      return snapshot === null ? null : {
        kind: "paused-flight",
        phaseCode,
        snapshot,
        canResume,
        overlay: previous?.kind === "paused-flight" ? previous.overlay : { kind: "menu" }
      };
    case 7: return { kind: "result", phaseCode: 7, snapshot };
    case 8: return { kind: "briefing-failed", phaseCode: 8 };
    case 9: return { kind: "replay", phaseCode: 9, snapshot };
    case 10: return { kind: "attract", phaseCode: 10, snapshot: null };
    default: return null;
  }
}

export function createInitialAppModel(): AppModel {
  return Object.freeze({
    status: "Screen renderer is initializing",
    webXrAvailable: false,
    phoneVrAvailable: false,
    presentation: Object.freeze({ type: "uninitialized" }),
    gameSession: Object.freeze({ kind: "boot", phaseCode: -1 }),
    controlModeCode: 0,
    difficulty: Object.freeze({
      presetCode: 4,
      informationCode: 0,
      hudProfile: fullHudProfile(),
      assistanceCode: 3,
      weatherCode: 0
    }),
    configurationMetadata: null,
    flightAnalysis: null,
    pendingAnalysisRequestId: null,
    nextAnalysisRequestId: 1,
    resultTab: "summary",
    analysisChart: "map",
    analysisCursorTimeSeconds: 0,
    analysisCursorSample: null,
    pendingAnalysisCursorRequestId: null,
    nextAnalysisCursorRequestId: 1,
    replayPose: null,
    pendingReplayPoseRequestId: null,
    nextReplayPoseRequestId: 1,
    replayPlaying: false,
    replayViewMode: "cinematic",
    replaySpeed: 1,
    replayClockGeneration: 0,
    pendingReplayClockRequestId: null,
    nextReplayClockRequestId: 1,
    replayCameraMode: "auto",
    storedFlightRecords: Object.freeze([]),
    storedFlightRecordsStatus: "保存記録を読み込んでいる",
    pendingRecordListRequestId: null,
    nextRecordListRequestId: 1,
    pendingGameRequestId: null,
    expectedLaunchRequestId: null,
    flightRuntime: Object.freeze({ kind: "inactive" }),
    nextRequestId: 1
  });
}

export function updateApp(model: AppModel, message: AppMessage): AppTransition {
  if (model.presentation.type === "cached") {
    if (message.type === "page-suspended" || message.type === "ui-action"
        || message.type === "replay-clock-tick" || message.type === "backend-ended") return transition(model);
    if (message.type === "permission-completed") {
      return transition(model, message.ok ? [{ type: "cancel-pending-request", mode: message.mode }] : []);
    }
    const retained = withModel(model, { presentation: model.presentation.retained });
    if (message.type === "page-restored") {
      const recovered = retained.presentation.type === "initializing" || retained.presentation.type === "uninitialized"
          || retained.presentation.type === "failed"
        ? transition(retained)
        : beginScreenRecovery(retained, { origin: "page-restoration", from: null });
      const playback = recovered.model.gameSession.kind === "attract"
        ? beginReplayClockCommand(recovered.model, { kind: "play" }, true)
        : transition(recovered.model);
      return transition(playback.model, [
        ...recovered.effects, { type: "restore-page-flight" }, ...playback.effects
      ]);
    }
    const updated = updateApp(retained, message);
    if (updated.model.presentation.type === "hidden") return updated;
    if (updated.model.presentation.type === "cached") return updated;
    return transition(withModel(updated.model, {
      presentation: Object.freeze({ type: "cached", retained: updated.model.presentation })
    }), updated.effects);
  }
  if (model.presentation.type === "hidden") {
    if (message.type === "permission-completed" && message.ok) {
      return transition(model, [{ type: "cancel-pending-request", mode: message.mode }]);
    }
    return transition(model);
  }

  switch (message.type) {
    case "page-restored":
      return transition(model);
    case "page-suspended": {
      let suspended = withModel(model, {
        replayPlaying: false,
        replayClockGeneration: model.replayClockGeneration + 1,
        pendingReplayClockRequestId: null
      });
      const effects: AppEffect[] = [
        { type: "suspend-page-flight" },
        { type: "cancel-pending-request", mode: "webxr" },
        { type: "cancel-pending-request", mode: "phone-vr" }
      ];
      if (model.gameSession.kind === "replay" || model.gameSession.kind === "attract") {
        const paused = beginReplayClockCommand(suspended, { kind: "pause" }, false);
        suspended = paused.model;
        effects.push(...paused.effects);
      }
      if (suspended.presentation.type === "hidden" || suspended.presentation.type === "cached") return transition(model);
      return transition(withModel(suspended, {
        presentation: Object.freeze({ type: "cached", retained: suspended.presentation })
      }), effects);
    }
    case "initialize": {
      if (model.presentation.type !== "uninitialized") return transition(model);
      const requestId = model.nextRequestId;
      return transition(
        withModel(model, {
          presentation: Object.freeze({ type: "initializing", requestId }),
          nextRequestId: requestId + 1
        }),
        [{ type: "initialize-presentation", requestId }]
      );
    }
    case "presentation-initialized": {
      if (model.presentation.type !== "initializing" || model.presentation.requestId !== message.requestId) {
        return transition(model);
      }
      if (message.activeMode === null) {
        const status = "Presentation initialization completed without an active backend";
        return transition(withModel(model, {
          presentation: Object.freeze({ type: "failed", message: status }),
          webXrAvailable: message.webXrAvailable,
          phoneVrAvailable: message.phoneVrAvailable,
          status
        }));
      }
      return beginStoredFlightRecordLoad(withModel(model, {
        presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
        webXrAvailable: message.webXrAvailable,
        phoneVrAvailable: message.phoneVrAvailable,
        status: message.status
      }));
    }
    case "presentation-initialization-failed": {
      if (model.presentation.type !== "initializing" || model.presentation.requestId !== message.requestId) {
        return transition(model);
      }
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "failed", message: message.message }),
        status: message.message
      }));
    }
    case "ui-action":
      return updateUiAction(model, message.action);
    case "refresh-stored-flight-records":
      return beginStoredFlightRecordLoad(model);
    case "stored-flight-records-loaded":
      if (model.pendingRecordListRequestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        storedFlightRecords: Object.freeze([...message.records]),
        storedFlightRecordsStatus: message.records.length === 0 ? "保存済みFlightRecordはない" : "",
        pendingRecordListRequestId: null
      }));
    case "stored-flight-records-failed":
      if (model.pendingRecordListRequestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        storedFlightRecordsStatus: `FlightRecord一覧を取得できない: ${message.message}`,
        pendingRecordListRequestId: null
      }));
    case "permission-completed":
      return updatePermissionCompletion(model, message);
    case "backend-transition-completed":
      return updateBackendCompletion(model, message);
    case "backend-ended": {
      if (model.presentation.type !== "ready" || model.presentation.mode !== message.mode) return transition(model);
      return beginScreenRecovery(model, { origin: "backend-fault", from: message.mode, cause: message.message });
    }
    case "flight-controller-activated": {
      if (!acceptsFlightActivation(model, message.launchRequestId, gameSessionPhaseCode(model.gameSession), gameSessionCountdown(model.gameSession), message.projection)) return transition(model);
      const activated = withModel(model, { expectedLaunchRequestId: null, flightRuntime: Object.freeze({ kind: "active", launchRequestId: message.launchRequestId }), status: "" });
      return updateApp(activated, { type: "game-session-synced", ...message.projection });
    }
    case "flight-controller-stopped": {
      const flightRuntime = stopFlightRuntime(model.flightRuntime, message.launchRequestId, gameSessionPhaseCode(model.gameSession), message.cause);
      if (flightRuntime === model.flightRuntime || model.expectedLaunchRequestId !== null) return transition(model);
      const canResume = model.gameSession.kind === "paused-flight" && model.gameSession.canResume;
      const gameSession = model.gameSession.kind === "result" ? model.gameSession
        : gameSessionState(gameSessionPhaseCode(model.gameSession), 0, message.snapshot, canResume, model.gameSession);
      return gameSession === null ? transition(model) : transition(withModel(model, { flightRuntime, gameSession: Object.freeze(gameSession) }));
    }
    case "flight-launch-failed": {
      const result = failFlightLaunch(model, message.launchRequestId, gameSessionPhaseCode(model.gameSession), gameSessionCountdown(model.gameSession), message.cause, message.outcome);
      if (result === null) return transition(model);
      const failed = withModel(model, { ...result.correlation, status: `発進準備に失敗した: ${message.cause}` });
      return result.projection === null ? transition(failed) : updateApp(failed, { type: "game-session-synced", ...result.projection });
    }
    case "flight-recovery-completed": {
      if (model.pendingGameRequestId !== message.requestId || model.flightRuntime.kind !== "stopped" && model.flightRuntime.kind !== "projection-unavailable"
          || model.flightRuntime.launchRequestId !== message.launchRequestId) return transition(model);
      if (gameSessionState(message.projection.phaseCode, message.projection.countdownRemaining, message.projection.snapshot, message.projection.canResume, model.gameSession) === null) return transition(withModel(model, { pendingGameRequestId: null, status: "終了処理後の飛行状態を取得できない" }));
      const recovered = withModel(model, { flightRuntime: Object.freeze({ ...model.flightRuntime, kind: "stopped" }) });
      return updateApp(recovered, { type: "game-operation-completed", requestId: message.requestId, ...message.projection });
    }
    case "flight-recovery-failed": {
      if (model.pendingGameRequestId !== message.requestId || model.flightRuntime.kind !== "stopped" && model.flightRuntime.kind !== "projection-unavailable"
          || model.flightRuntime.launchRequestId !== message.launchRequestId) return transition(model);
      return transition(withModel(model, { pendingGameRequestId: null, status: `安全終了の状態確認に失敗した: ${message.cause}` }));
    }
    case "game-session-synced": {
      if (model.flightRuntime.kind === "projection-unavailable" || model.expectedLaunchRequestId !== null && (message.phaseCode === 5 || message.phaseCode === 6)) return transition(model);
      const gameSession = gameSessionState(message.phaseCode, message.countdownRemaining, message.snapshot, message.canResume ?? false, model.gameSession);
      if (gameSession === null) return transition(withModel(model, { status: "無効なGameSession snapshotを破棄した" }));
      const previousPhaseCode = gameSessionPhaseCode(model.gameSession);
      const nextPhaseCode = gameSessionPhaseCode(gameSession);
      const returningReplay = previousPhaseCode === 9 && nextPhaseCode === 7;
      const enteringResult = entersResult(previousPhaseCode, nextPhaseCode) && !returningReplay;
      const analysisRequestId = enteringResult ? model.nextAnalysisRequestId : null;
      const effects: AppEffect[] = enteringResult && analysisRequestId !== null
        ? [{ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: analysisRequestId }]
        : [];
      return transition(withModel(model, {
        gameSession: Object.freeze(gameSession),
        expectedLaunchRequestId: nextPhaseCode === 4 ? model.expectedLaunchRequestId : null,
        controlModeCode: message.controlModeCode,
        difficulty: message.difficulty,
        configurationMetadata: message.configurationMetadata,
        flightAnalysis: [7, 9, 10].includes(nextPhaseCode) ? model.flightAnalysis : null,
        pendingAnalysisRequestId: analysisRequestId,
        resultTab: enteringResult ? "summary" : model.resultTab,
        analysisChart: enteringResult ? "map" : model.analysisChart,
        analysisCursorTimeSeconds: enteringResult ? 0 : model.analysisCursorTimeSeconds,
        analysisCursorSample: [7, 9, 10].includes(nextPhaseCode) && !enteringResult ? model.analysisCursorSample : null,
        pendingAnalysisCursorRequestId: [7, 9, 10].includes(nextPhaseCode) && !enteringResult
          ? model.pendingAnalysisCursorRequestId
          : null,
        nextAnalysisRequestId: enteringResult ? model.nextAnalysisRequestId + 1 : model.nextAnalysisRequestId,
        replayPlaying: nextPhaseCode === 10 || nextPhaseCode === 9 ? model.replayPlaying : false,
        replayClockGeneration: [9, 10].includes(nextPhaseCode) ? model.replayClockGeneration : model.replayClockGeneration + 1,
        pendingReplayClockRequestId: [9, 10].includes(nextPhaseCode) ? model.pendingReplayClockRequestId : null
      }), effects);
    }
    case "game-operation-completed": {
      const gameSession = gameSessionState(message.phaseCode, message.countdownRemaining, message.snapshot, message.canResume ?? false, model.gameSession);
      if (model.pendingGameRequestId !== message.requestId) return transition(model);
      if (gameSession === null) return transition(withModel(model, {
        pendingGameRequestId: null,
        status: "GameSessionから不正な状態snapshotを受信した"
      }));
      const previousPhaseCode = gameSessionPhaseCode(model.gameSession);
      const nextPhaseCode = gameSessionPhaseCode(gameSession);
      const returningReplay = previousPhaseCode === 9 && nextPhaseCode === 7;
      const enteringResult = entersResult(previousPhaseCode, nextPhaseCode) && !returningReplay;
      const enteringReplay = previousPhaseCode !== 9 && nextPhaseCode === 9;
      const enteringAttract = previousPhaseCode !== 10 && nextPhaseCode === 10;
      const enteringPlayback = enteringReplay || enteringAttract;
      const leavingPlayback = [9, 10].includes(previousPhaseCode) && ![9, 10].includes(nextPhaseCode);
      const playbackClockRequestId = enteringPlayback ? model.nextReplayClockRequestId : null;
      const replayClockGeneration = enteringPlayback || leavingPlayback
        ? model.replayClockGeneration + 1
        : model.replayClockGeneration;
      const analysisRequestId = enteringResult || enteringAttract || (enteringReplay && model.flightAnalysis === null)
        ? model.nextAnalysisRequestId
        : null;
      const effects: AppEffect[] = [];
      if (playbackClockRequestId !== null) {
        effects.push({
          type: "control-replay-clock",
          requestId: playbackClockRequestId,
          generation: replayClockGeneration,
          command: {
            kind: "synchronize",
            seekTimeSeconds: enteringReplay && previousPhaseCode === 7
              ? model.analysisCursorTimeSeconds
              : null
          }
        });
      }
      if (enteringResult && analysisRequestId !== null) {
        effects.push({ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: analysisRequestId });
      } else if (enteringPlayback && analysisRequestId !== null) {
        effects.push({ type: "load-flight-analysis", requestId: analysisRequestId });
      }
      return transition(withModel(model, {
        pendingGameRequestId: null,
        gameSession: Object.freeze(gameSession),
        expectedLaunchRequestId: nextPhaseCode === 4 ? model.expectedLaunchRequestId : null,
        controlModeCode: message.controlModeCode,
        difficulty: message.difficulty,
        configurationMetadata: message.configurationMetadata,
        status: "",
        flightAnalysis: [7, 9, 10].includes(nextPhaseCode) ? model.flightAnalysis : null,
        pendingAnalysisRequestId: analysisRequestId,
        resultTab: enteringResult ? "summary" : model.resultTab,
        analysisChart: enteringResult ? "map" : model.analysisChart,
        analysisCursorTimeSeconds: enteringResult || enteringAttract || (enteringReplay && previousPhaseCode !== 7)
          ? 0
          : model.analysisCursorTimeSeconds,
        analysisCursorSample: [7, 9, 10].includes(nextPhaseCode) && !enteringResult ? model.analysisCursorSample : null,
        pendingAnalysisCursorRequestId: [7, 9, 10].includes(nextPhaseCode) && !enteringResult
          ? model.pendingAnalysisCursorRequestId
          : null,
        nextAnalysisRequestId: analysisRequestId === null ? model.nextAnalysisRequestId : analysisRequestId + 1,
        replayPose: enteringPlayback ? null : [9, 10].includes(nextPhaseCode) ? model.replayPose : null,
        pendingReplayPoseRequestId: enteringPlayback ? null : model.pendingReplayPoseRequestId,
        replayPlaying: enteringPlayback ? false : [9, 10].includes(nextPhaseCode) ? model.replayPlaying : false,
        replayViewMode: enteringReplay ? "cinematic" : model.replayViewMode,
        replayClockGeneration,
        pendingReplayClockRequestId: enteringPlayback
          ? playbackClockRequestId
          : leavingPlayback ? null : model.pendingReplayClockRequestId,
        nextReplayClockRequestId: playbackClockRequestId === null
          ? model.nextReplayClockRequestId
          : playbackClockRequestId + 1
      }), effects);
    }
    case "flight-analysis-loaded":
      if (model.pendingAnalysisRequestId !== message.requestId || ![7, 9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      {
        const requestId = model.nextAnalysisCursorRequestId;
        const playbackPhase = gameSessionPhaseCode(model.gameSession);
        const replayPoseRequestId = [9, 10].includes(playbackPhase) ? model.nextReplayPoseRequestId : null;
        const timeSeconds = model.analysisCursorTimeSeconds;
        const effects: AppEffect[] = [{ type: "load-flight-analysis-cursor", requestId, timeSeconds }];
        if (replayPoseRequestId !== null) {
          effects.push({ type: "load-flight-replay-pose", requestId: replayPoseRequestId, timeSeconds });
        }
        if ([9, 10].includes(playbackPhase) && model.replayPlaying) {
          effects.push({ type: "schedule-replay-clock-tick", generation: model.replayClockGeneration, delayMilliseconds: 50 });
        }
        return transition(withModel(model, {
          flightAnalysis: message.data,
          pendingAnalysisRequestId: null,
          pendingAnalysisCursorRequestId: requestId,
          nextAnalysisCursorRequestId: requestId + 1,
          pendingReplayPoseRequestId: replayPoseRequestId,
          nextReplayPoseRequestId: replayPoseRequestId === null ? model.nextReplayPoseRequestId : replayPoseRequestId + 1
        }), effects);
      }
    case "flight-analysis-failed":
      if (model.pendingAnalysisRequestId !== message.requestId || ![7, 9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        flightAnalysis: null,
        pendingAnalysisRequestId: null,
        pendingAnalysisCursorRequestId: null,
        replayPlaying: gameSessionPhaseCode(model.gameSession) === 10 ? false : model.replayPlaying,
        replayClockGeneration: gameSessionPhaseCode(model.gameSession) === 10
          ? model.replayClockGeneration + 1
          : model.replayClockGeneration,
        status: `Analysisデータを取得できない: ${message.message}`
      }));
    case "flight-analysis-cursor-loaded":
      if (model.pendingAnalysisCursorRequestId !== message.requestId || ![7, 9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, { analysisCursorSample: message.sample, pendingAnalysisCursorRequestId: null }));
    case "flight-analysis-cursor-failed":
      if (model.pendingAnalysisCursorRequestId !== message.requestId || ![7, 9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        analysisCursorSample: null,
        pendingAnalysisCursorRequestId: null,
        status: `Analysis cursorを取得できない: ${message.message}`
      }));
    case "flight-replay-pose-loaded":
      if (model.pendingReplayPoseRequestId !== message.requestId || ![9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, { replayPose: message.pose, pendingReplayPoseRequestId: null }));
    case "flight-replay-pose-failed":
      if (model.pendingReplayPoseRequestId !== message.requestId || ![9, 10].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        replayPose: null,
        pendingReplayPoseRequestId: null,
        replayPlaying: gameSessionPhaseCode(model.gameSession) === 10 ? false : model.replayPlaying,
        replayClockGeneration: gameSessionPhaseCode(model.gameSession) === 10
          ? model.replayClockGeneration + 1
          : model.replayClockGeneration,
        status: `Replay poseを取得できない: ${message.message}`
      }));
    case "replay-clock-tick":
      return updateReplayClock(model, message);
    case "replay-clock-command-completed": {
      const phaseCode = gameSessionPhaseCode(model.gameSession);
      if (model.pendingReplayClockRequestId !== message.requestId
          || model.replayClockGeneration !== message.generation
          || ![9, 10].includes(phaseCode)) return transition(model);
      const speed = playbackRateForCode(message.state.rateCode);
      if (speed === null || !Number.isFinite(message.state.timeSeconds) || message.state.timeSeconds < 0) {
        return transition(withModel(model, {
          pendingReplayClockRequestId: null,
          replayPlaying: false,
          replayClockGeneration: model.replayClockGeneration + 1,
          status: "Rust Replay clock returned an invalid state"
        }));
      }
      const next = withModel(model, {
        pendingReplayClockRequestId: null,
        analysisCursorTimeSeconds: message.state.timeSeconds,
        replaySpeed: speed,
        replayPlaying: message.state.playing
      });
      const queried = model.flightAnalysis === null
        ? { model: next, effects: [] as AppEffect[] }
        : withReplayQuery(next, message.state.timeSeconds);
      if (message.state.playing && model.flightAnalysis !== null) {
        queried.effects.push({
          type: "schedule-replay-clock-tick",
          generation: message.generation,
          delayMilliseconds: 50
        });
      }
      return transition(queried.model, queried.effects);
    }
    case "replay-clock-command-failed":
      if (model.pendingReplayClockRequestId !== message.requestId
          || model.replayClockGeneration !== message.generation) return transition(model);
      return transition(withModel(model, {
        pendingReplayClockRequestId: null,
        replayPlaying: message.state?.playing ?? false,
        replaySpeed: message.state === undefined ? model.replaySpeed : playbackRateForCode(message.state.rateCode) ?? model.replaySpeed,
        analysisCursorTimeSeconds: message.state?.timeSeconds ?? model.analysisCursorTimeSeconds,
        replayClockGeneration: model.replayClockGeneration + 1,
        status: `Replay clock operation failed: ${message.message}`
      }));
    case "game-session-status":
      return transition(withModel(model, { status: message.message }));
    case "game-operation-failed": {
      if (model.pendingGameRequestId !== message.requestId) return transition(model);
      if (model.expectedLaunchRequestId === message.requestId) model = withModel(model, { expectedLaunchRequestId: null });
      if (message.currentSession !== undefined) {
        const completed = updateApp(model, {
          type: "game-operation-completed",
          requestId: message.requestId,
          ...message.currentSession
        });
        if (completed.model.status === "GameSessionから不正な状態snapshotを受信した") return completed;
        return transition(withModel(completed.model, { status: message.message }), completed.effects);
      }
      return transition(withModel(model, {
        pendingGameRequestId: null,
        status: message.message
      }));
    }
    case "page-hidden": {
      return transition(withModel(model, {
        expectedLaunchRequestId: null,
        flightRuntime: model.flightRuntime.kind === "active" ? Object.freeze({ kind: "inactive" }) : model.flightRuntime,
        presentation: Object.freeze({ type: "hidden" }),
        replayPlaying: false,
        replayClockGeneration: model.replayClockGeneration + 1,
        pendingReplayClockRequestId: null,
        status: "Page hidden"
      }), [
        { type: "cancel-pending-request", mode: "webxr" },
        { type: "cancel-pending-request", mode: "phone-vr" },
        { type: "dispose-presentation" }
      ]);
    }
    default:
      return assertNever(message);
  }
}

function updateUiAction(model: AppModel, action: UiAction): AppTransition {
  if (action.type === "activate" && action.controlId === flightRecoveryControlId(model) && model.pendingGameRequestId === null
      && (model.flightRuntime.kind === "stopped" || model.flightRuntime.kind === "projection-unavailable")) {
    const requestId = model.nextRequestId;
    return transition(withModel(model, { pendingGameRequestId: requestId, nextRequestId: requestId + 1, status: "飛行状態を確認して安全終了を要求している" }),
      [{ type: "recover-stopped-flight", launchRequestId: model.flightRuntime.launchRequestId, requestId }]);
  }
  if (action.type === "activate" && action.controlId.startsWith("flight-recovery-abort-")) return transition(model);
  if (model.flightRuntime.kind === "projection-unavailable" && isGameFlowActivation(action)) return transition(model);
  if (action.type === "activate" && action.controlId === "game-flight-resume"
      && (model.gameSession.kind !== "paused-flight" || !model.gameSession.canResume || model.flightRuntime.kind === "stopped")) {
    return transition(model);
  }
  if (action.type === "activate" && model.pendingGameRequestId === null && model.gameSession.kind === "paused-flight") {
    const overlays: Readonly<Record<string, PauseOverlayState>> = {
      "game-pause-open-settings": { kind: "settings" },
      "game-pause-open-help": { kind: "help" },
      "game-pause-settings-back": { kind: "menu" },
      "game-pause-help-back": { kind: "menu" }
    };
    const overlay = overlays[action.controlId];
    if (overlay !== undefined) {
      return transition(withModel(model, {
        gameSession: { ...model.gameSession, overlay },
        status: ""
      }));
    }
  }
  if (action.type === "activate" && action.controlId.startsWith("game-title-open-record-")
      && model.pendingGameRequestId !== null) return transition(model);
  if (action.type === "set-toggle") {
    const cueCode = informationCueCode(action.controlId);
    if (cueCode === null || gameSessionPhaseCode(model.gameSession) !== 1) return transition(model);
    return beginGameOperation(model, { kind: "set-information-cue", cueCode, visible: action.value });
  }
  if (action.type === "activate" && gameSessionPhaseCode(model.gameSession) === 0 && model.pendingGameRequestId === null) {
    const match = /^game-title-open-record-(\d+)$/.exec(action.controlId);
    const id = match === null ? null : Number(match[1]);
    if (id !== null && Number.isSafeInteger(id) && model.storedFlightRecords.some((record) => record.id === id)) {
      const requestId = model.nextRequestId;
      return transition(withModel(model, {
        pendingGameRequestId: requestId,
        nextRequestId: requestId + 1,
        status: `FlightRecord ${String(id)}を開いている`
      }), [{ type: "open-stored-flight-record", id, requestId }]);
    }
  }
  if (action.type === "set-range" && action.controlId === "game-replay-cursor"
      && gameSessionPhaseCode(model.gameSession) === 9 && model.flightAnalysis !== null && Number.isFinite(action.value)) {
    const timeSeconds = Math.min(model.flightAnalysis.summary.durationSeconds, Math.max(0, action.value));
    return beginReplayClockCommand(model, { kind: "seek", timeSeconds }, true);
  }
  if (action.type === "set-range" && action.controlId === "game-analysis-cursor"
      && gameSessionPhaseCode(model.gameSession) === 7 && model.flightAnalysis !== null && Number.isFinite(action.value)) {
    const timeSeconds = Math.min(model.flightAnalysis.summary.durationSeconds, Math.max(0, action.value));
    const requestId = model.nextAnalysisCursorRequestId;
    return transition(withModel(model, {
      analysisCursorTimeSeconds: timeSeconds,
      analysisCursorSample: null,
      pendingAnalysisCursorRequestId: requestId,
      nextAnalysisCursorRequestId: requestId + 1
    }), [{ type: "load-flight-analysis-cursor", requestId, timeSeconds }]);
  }
  if (action.type === "activate" && [7, 9].includes(gameSessionPhaseCode(model.gameSession))) {
    if (action.controlId === "game-result-open-analysis") {
      return transition(withModel(model, { resultTab: "analysis" }));
    }
    if (action.controlId === "game-result-open-summary") {
      return transition(withModel(model, { resultTab: "summary" }));
    }
    if (action.controlId === "game-analysis-map" && (gameSessionPhaseCode(model.gameSession) === 7 || model.replayViewMode === "analysis")) {
      return transition(withModel(model, { analysisChart: "map" }));
    }
    if (action.controlId === "game-analysis-altitude" && (gameSessionPhaseCode(model.gameSession) === 7 || model.replayViewMode === "analysis")) {
      return transition(withModel(model, { analysisChart: "altitude" }));
    }
    if (action.controlId === "game-analysis-speed" && (gameSessionPhaseCode(model.gameSession) === 7 || model.replayViewMode === "analysis")) {
      return transition(withModel(model, { analysisChart: "speed" }));
    }
  }
  if (action.type === "activate" && gameSessionPhaseCode(model.gameSession) === 9) {
    if (action.controlId === "game-replay-view-mode") {
      const nextMode = model.replayViewMode === "cinematic"
        ? "telemetry"
        : model.replayViewMode === "telemetry" ? "analysis" : "cinematic";
      return transition(withModel(model, { replayViewMode: nextMode }));
    }
    if (action.controlId === "game-replay-camera") {
      if (model.presentation.type !== "ready") return transition(model);
      const cameraModes: readonly AppModel["replayCameraMode"][] = [
        "auto", "pilot", "chase", "orbit", "platform", "shore", "overhead", "side", "front", "telephoto"
      ];
      const nextCameraMode = cameraModes[(cameraModes.indexOf(model.replayCameraMode) + 1) % cameraModes.length] ?? "auto";
      return transition(withModel(model, { replayCameraMode: nextCameraMode }));
    }
    if (action.controlId === "game-replay-play-pause") {
      if (model.flightAnalysis === null) return transition(model);
      return beginReplayClockCommand(model, { kind: model.replayPlaying ? "pause" : "play" }, true);
    }
    if (action.controlId === "game-replay-speed-0_5") return beginReplayClockCommand(model, { kind: "rate", rateCode: 0 }, true);
    if (action.controlId === "game-replay-speed-1") return beginReplayClockCommand(model, { kind: "rate", rateCode: 1 }, true);
    if (action.controlId === "game-replay-speed-2") return beginReplayClockCommand(model, { kind: "rate", rateCode: 2 }, true);
  }
  if (action.type === "focus" || action.type === "back" || action.type === "scroll") return transition(model);
  if (action.type === "recenter-menu") {
    if (isInVr(model.presentation)) {
      return transition(withModel(model, { status: "Menu placement updated" }), [{ type: "recenter-menu" }]);
    }
    return transition(model);
  }
  if (action.type !== "activate") {
    return transition(withModel(model, { status: `Action ${action.type} is unavailable in ${model.gameSession.kind}` }));
  }
  const gameOperation = operationForGameAction(gameSessionPhaseCode(model.gameSession), action.controlId);
  if (gameOperation !== null) return beginGameOperation(model, gameOperation);
  if (action.controlId === "boot-enter-webxr") {
    return beginPermissionRequest(model, "webxr");
  }
  if (action.controlId === "boot-enter-phone-vr") {
    return beginPermissionRequest(model, "phone-vr");
  }
  if (action.controlId === "boot-exit-vr") {
    return beginBackendSwitch(model, "screen", "Ending presentation session");
  }
  if (action.controlId === "boot-recenter-phone-tracking") {
    if (model.presentation.type !== "ready" || model.presentation.mode !== "phone-vr") return transition(model);
    return transition(withModel(model, { status: "Phone VR tracking reference updated" }), [{ type: "recenter-tracking" }]);
  }
  if (action.controlId === "boot-recenter-menu") {
    if (!isInVr(model.presentation)) return transition(model);
    return transition(withModel(model, { status: "Menu placement updated" }), [{ type: "recenter-menu" }]);
  }
  return transition(withModel(model, { status: `Action ${action.controlId} is unavailable in ${model.gameSession.kind}` }));
}

function updateReplayClock(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "replay-clock-tick" }>
): AppTransition {
  const phaseCode = gameSessionPhaseCode(model.gameSession);
  if (!model.replayPlaying || ![9, 10].includes(phaseCode) || message.generation !== model.replayClockGeneration
      || model.flightAnalysis === null || !Number.isFinite(message.elapsedSeconds) || message.elapsedSeconds < 0) {
    return transition(model);
  }
  return beginReplayClockCommand(model, { kind: "advance", elapsedSeconds: message.elapsedSeconds }, false);
}

function beginReplayClockCommand(
  model: AppModel,
  command: ReplayClockCommand,
  restartSchedule: boolean
): AppTransition {
  if (model.pendingReplayClockRequestId !== null) return transition(model);
  const requestId = model.nextReplayClockRequestId;
  const generation = restartSchedule ? model.replayClockGeneration + 1 : model.replayClockGeneration;
  return transition(withModel(model, {
    pendingReplayClockRequestId: requestId,
    nextReplayClockRequestId: requestId + 1,
    replayClockGeneration: generation
  }), [{ type: "control-replay-clock", requestId, generation, command }]);
}

function playbackRateForCode(code: number): AppModel["replaySpeed"] | null {
  switch (code) {
    case 0: return 0.5;
    case 1: return 1;
    case 2: return 2;
    default: return null;
  }
}

function withReplayQuery(model: AppModel, timeSeconds: number): { readonly model: AppModel; readonly effects: AppEffect[] } {
  const replayRequestId = model.nextReplayPoseRequestId;
  const cursorRequestId = model.nextAnalysisCursorRequestId;
  model = withModel(model, {
    pendingReplayPoseRequestId: replayRequestId,
    nextReplayPoseRequestId: replayRequestId + 1,
    pendingAnalysisCursorRequestId: cursorRequestId,
    nextAnalysisCursorRequestId: cursorRequestId + 1
  });
  return {
    model,
    effects: [
      { type: "load-flight-replay-pose", requestId: replayRequestId, timeSeconds },
      { type: "load-flight-analysis-cursor", requestId: cursorRequestId, timeSeconds }
    ]
  };
}

function operationForGameAction(phaseCode: number, controlId: string): NamedGameSessionOperation | null {
  if (controlId === "game-briefing-cancel" && (phaseCode === 2 || phaseCode === 3)) {
    return "cancel-briefing";
  }
  const operations: Readonly<Record<string, readonly [number, NamedGameSessionOperation]>> = {
    "game-title-start": [0, "open-setup"],
    "game-setup-start": [1, "prepare"],
    "game-setup-mode-manual": [1, "set-control-manual"],
    "game-setup-mode-shared": [1, "set-control-shared"],
    "game-setup-mode-automatic": [1, "set-control-automatic"],
    "game-setup-preset": [1, "cycle-difficulty-preset"],
    "game-setup-information": [1, "cycle-information-level"],
    "game-setup-assistance": [1, "cycle-assistance-level"],
    "game-setup-weather": [1, "cycle-weather-class"],
    "game-setup-back": [1, "return-to-title"],
    "game-briefing-start": [3, "start-flight"],
    "game-countdown-cancel": [4, "cancel-countdown"],
    "game-flight-pause": [5, "pause"],
    "game-flight-abort": [5, "abort"],
    "game-flight-resume": [6, "resume"],
    "game-paused-abort": [6, "abort"],
    "game-result-retry": [7, "retry"],
    "game-result-replay": [7, "enter-replay"],
    "game-replay-return": [9, "leave-replay"],
    "game-title-demo": [0, "enter-attract"],
    "game-attract-return": [10, "leave-attract"],
    "game-result-setup": [7, "open-setup"],
    "game-result-title": [7, "return-to-title"],
    "game-briefing-retry": [8, "retry-briefing"],
    "game-failed-setup": [8, "cancel-briefing"]
  };
  const entry = operations[controlId];
  return entry !== undefined && entry[0] === phaseCode ? entry[1] : null;
}

function informationCueCode(controlId: string): number | null {
  const cueCodes: Readonly<Record<string, number>> = {
    "game-setup-information-telemetry": 0,
    "game-setup-information-attitude": 1,
    "game-setup-information-wind": 2,
    "game-setup-information-flight-path": 3,
    "game-setup-information-angle-of-attack": 4,
    "game-setup-information-warnings": 5
  };
  return cueCodes[controlId] ?? null;
}

function beginGameOperation(model: AppModel, operation: GameSessionOperation): AppTransition {
  if (model.pendingGameRequestId !== null) return transition(model);
  const requestId = model.nextRequestId;
  return transition(withModel(model, {
    pendingGameRequestId: requestId,
    expectedLaunchRequestId: operation === "start-flight" ? requestId : operation === "cancel-countdown" ? null : model.expectedLaunchRequestId,
    nextRequestId: requestId + 1,
    status: "ゲーム状態を更新している"
  }), [{ type: "game-session-operation", operation, requestId }]);
}

function beginStoredFlightRecordLoad(model: AppModel): AppTransition {
  const requestId = model.nextRecordListRequestId;
  return transition(withModel(model, {
    pendingRecordListRequestId: requestId,
    nextRecordListRequestId: requestId + 1,
    storedFlightRecordsStatus: "保存記録を読み込んでいる"
  }), [{ type: "load-stored-flight-records", requestId }]);
}

function entersResult(previousPhaseCode: number, nextPhaseCode: number): boolean {
  return previousPhaseCode !== 7 && nextPhaseCode === 7;
}

function beginPermissionRequest(model: AppModel, mode: "webxr" | "phone-vr"): AppTransition {
  const available = mode === "webxr" ? model.webXrAvailable : model.phoneVrAvailable;
  if (!available || !canRequestPresentation(model.presentation)) return transition(model);
  const requestId = model.nextRequestId;
  const from = model.presentation.type === "ready" ? model.presentation.mode : null;
  const status = mode === "webxr" ? "Waiting for WebXR permission" : "Waiting for phone orientation permission";
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", origin: "user-request", requestId, from, to: mode, phase: "requesting" }),
    nextRequestId: requestId + 1,
    status
  }), [{ type: "request-permission", mode, requestId }]);
}

function beginBackendSwitch(model: AppModel, mode: PresentationMode, status: string): AppTransition {
  if (model.presentation.type !== "ready" || model.presentation.mode === mode) return transition(model);
  const requestId = model.nextRequestId;
  const from = model.presentation.mode;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", origin: "user-request", requestId, from, to: mode, phase: "stopping" }),
    nextRequestId: requestId + 1,
    status
  }), [{ type: "switch-backend", mode, requestId }]);
}

function updatePermissionCompletion(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "permission-completed" }>
): AppTransition {
  const current = model.presentation;
  if (current.type !== "transitioning" || current.requestId !== message.requestId || current.to !== message.mode || current.phase !== "requesting") {
    return transition(model, message.ok ? [{ type: "cancel-pending-request", mode: message.mode }] : []);
  }
  if (!message.ok) {
    const status = message.message;
    const presentation = current.from === null
      ? Object.freeze({ type: "failed" as const, message: status })
      : Object.freeze({ type: "ready" as const, mode: current.from });
    return transition(withModel(model, { presentation, status }));
  }
  return transition(withModel(model, {
    presentation: Object.freeze({ ...current, phase: "starting" }),
    status: message.mode === "webxr" ? "Starting WebXR presentation" : "Starting Phone VR presentation"
  }), [{ type: "switch-backend", mode: message.mode, requestId: message.requestId }]);
}

function updateBackendCompletion(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "backend-transition-completed" }>
): AppTransition {
  const current = model.presentation;
  if (current.type !== "transitioning" || current.phase === "requesting"
      || current.requestId !== message.requestId || current.to !== message.requestedMode) {
    return transition(model);
  }
  const causePrefix = current.origin === "backend-fault" ? `${current.cause}; ` : "";
  if (message.activeMode !== null) {
    const outcome = message.ok
      ? message.activeMode === message.requestedMode
        ? message.successStatus
        : `Backend ${message.requestedMode} reported success while ${labelForMode(message.activeMode)} is active`
      : `${message.message}; ${labelForMode(message.activeMode)} is active`;
    return transition(withModel(model, {
      presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
      status: `${causePrefix}${outcome}`
    }));
  }
  const outcome = message.ok
    ? `Backend ${message.requestedMode} reported success without an active backend`
    : message.message;
  const status = `${causePrefix}${outcome}`;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "failed", message: status }),
    status
  }));
}

function beginScreenRecovery(model: AppModel, origin: ScreenRecoveryOrigin): AppTransition {
  const requestId = model.nextRequestId;
  const message = origin.origin === "backend-fault" ? origin.cause : "Page restored";
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", ...origin, requestId, to: "screen", phase: "stopping" }),
    nextRequestId: requestId + 1,
    status: `${message}; restoring Screen`
  }), [{ type: "switch-backend", mode: "screen", requestId }]);
}

function canRequestPresentation(state: PresentationUiState): boolean {
  return state.type === "failed" || (state.type === "ready" && state.mode === "screen");
}

function isInVr(state: PresentationUiState): boolean {
  return state.type === "ready" && state.mode !== "screen";
}

function labelForMode(mode: PresentationMode): string {
  switch (mode) {
    case "screen": return "Screen";
    case "webxr": return "WebXR";
    case "phone-vr": return "Phone VR";
  }
}

function withModel(model: AppModel, changes: Partial<AppModel>): AppModel {
  return Object.freeze({ ...model, ...changes });
}

function fullHudProfile(): HudProfileUiState {
  return Object.freeze({
    telemetry: true,
    attitude: true,
    wind: true,
    flightPath: true,
    angleOfAttack: true,
    warnings: true
  });
}

function transition(model: AppModel, effects: readonly AppEffect[] = []): AppTransition {
  return Object.freeze({ model, effects: Object.freeze([...effects]) });
}

function assertNever(value: never): never {
  throw new Error(`Unhandled application message: ${JSON.stringify(value)}`);
}
