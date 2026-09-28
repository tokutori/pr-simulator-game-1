import type { FlightCameraMode, FlightRenderPose, PresentationMode } from "../render/contracts/runtime.js";
import type { UiAction } from "../render/contracts/ui.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightAnalysisData, FlightAnalysisSample } from "../game/flight-record-query.js";

export interface StoredFlightRecordUiEntry {
  readonly id: number;
  readonly savedAt: string;
}

export type GameSessionOperation =
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
  | "leave-replay";

export type PresentationUiState =
  | { readonly type: "uninitialized" }
  | { readonly type: "initializing"; readonly requestId: number }
  | { readonly type: "ready"; readonly mode: PresentationMode }
  | {
      readonly type: "transitioning";
      readonly requestId: number;
      readonly from: PresentationMode | null;
      readonly to: PresentationMode;
      readonly phase: "requesting" | "stopping" | "starting";
    }
  | { readonly type: "failed"; readonly message: string }
  | { readonly type: "hidden" };

export type GameSessionUiState =
  | { readonly kind: "boot"; readonly phaseCode: -1 }
  | { readonly kind: "title"; readonly phaseCode: 0 }
  | { readonly kind: "setup"; readonly phaseCode: 1 }
  | { readonly kind: "briefing-preparing"; readonly phaseCode: 2 }
  | { readonly kind: "briefing-ready"; readonly phaseCode: 3 }
  | { readonly kind: "countdown"; readonly phaseCode: 4; readonly countdownRemaining: number }
  | { readonly kind: "flight"; readonly phaseCode: 5 | 6; readonly snapshot: FlightSnapshot }
  | { readonly kind: "result"; readonly phaseCode: 7; readonly snapshot: FlightSnapshot | null }
  | { readonly kind: "briefing-failed"; readonly phaseCode: 8 }
  | { readonly kind: "replay"; readonly phaseCode: 9; readonly snapshot: FlightSnapshot | null };

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
  readonly replaySpeed: 0.5 | 1 | 2;
  readonly replayClockGeneration: number;
  readonly replayCameraMode: "auto" | FlightCameraMode;
  readonly storedFlightRecords: readonly StoredFlightRecordUiEntry[];
  readonly storedFlightRecordsStatus: string;
  readonly pendingRecordListRequestId: number | null;
  readonly nextRecordListRequestId: number;
  readonly pendingGameRequestId: number | null;
  readonly nextRequestId: number;
}

export interface DifficultyUiState {
  readonly presetCode: number;
  readonly informationCode: number;
  readonly assistanceCode: number;
  readonly weatherCode: number;
}

export interface ConfigurationMetadataUiState {
  readonly presetCode: number;
  readonly informationCode: number;
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
  | {
      readonly type: "game-session-synced";
      readonly phaseCode: number;
      readonly controlModeCode: number;
      readonly difficulty: DifficultyUiState;
      readonly configurationMetadata: ConfigurationMetadataUiState | null;
      readonly countdownRemaining: number;
      readonly snapshot: FlightSnapshot | null;
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
    }
  | { readonly type: "game-session-status"; readonly message: string }
  | { readonly type: "flight-analysis-loaded"; readonly requestId: number; readonly data: FlightAnalysisData }
  | { readonly type: "flight-analysis-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-analysis-cursor-loaded"; readonly requestId: number; readonly sample: FlightAnalysisSample }
  | { readonly type: "flight-analysis-cursor-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-replay-pose-loaded"; readonly requestId: number; readonly pose: FlightRenderPose }
  | { readonly type: "flight-replay-pose-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "replay-clock-tick"; readonly generation: number; readonly elapsedSeconds: number }
  | { readonly type: "game-operation-failed"; readonly requestId: number; readonly message: string };

export type AppEffect =
  | { readonly type: "initialize-presentation"; readonly requestId: number }
  | { readonly type: "request-permission"; readonly mode: "webxr" | "phone-vr"; readonly requestId: number }
  | { readonly type: "switch-backend"; readonly mode: PresentationMode; readonly requestId: number }
  | { readonly type: "cancel-pending-request"; readonly mode: "webxr" | "phone-vr" }
  | { readonly type: "recenter-tracking" }
  | { readonly type: "recenter-menu" }
  | { readonly type: "dispose-presentation" }
  | { readonly type: "persist-flight-record" }
  | { readonly type: "load-stored-flight-records"; readonly requestId: number }
  | { readonly type: "open-stored-flight-record"; readonly id: number; readonly requestId: number }
  | { readonly type: "load-flight-analysis"; readonly requestId: number }
  | { readonly type: "load-flight-analysis-cursor"; readonly requestId: number; readonly timeSeconds: number }
  | { readonly type: "load-flight-replay-pose"; readonly requestId: number; readonly timeSeconds: number }
  | { readonly type: "schedule-replay-clock-tick"; readonly generation: number; readonly delayMilliseconds: number }
  | { readonly type: "game-session-operation"; readonly operation: GameSessionOperation; readonly requestId: number };

export interface AppTransition {
  readonly model: AppModel;
  readonly effects: readonly AppEffect[];
}

export function gameSessionPhaseCode(session: GameSessionUiState): number {
  return session.phaseCode;
}

export function gameSessionSnapshot(session: GameSessionUiState): FlightSnapshot | null {
  return "snapshot" in session ? session.snapshot : null;
}

export function gameSessionCountdown(session: GameSessionUiState): number {
  return session.kind === "countdown" ? session.countdownRemaining : 0;
}

export function gameSessionState(
  phaseCode: number,
  countdownRemaining: number,
  snapshot: FlightSnapshot | null
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
    case 6:
      return snapshot === null ? null : { kind: "flight", phaseCode, snapshot };
    case 7: return { kind: "result", phaseCode: 7, snapshot };
    case 8: return { kind: "briefing-failed", phaseCode: 8 };
    case 9: return { kind: "replay", phaseCode: 9, snapshot };
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
    difficulty: Object.freeze({ presetCode: 4, informationCode: 0, assistanceCode: 3, weatherCode: 0 }),
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
    replaySpeed: 1,
    replayClockGeneration: 0,
    replayCameraMode: "auto",
    storedFlightRecords: Object.freeze([]),
    storedFlightRecordsStatus: "保存記録を読み込んでいる",
    pendingRecordListRequestId: null,
    nextRecordListRequestId: 1,
    pendingGameRequestId: null,
    nextRequestId: 1
  });
}

export function updateApp(model: AppModel, message: AppMessage): AppTransition {
  if (model.presentation.type === "hidden") {
    if (message.type === "permission-completed" && message.ok) {
      return transition(model, [{ type: "cancel-pending-request", mode: message.mode }]);
    }
    return transition(model);
  }

  switch (message.type) {
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
      return beginScreenRecovery(model, message.message, message.mode);
    }
    case "game-session-synced": {
      const gameSession = gameSessionState(message.phaseCode, message.countdownRemaining, message.snapshot);
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
        controlModeCode: message.controlModeCode,
        difficulty: message.difficulty,
        configurationMetadata: message.configurationMetadata,
        flightAnalysis: nextPhaseCode === 7 || nextPhaseCode === 9 ? model.flightAnalysis : null,
        pendingAnalysisRequestId: analysisRequestId,
        resultTab: enteringResult ? "summary" : model.resultTab,
        analysisChart: enteringResult ? "map" : model.analysisChart,
        analysisCursorTimeSeconds: enteringResult ? 0 : model.analysisCursorTimeSeconds,
        analysisCursorSample: [7, 9].includes(nextPhaseCode) && !enteringResult ? model.analysisCursorSample : null,
        pendingAnalysisCursorRequestId: [7, 9].includes(nextPhaseCode) && !enteringResult
          ? model.pendingAnalysisCursorRequestId
          : null,
        nextAnalysisRequestId: enteringResult ? model.nextAnalysisRequestId + 1 : model.nextAnalysisRequestId,
        replayPlaying: nextPhaseCode === 9 ? model.replayPlaying : false,
        replayClockGeneration: nextPhaseCode === 9 ? model.replayClockGeneration : model.replayClockGeneration + 1
      }), effects);
    }
    case "game-operation-completed": {
      const gameSession = gameSessionState(message.phaseCode, message.countdownRemaining, message.snapshot);
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
      const analysisRequestId = enteringResult || (enteringReplay && model.flightAnalysis === null)
        ? model.nextAnalysisRequestId
        : null;
      const replayPoseRequestId = enteringReplay && model.flightAnalysis !== null
        ? model.nextReplayPoseRequestId
        : null;
      const effects: AppEffect[] = enteringResult && analysisRequestId !== null
        ? [{ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: analysisRequestId }]
        : enteringReplay && analysisRequestId !== null
          ? [{ type: "load-flight-analysis", requestId: analysisRequestId }]
          : enteringReplay && replayPoseRequestId !== null
            ? [{ type: "load-flight-replay-pose", requestId: replayPoseRequestId, timeSeconds: model.analysisCursorTimeSeconds }]
          : [];
      return transition(withModel(model, {
        pendingGameRequestId: null,
        gameSession: Object.freeze(gameSession),
        controlModeCode: message.controlModeCode,
        difficulty: message.difficulty,
        configurationMetadata: message.configurationMetadata,
        status: "",
        flightAnalysis: nextPhaseCode === 7 || nextPhaseCode === 9 ? model.flightAnalysis : null,
        pendingAnalysisRequestId: analysisRequestId,
        resultTab: enteringResult ? "summary" : model.resultTab,
        analysisChart: enteringResult ? "map" : model.analysisChart,
        analysisCursorTimeSeconds: enteringResult ? 0 : model.analysisCursorTimeSeconds,
        analysisCursorSample: [7, 9].includes(nextPhaseCode) && !enteringResult ? model.analysisCursorSample : null,
        pendingAnalysisCursorRequestId: [7, 9].includes(nextPhaseCode) && !enteringResult
          ? model.pendingAnalysisCursorRequestId
          : null,
        nextAnalysisRequestId: analysisRequestId === null ? model.nextAnalysisRequestId : analysisRequestId + 1,
        replayPose: enteringReplay ? null : nextPhaseCode === 9 ? model.replayPose : null,
        pendingReplayPoseRequestId: replayPoseRequestId,
        nextReplayPoseRequestId: replayPoseRequestId === null ? model.nextReplayPoseRequestId : replayPoseRequestId + 1,
        replayPlaying: nextPhaseCode === 9 ? enteringReplay ? false : model.replayPlaying : false,
        replayClockGeneration: nextPhaseCode === 9 ? model.replayClockGeneration : model.replayClockGeneration + 1
      }), effects);
    }
    case "flight-analysis-loaded":
      if (model.pendingAnalysisRequestId !== message.requestId || ![7, 9].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      {
        const requestId = model.nextAnalysisCursorRequestId;
        const replayPoseRequestId = gameSessionPhaseCode(model.gameSession) === 9 ? model.nextReplayPoseRequestId : null;
        const effects: AppEffect[] = [{ type: "load-flight-analysis-cursor", requestId, timeSeconds: 0 }];
        if (replayPoseRequestId !== null) {
          effects.push({ type: "load-flight-replay-pose", requestId: replayPoseRequestId, timeSeconds: 0 });
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
      if (model.pendingAnalysisRequestId !== message.requestId || ![7, 9].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        flightAnalysis: null,
        pendingAnalysisRequestId: null,
        pendingAnalysisCursorRequestId: null,
        status: `Analysisデータを取得できない: ${message.message}`
      }));
    case "flight-analysis-cursor-loaded":
      if (model.pendingAnalysisCursorRequestId !== message.requestId || ![7, 9].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, { analysisCursorSample: message.sample, pendingAnalysisCursorRequestId: null }));
    case "flight-analysis-cursor-failed":
      if (model.pendingAnalysisCursorRequestId !== message.requestId || ![7, 9].includes(gameSessionPhaseCode(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        analysisCursorSample: null,
        pendingAnalysisCursorRequestId: null,
        status: `Analysis cursorを取得できない: ${message.message}`
      }));
    case "flight-replay-pose-loaded":
      if (model.pendingReplayPoseRequestId !== message.requestId || gameSessionPhaseCode(model.gameSession) !== 9) return transition(model);
      return transition(withModel(model, { replayPose: message.pose, pendingReplayPoseRequestId: null }));
    case "flight-replay-pose-failed":
      if (model.pendingReplayPoseRequestId !== message.requestId || gameSessionPhaseCode(model.gameSession) !== 9) return transition(model);
      return transition(withModel(model, {
        replayPose: null,
        pendingReplayPoseRequestId: null,
        status: `Replay poseを取得できない: ${message.message}`
      }));
    case "replay-clock-tick":
      return updateReplayClock(model, message);
    case "game-session-status":
      return transition(withModel(model, { status: message.message }));
    case "game-operation-failed": {
      if (model.pendingGameRequestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        pendingGameRequestId: null,
        status: message.message
      }));
    }
    case "page-hidden": {
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "hidden" }),
        replayPlaying: false,
        replayClockGeneration: model.replayClockGeneration + 1,
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
  if (action.type === "activate" && action.controlId.startsWith("game-title-open-record-")
      && model.pendingGameRequestId !== null) return transition(model);
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
    const replayRequestId = model.nextReplayPoseRequestId;
    const cursorRequestId = model.nextAnalysisCursorRequestId;
    return transition(withModel(model, {
      analysisCursorTimeSeconds: timeSeconds,
      pendingReplayPoseRequestId: replayRequestId,
      nextReplayPoseRequestId: replayRequestId + 1,
      pendingAnalysisCursorRequestId: cursorRequestId,
      nextAnalysisCursorRequestId: cursorRequestId + 1
    }), [
      { type: "load-flight-replay-pose", requestId: replayRequestId, timeSeconds },
      { type: "load-flight-analysis-cursor", requestId: cursorRequestId, timeSeconds }
    ]);
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
  if (action.type === "activate" && gameSessionPhaseCode(model.gameSession) === 7) {
    if (action.controlId === "game-result-open-analysis") {
      return transition(withModel(model, { resultTab: "analysis" }));
    }
    if (action.controlId === "game-result-open-summary") {
      return transition(withModel(model, { resultTab: "summary" }));
    }
    if (action.controlId === "game-analysis-map") {
      return transition(withModel(model, { analysisChart: "map" }));
    }
    if (action.controlId === "game-analysis-altitude") {
      return transition(withModel(model, { analysisChart: "altitude" }));
    }
    if (action.controlId === "game-analysis-speed") {
      return transition(withModel(model, { analysisChart: "speed" }));
    }
  }
  if (action.type === "activate" && gameSessionPhaseCode(model.gameSession) === 9) {
    if (action.controlId === "game-replay-camera") {
      if (model.presentation.type !== "ready" || model.presentation.mode !== "screen") return transition(model);
      const nextCameraMode = model.replayCameraMode === "auto"
        ? "pilot"
        : model.replayCameraMode === "pilot"
          ? "chase"
          : "auto";
      return transition(withModel(model, { replayCameraMode: nextCameraMode }));
    }
    if (action.controlId === "game-replay-play-pause") {
      if (model.flightAnalysis === null) return transition(model);
      if (model.replayPlaying) {
        return transition(withModel(model, {
          replayPlaying: false,
          replayClockGeneration: model.replayClockGeneration + 1
        }));
      }
      const durationSeconds = model.flightAnalysis.summary.durationSeconds;
      const restart = model.analysisCursorTimeSeconds >= durationSeconds;
      const generation = model.replayClockGeneration + 1;
      const startTime = restart ? 0 : model.analysisCursorTimeSeconds;
      const base = withModel(model, {
        replayPlaying: true,
        replayClockGeneration: generation,
        analysisCursorTimeSeconds: startTime
      });
      const effects: AppEffect[] = [{ type: "schedule-replay-clock-tick", generation, delayMilliseconds: 50 }];
      const queried = restart ? withReplayQuery(base, startTime) : { model: base, effects: [] };
      effects.push(...queried.effects);
      return transition(queried.model, effects);
    }
    if (action.controlId === "game-replay-speed-0_5") return transition(withModel(model, { replaySpeed: 0.5 }));
    if (action.controlId === "game-replay-speed-1") return transition(withModel(model, { replaySpeed: 1 }));
    if (action.controlId === "game-replay-speed-2") return transition(withModel(model, { replaySpeed: 2 }));
  }
  if (action.type === "focus" || action.type === "back" || action.type === "scroll") return transition(model);
  if (action.type === "recenter-menu") {
    if (isInVr(model.presentation)) {
      return transition(withModel(model, { status: "Menu placement updated" }), [{ type: "recenter-menu" }]);
    }
    return transition(model);
  }
  if (action.type !== "activate") {
    return transition(withModel(model, { status: `Action ${action.type} is unavailable in Boot` }));
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
  return transition(withModel(model, { status: `Action ${action.controlId} is unavailable in Boot` }));
}

function updateReplayClock(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "replay-clock-tick" }>
): AppTransition {
  if (!model.replayPlaying || gameSessionPhaseCode(model.gameSession) !== 9 || message.generation !== model.replayClockGeneration
      || model.flightAnalysis === null || !Number.isFinite(message.elapsedSeconds) || message.elapsedSeconds < 0) {
    return transition(model);
  }
  const durationSeconds = model.flightAnalysis.summary.durationSeconds;
  const timeSeconds = Math.min(durationSeconds, model.analysisCursorTimeSeconds + message.elapsedSeconds * model.replaySpeed);
  const reachedEnd = timeSeconds >= durationSeconds;
  const generation = reachedEnd ? model.replayClockGeneration + 1 : model.replayClockGeneration;
  const next = withModel(model, {
    analysisCursorTimeSeconds: timeSeconds,
    replayPlaying: !reachedEnd,
    replayClockGeneration: generation
  });
  const queried = withReplayQuery(next, timeSeconds);
  const effects = queried.effects;
  if (!reachedEnd) effects.push({ type: "schedule-replay-clock-tick", generation, delayMilliseconds: 50 });
  return transition(queried.model, effects);
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

function operationForGameAction(phaseCode: number, controlId: string): GameSessionOperation | null {
  const operations: Readonly<Record<string, readonly [number, GameSessionOperation]>> = {
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
    "game-briefing-cancel": [2, "cancel-briefing"],
    "game-briefing-start": [3, "start-flight"],
    "game-countdown-cancel": [4, "cancel-countdown"],
    "game-flight-pause": [5, "pause"],
    "game-flight-abort": [5, "abort"],
    "game-flight-resume": [6, "resume"],
    "game-paused-abort": [6, "abort"],
    "game-result-retry": [7, "retry"],
    "game-result-replay": [7, "enter-replay"],
    "game-replay-return": [9, "leave-replay"],
    "game-result-setup": [7, "open-setup"],
    "game-result-title": [7, "return-to-title"],
    "game-briefing-retry": [8, "retry-briefing"],
    "game-failed-setup": [8, "cancel-briefing"]
  };
  const entry = operations[controlId];
  return entry !== undefined && entry[0] === phaseCode ? entry[1] : null;
}

function beginGameOperation(model: AppModel, operation: GameSessionOperation): AppTransition {
  if (model.pendingGameRequestId !== null) return transition(model);
  const requestId = model.nextRequestId;
  return transition(withModel(model, {
    pendingGameRequestId: requestId,
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
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: mode, phase: "requesting" }),
    nextRequestId: requestId + 1,
    status
  }), [{ type: "request-permission", mode, requestId }]);
}

function beginBackendSwitch(model: AppModel, mode: PresentationMode, status: string): AppTransition {
  if (model.presentation.type !== "ready" || model.presentation.mode === mode) return transition(model);
  const requestId = model.nextRequestId;
  const from = model.presentation.mode;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: mode, phase: "stopping" }),
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
  if (current.type !== "transitioning" || current.requestId !== message.requestId || current.to !== message.requestedMode) {
    return transition(model);
  }
  if (message.activeMode !== null) {
    const status = message.ok && message.activeMode === message.requestedMode
      ? message.successStatus
      : `${message.message}; ${labelForMode(message.activeMode)} is active`;
    return transition(withModel(model, {
      presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
      status
    }));
  }
  const status = message.ok
    ? `Backend ${message.requestedMode} reported success without an active backend`
    : message.message;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "failed", message: status }),
    status
  }));
}

function beginScreenRecovery(
  model: AppModel,
  message: string,
  from: "webxr" | "phone-vr"
): AppTransition {
  const requestId = model.nextRequestId;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: "screen", phase: "stopping" }),
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

function transition(model: AppModel, effects: readonly AppEffect[] = []): AppTransition {
  return Object.freeze({ model, effects: Object.freeze([...effects]) });
}

function assertNever(value: never): never {
  throw new Error(`Unhandled application message: ${JSON.stringify(value)}`);
}
