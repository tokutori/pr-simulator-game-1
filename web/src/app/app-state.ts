import type { GamePhase } from "./session-snapshot.js";
import type { FlightCameraMode, LakeWaterQualityCleanupResult, PresentationMode, RecordedFlightRenderPose } from "../render/contracts/runtime.js";
import type { LakeWaterQuality } from "../render/contracts/lake-water.js";
import type { UiAction } from "../render/contracts/ui.js";
import type { MenuScrollContext, MenuScrollIntent, MenuScrollScope, MenuScrollState } from "../render/contracts/menu-layout.js";
import type { FlightSnapshotInput } from "./session-snapshot.js";
import { isRetainedLiveSnapshot } from "./session-snapshot.js";
import type { GameSessionProjection, LiveSessionSnapshot, TailGameSessionProjection } from "./session-snapshot.js";
import type { FlightAnalysisInput, FlightAnalysisCursorInput } from "../game/flight-analysis-view.js";
import { analysisCursorMatches } from "../game/flight-analysis-view.js";
import type { GameUiView } from "../generated/game-ui-view.js";
import type { GameUiAction } from "../generated/game-ui-action.js";
import type { GameUiEffect } from "../generated/game-ui-effects.js";
import type { GameUiPlatformInput } from "../generated/game-ui-input.js";
import type { SessionCommand } from "./game-ui-types.js";
import { sharedGameUiAction } from "./game-view.js";

export interface StoredFlightRecordUiEntry {
  readonly id: number;
  readonly savedAt: string;
  readonly personalBest: boolean;
}

export type RecordListState = { readonly nextRequestId: number } & (
  | { readonly kind: "unloaded" }
  | { readonly kind: "loading"; readonly requestId: number; readonly retained: readonly StoredFlightRecordUiEntry[] }
  | { readonly kind: "ready"; readonly records: readonly StoredFlightRecordUiEntry[] }
  | { readonly kind: "failed"; readonly message: string; readonly retained: readonly StoredFlightRecordUiEntry[] }
);

const EMPTY_STORED_FLIGHT_RECORDS: readonly StoredFlightRecordUiEntry[] = Object.freeze([]);

export type FlightAnalysisState = { readonly nextRequestId: number } & (
  | { readonly kind: "idle" }
  | { readonly kind: "loading"; readonly requestId: number; readonly retained: FlightAnalysisInput | null }
  | { readonly kind: "ready"; readonly data: FlightAnalysisInput }
  | { readonly kind: "failed"; readonly message: string; readonly retained: FlightAnalysisInput | null }
);

export type AnalysisCursorState = { readonly nextRequestId: number } & (
  | { readonly kind: "idle" }
  | { readonly kind: "loading"; readonly requestId: number; readonly retained: FlightAnalysisCursorInput | null }
  | { readonly kind: "ready"; readonly sample: FlightAnalysisCursorInput }
  | { readonly kind: "failed"; readonly message: string; readonly retained: FlightAnalysisCursorInput | null }
);

export interface ReplayClockState {
  readonly timeSeconds: number;
  readonly rateCode: 0 | 1 | 2;
  readonly playing: boolean;
}

export type ReplayPoseQueryState = { readonly nextRequestId: number } & (
  | { readonly kind: "idle" }
  | { readonly kind: "loading"; readonly requestId: number; readonly retained: RecordedFlightRenderPose | null }
  | { readonly kind: "ready"; readonly pose: RecordedFlightRenderPose }
  | { readonly kind: "failed"; readonly message: string }
);

export type ReplayClockRequestState = {
  readonly generation: number;
  readonly nextRequestId: number;
  readonly observation: {
    readonly timeSeconds: number;
    readonly speed: 0.5 | 1 | 2;
    readonly playing: boolean;
  };
} & (
  | { readonly kind: "settled" }
  | { readonly kind: "requesting"; readonly requestId: number }
  | { readonly kind: "failed"; readonly message: string }
);

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

export type GameSessionUiState = Readonly<{ phase: "boot" }> | GameSessionProjection;

export type MenuFocusState =
  | { readonly kind: "none" }
  | { readonly kind: "control"; readonly controlId: string };

export type AppMenuScrollState =
  | Extract<MenuScrollState, { kind: "closed" }>
  | (Extract<MenuScrollState, { kind: "active" }> & { readonly focus: MenuFocusState });

export interface FlightLogSource {
  readonly phase: "result" | "replay";
  readonly revision: number;
}

export type FlightLogDownloadState =
  | { readonly kind: "idle" }
  | { readonly kind: "pending"; readonly requestId: number; readonly source: FlightLogSource; readonly format: "csv" | "json" }
  | { readonly kind: "requested"; readonly message: string }
  | { readonly kind: "failed"; readonly message: string };

export type LakeWaterQualityUiState =
  | { readonly kind: "ready"; readonly applied: LakeWaterQuality; readonly cleanup: LakeWaterQualityCleanupResult }
  | { readonly kind: "applying"; readonly applied: LakeWaterQuality; readonly requested: LakeWaterQuality; readonly requestId: number }
  | { readonly kind: "failed"; readonly applied: LakeWaterQuality; readonly requested: LakeWaterQuality; readonly message: string };

export interface AppModel {
  readonly sharedGameUi: GameUiView | null;
  readonly lakeWaterQuality: LakeWaterQualityUiState;
  readonly screenRenderScale: GameUiPlatformInput["screen_render_scale"] | null;
  readonly flightLogDownload: FlightLogDownloadState;
  readonly nextFlightLogDownloadRequestId: number;
  readonly flightRecordSourceRevision: number;
  readonly status: string;
  readonly webXrAvailable: boolean;
  readonly phoneVrAvailable: boolean;
  readonly presentation: PresentationUiState;
  readonly gameSession: GameSessionUiState;
  readonly flightExecution: FlightExecutionUiState;
  readonly menuScroll: AppMenuScrollState;
  readonly briefingDetailsOpen: boolean;
  readonly analysis: FlightAnalysisState;
  readonly resultTab: "summary" | "analysis";
  readonly analysisChart: "map" | "altitude" | "speed";
  readonly analysisCursorTimeSeconds: number;
  readonly analysisCursor: AnalysisCursorState;
  readonly replayPose: ReplayPoseQueryState;
  readonly replayViewMode: "cinematic" | "telemetry" | "analysis";
  readonly replayClock: ReplayClockRequestState;
  readonly replayCameraMode: "auto" | FlightCameraMode;
  readonly recordList: RecordListState;
  readonly pendingGameRequestId: number | null;
  readonly nextRequestId: number;
}

export interface FlightControllerIdentity {
  readonly sessionId: number;
  readonly controllerId: number;
}

export type FlightExecutionUiState =
  | { readonly kind: "unbound" }
  | { readonly kind: "ready"; readonly identity: FlightControllerIdentity }
  | {
      readonly kind: "stopped";
      readonly identity: FlightControllerIdentity;
      readonly message: string;
      readonly snapshot: LiveSessionSnapshot;
    };

export type AppMessage =
  | { readonly type: "screen-render-observed"; readonly snapshot: GameUiPlatformInput["screen_render_scale"] }
  | { readonly type: "shared-game-ui-observed"; readonly view: GameUiView }
  | { readonly type: "shared-game-ui-effect"; readonly effect: GameUiEffect }
  | { readonly type: "lake-water-quality-applied"; readonly requestId: number; readonly quality: LakeWaterQuality; readonly cleanup: LakeWaterQualityCleanupResult }
  | { readonly type: "lake-water-quality-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-log-download-requested"; readonly requestId: number; readonly source: FlightLogSource }
  | { readonly type: "flight-log-download-failed"; readonly requestId: number; readonly source: FlightLogSource; readonly message: string }
  | { readonly type: "initialize" }
  | { readonly type: "flight-controller-ready"; readonly identity: FlightControllerIdentity }
  | {
      readonly type: "flight-controller-stopped";
      readonly identity: FlightControllerIdentity;
      readonly message: string;
      readonly snapshot: FlightSnapshotInput;
    }
  | { readonly type: "tail-controller-terminal"; readonly identity: FlightControllerIdentity;
      readonly projection: Extract<TailGameSessionProjection, { phase: "result" }> }
  | { readonly type: "flight-controller-result-feedback"; readonly identity: FlightControllerIdentity; readonly message: string }
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
  | {
      readonly type: "menu-scroll-synchronized";
      readonly generation: number;
      readonly target: { readonly kind: "closed" } | { readonly kind: "active"; readonly scope: MenuScrollScope };
    }
  | { readonly type: "menu-scroll"; readonly context: MenuScrollContext; readonly intent: MenuScrollIntent }
  | { readonly type: "menu-focus"; readonly context: MenuScrollContext; readonly focus: MenuFocusState }
  | { readonly type: "menu-scroll-invalidated"; readonly context: MenuScrollContext }
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
  | (Readonly<{ type: "game-session-synced" }> & GameSessionProjection)
  | (Readonly<{ type: "game-operation-completed"; requestId: number; analysisSourceInvalidated?: boolean }> & GameSessionProjection)
  | { readonly type: "game-session-status"; readonly message: string }
  | { readonly type: "flight-analysis-loaded"; readonly requestId: number; readonly data: FlightAnalysisInput }
  | { readonly type: "flight-analysis-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-analysis-cursor-loaded"; readonly requestId: number; readonly sample: FlightAnalysisCursorInput }
  | { readonly type: "flight-analysis-cursor-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "flight-replay-pose-loaded"; readonly requestId: number; readonly pose: RecordedFlightRenderPose }
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
      readonly analysisSourceInvalidated?: boolean;
    };

export type AppEffect =
  | { readonly type: "select-screen-render-scale"; readonly scale: Extract<GameUiPlatformInput["screen_render_scale"]["selection"], { kind: "manual" }>["scale"] }
  | { readonly type: "select-screen-render-auto"; readonly enabled: boolean }
  | { readonly type: "update-shared-game-ui"; readonly action: GameUiAction }
  | { readonly type: "shared-game-session-command"; readonly command: SessionCommand; readonly requestId: number }
  | { readonly type: "shared-record-action"; readonly action: Extract<GameUiEffect, { kind: "record" }>["action"] }
  | { readonly type: "shared-replay-action"; readonly action: Extract<GameUiEffect, { kind: "record" }>["action"]; readonly requestId: number; readonly generation: number }
  | { readonly type: "apply-lake-water-quality"; readonly requestId: number; readonly quality: LakeWaterQuality }
  | { readonly type: "download-flight-log"; readonly requestId: number; readonly source: FlightLogSource; readonly format: "csv" | "json" }
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
  ;

export interface AppTransition {
  readonly model: AppModel;
  readonly effects: readonly AppEffect[];
}

export function canSelectLakeWaterQuality(model: AppModel): boolean {
  return model.lakeWaterQuality.kind !== "applying" && canSelectRenderingSettings(model);
}

function canSelectRenderingSettings(model: AppModel): boolean {
  return model.presentation.type === "ready" && model.pendingGameRequestId === null
    && (model.gameSession.phase === "flight_setup" || (model.gameSession.phase === "flight_paused"
      && model.sharedGameUi?.selections.pause_overlay === "settings" && model.flightExecution.kind !== "stopped"));
}

export function gameSessionPhase(session: GameSessionUiState): GamePhase {
  return session.phase;
}

export function isStaleGameFlowActivation(
  action: UiAction,
  displayedPhase: GamePhase,
  actualPhase: GamePhase
): boolean {
  return isGameFlowActivation(action) && displayedPhase !== actualPhase;
}

export function isGameFlowActivation(action: UiAction): boolean {
  return (action.type === "activate" || action.type === "set-toggle") && action.controlId.startsWith("game-");
}

export function gameSessionSnapshot(session: GameSessionUiState): FlightSnapshotInput | null {
  return "display" in session && session.display.kind === "available" ? session.display.value : null;
}

export function gameSessionCountdown(session: GameSessionUiState): number {
  return session.phase === "countdown" ? session.countdown_remaining_seconds : 0;
}

export function createInitialAppModel(): AppModel {
  return freezeAppModel({
    sharedGameUi: null,
    lakeWaterQuality: Object.freeze({ kind: "ready", applied: "high", cleanup: Object.freeze({ kind: "complete" }) }),
    screenRenderScale: null,
    flightLogDownload: Object.freeze({ kind: "idle" }),
    nextFlightLogDownloadRequestId: 1,
    flightRecordSourceRevision: 0,
    status: "Screen renderer is initializing",
    webXrAvailable: false,
    phoneVrAvailable: false,
    presentation: Object.freeze({ type: "uninitialized" }),
    gameSession: Object.freeze({ phase: "boot" }),
    flightExecution: Object.freeze({ kind: "unbound" }),
    menuScroll: Object.freeze({ kind: "closed", generation: 0 }),
    analysis: Object.freeze({ kind: "idle", nextRequestId: 1 }),
    analysisCursor: Object.freeze({ kind: "idle", nextRequestId: 1 }),
    replayPose: Object.freeze({ kind: "idle", nextRequestId: 1 }),
    replayClock: Object.freeze({ kind: "settled", generation: 0, nextRequestId: 1,
      observation: Object.freeze({ timeSeconds: 0, speed: 1, playing: false }) }),
    recordList: Object.freeze({ kind: "unloaded", nextRequestId: 1 }),
    pendingGameRequestId: null,
    nextRequestId: 1
  });
}

export function recordListEntries(state: RecordListState): readonly StoredFlightRecordUiEntry[] {
  switch (state.kind) {
    case "unloaded": return EMPTY_STORED_FLIGHT_RECORDS;
    case "ready": return state.records;
    case "loading":
    case "failed": return state.retained;
  }
}

export function recordListNotice(state: RecordListState): string {
  switch (state.kind) {
    case "unloaded": return "";
    case "loading": return "保存記録を読み込んでいる";
    case "ready": return state.records.length === 0 ? "保存記録はまだない。" : "";
    case "failed": return `保存記録一覧を取得できない: ${state.message}`;
  }
}

export function flightAnalysisData(state: FlightAnalysisState): FlightAnalysisInput | null {
  switch (state.kind) {
    case "idle": return null;
    case "ready": return state.data;
    case "loading":
    case "failed": return state.retained;
  }
}

export function analysisCursorData(state: AnalysisCursorState): FlightAnalysisCursorInput | null {
  switch (state.kind) {
    case "idle": return null;
    case "ready": return state.sample;
    case "loading":
    case "failed": return state.retained;
  }
}

export function replayPoseData(state: ReplayPoseQueryState): RecordedFlightRenderPose | null {
  switch (state.kind) {
    case "idle":
    case "failed": return null;
    case "ready": return state.pose;
    case "loading": return state.retained;
  }
}

export function replayClockObservation(state: ReplayClockRequestState): ReplayClockRequestState["observation"] {
  return state.observation;
}

export function updateApp(model: AppModel, message: AppMessage): AppTransition {
  if (model.presentation.type === "cached") {
    if (message.type === "page-suspended" || message.type === "ui-action"
        || message.type === "replay-clock-tick" || message.type === "backend-ended"
        || message.type === "menu-scroll-synchronized" || message.type === "menu-scroll"
        || message.type === "menu-focus" || message.type === "menu-scroll-invalidated") return transition(model);
    if (message.type === "permission-completed") {
      return transition(model, message.ok ? [{ type: "cancel-pending-request", mode: message.mode }] : []);
    }
    const retained = withModel(model, { presentation: model.presentation.retained });
    if (message.type === "page-restored") {
      const recovered = retained.presentation.type === "initializing" || retained.presentation.type === "uninitialized"
          || retained.presentation.type === "failed"
        ? transition(retained)
        : beginScreenRecovery(retained, { origin: "page-restoration", from: null });
      const playback = recovered.model.gameSession.phase === "attract"
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
    case "screen-render-observed":
      return transition(withModel(model, { screenRenderScale: message.snapshot,
        lakeWaterQuality: model.lakeWaterQuality.applied === message.snapshot.water_quality ? model.lakeWaterQuality
          : Object.freeze({ ...model.lakeWaterQuality, applied: message.snapshot.water_quality }) }));
    case "shared-game-ui-observed": {
      const next = withModel(model, { sharedGameUi: message.view });
      return ["replay", "attract"].includes(gameSessionPhase(model.gameSession)) && next.replayCameraMode !== model.replayCameraMode
        ? withReplayPoseQuery(next, next.analysisCursorTimeSeconds, false) : transition(next);
    }
    case "shared-game-ui-effect":
      return updateSharedUiEffect(model, message.effect);
    case "lake-water-quality-applied": {
      const quality = model.lakeWaterQuality;
      if (quality.kind !== "applying" || quality.requestId !== message.requestId || quality.requested !== message.quality) return transition(model);
      return transition(withModel(model, {
        lakeWaterQuality: Object.freeze({ kind: "ready", applied: model.screenRenderScale?.water_quality ?? message.quality,
          cleanup: Object.freeze({ ...message.cleanup }) })
      }));
    }
    case "lake-water-quality-failed": {
      const quality = model.lakeWaterQuality;
      if (quality.kind !== "applying" || quality.requestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        lakeWaterQuality: Object.freeze({ kind: "failed", applied: quality.applied, requested: quality.requested, message: message.message })
      }));
    }
    case "flight-controller-ready": {
      const previous = model.flightExecution;
      const identity = message.identity;
      if (!validFlightControllerIdentity(identity)
          || (previous.kind !== "unbound" && (identity.controllerId <= previous.identity.controllerId
            || identity.sessionId < previous.identity.sessionId))) return transition(model);
      return transition(withModel(model, {
        flightExecution: Object.freeze({ kind: "ready", identity: Object.freeze({ ...identity }) })
      }));
    }
    case "flight-controller-stopped": {
      const execution = model.flightExecution;
      if (execution.kind !== "ready" || execution.identity.sessionId !== message.identity.sessionId
          || execution.identity.controllerId !== message.identity.controllerId
          || (model.gameSession.phase !== "flight_running" && model.gameSession.phase !== "flight_paused")) return transition(model);
      const snapshot = message.snapshot;
      if (!isRetainedLiveSnapshot(snapshot)) return transition(model);
      return transition(withModel(model, {
        flightExecution: Object.freeze({ kind: "stopped", identity: execution.identity,
          message: message.message, snapshot })
      }));
    }
    case "tail-controller-terminal": {
      const execution = model.flightExecution;
      if (execution.kind === "unbound" || execution.identity.sessionId !== message.identity.sessionId
          || execution.identity.controllerId !== message.identity.controllerId || model.gameSession.phase === "boot"
          || !["flight_running", "flight_paused", "result"].includes(model.gameSession.phase)) return transition(model);
      if (model.gameSession.phase === "result") return transition(model);
      return updateApp(model, { type: "game-session-synced", ...message.projection });
    }
    case "flight-controller-result-feedback": {
      const execution = model.flightExecution;
      if (execution.kind === "unbound" || execution.identity.sessionId !== message.identity.sessionId
          || execution.identity.controllerId !== message.identity.controllerId || model.gameSession.phase !== "result") return transition(model);
      return transition(withModel(model, { status: `終端表示処理に失敗した: ${message.message}` }));
    }
    case "flight-log-download-requested":
      if (!isCurrentFlightLogDownload(model, message.requestId, message.source)) return transition(model);
      return transition(withModel(model, {
        flightLogDownload: { kind: "requested", message: "飛行ログのダウンロードを要求した。保存の可否はブラウザーの設定に従う。" }
      }));
    case "flight-log-download-failed":
      if (!isCurrentFlightLogDownload(model, message.requestId, message.source)) return transition(model);
      return transition(withModel(model, {
        flightLogDownload: { kind: "failed", message: `飛行ログのダウンロード要求に失敗した: ${message.message}` }
      }));
    case "page-restored":
      return transition(model);
    case "page-suspended": {
      let suspended = withModel(model, {
        replayClock: stopReplayClock(model.replayClock)
      });
      const effects: AppEffect[] = [
        { type: "suspend-page-flight" },
        { type: "cancel-pending-request", mode: "webxr" },
        { type: "cancel-pending-request", mode: "phone-vr" }
      ];
      if (model.gameSession.phase === "replay" || model.gameSession.phase === "attract") {
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
    case "menu-scroll-synchronized":
      return synchronizeMenuScroll(model, message.generation, message.target);
    case "menu-scroll":
      return updateMenuScroll(model, message.context, message.intent);
    case "menu-focus":
      return updateMenuFocus(model, message.context, message.focus);
    case "menu-scroll-invalidated":
      return invalidateMenuScroll(model, message.context);
    case "refresh-stored-flight-records":
      return beginStoredFlightRecordLoad(model);
    case "stored-flight-records-loaded":
      if (model.recordList.kind !== "loading" || model.recordList.requestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        recordList: Object.freeze({ kind: "ready", nextRequestId: model.recordList.nextRequestId,
          records: Object.freeze([...message.records]) })
      }));
    case "stored-flight-records-failed":
      if (model.recordList.kind !== "loading" || model.recordList.requestId !== message.requestId) return transition(model);
      return transition(withModel(model, {
        recordList: Object.freeze({ kind: "failed", nextRequestId: model.recordList.nextRequestId,
          message: message.message, retained: model.recordList.retained })
      }));
    case "permission-completed":
      return updatePermissionCompletion(model, message);
    case "backend-transition-completed":
      return updateBackendCompletion(model, message);
    case "backend-ended": {
      if (model.presentation.type !== "ready" || model.presentation.mode !== message.mode) return transition(model);
      return beginScreenRecovery(model, { origin: "backend-fault", from: message.mode, cause: message.message });
    }
    case "game-session-synced": {
      const gameSession: GameSessionProjection = message;
      const previousPhase = gameSessionPhase(model.gameSession);
      const nextPhase = gameSessionPhase(gameSession);
      const returningReplay = previousPhase === "replay" && nextPhase === "result";
      const enteringResult = entersResult(previousPhase, nextPhase) && !returningReplay;
      const analysis = flightAnalysisData(model.analysis);
      const refreshingNamedAnalysis = ["result", "replay", "attract"].includes(nextPhase) && (analysisPhaseMismatch(analysis, nextPhase)
        || (returningReplay && analysis === null) || (nextPhase === "result" && model.analysis.kind === "idle"));
      const analysisRequestId = enteringResult || refreshingNamedAnalysis ? model.analysis.nextRequestId : null;
      const effects: AppEffect[] = enteringResult && analysisRequestId !== null
        ? [{ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: analysisRequestId }]
        : refreshingNamedAnalysis && analysisRequestId !== null ? [{ type: "load-flight-analysis", requestId: analysisRequestId }] : [];
      return transition(withModel(model, {
        gameSession: Object.freeze(gameSession),
        analysis: observeAnalysisPhase(model.analysis, nextPhase, analysisRequestId, refreshingNamedAnalysis),
        analysisCursor: ["result", "replay", "attract"].includes(nextPhase) && !enteringResult && !refreshingNamedAnalysis
          ? model.analysisCursor : Object.freeze({ kind: "idle", nextRequestId: model.analysisCursor.nextRequestId }),
        replayClock: ["replay", "attract"].includes(nextPhase) ? model.replayClock : stopReplayClock(model.replayClock)
      }), effects);
    }
    case "game-operation-completed": {
      const gameSession: GameSessionProjection = message;
      if (model.pendingGameRequestId !== message.requestId) return transition(model);
      const previousPhase = gameSessionPhase(model.gameSession);
      const nextPhase = gameSessionPhase(gameSession);
      const returningReplay = previousPhase === "replay" && nextPhase === "result";
      const enteringResult = entersResult(previousPhase, nextPhase) && !returningReplay;
      const enteringReplay = previousPhase !== "replay" && nextPhase === "replay";
      const enteringAttract = previousPhase !== "attract" && nextPhase === "attract";
      const enteringPlayback = enteringReplay || enteringAttract;
      const leavingPlayback = ["replay", "attract"].includes(previousPhase) && !["replay", "attract"].includes(nextPhase);
      const replayClock = enteringPlayback
        ? requestReplayClock({ ...model.replayClock,
          observation: Object.freeze({ ...model.replayClock.observation, playing: false }) }, true)
        : leavingPlayback ? stopReplayClock(model.replayClock)
        : ["replay", "attract"].includes(nextPhase) ? model.replayClock
        : Object.freeze({ ...model.replayClock,
          observation: Object.freeze({ ...model.replayClock.observation, playing: false }) });
      const analysis = flightAnalysisData(model.analysis);
      const refreshingNamedAnalysis = ["result", "replay", "attract"].includes(nextPhase) && (message.analysisSourceInvalidated === true
        || analysisPhaseMismatch(analysis, nextPhase) || (returningReplay && analysis === null)
        || (nextPhase === "result" && model.analysis.kind === "idle"));
      const analysisRequestId = enteringResult || enteringAttract || refreshingNamedAnalysis || (enteringReplay && analysis === null)
        ? model.analysis.nextRequestId
        : null;
      const effects: AppEffect[] = [];
      if (enteringPlayback && replayClock.kind === "requesting") {
        effects.push({
          type: "control-replay-clock",
          requestId: replayClock.requestId,
          generation: replayClock.generation,
          command: {
            kind: "synchronize",
            seekTimeSeconds: enteringReplay && previousPhase === "result"
              ? model.analysisCursorTimeSeconds
              : null
          }
        });
      }
      if (enteringResult && analysisRequestId !== null) {
        effects.push({ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: analysisRequestId });
      } else if ((enteringPlayback || refreshingNamedAnalysis) && analysisRequestId !== null) {
        effects.push({ type: "load-flight-analysis", requestId: analysisRequestId });
      }
      return transition(withModel(model, {
        pendingGameRequestId: null,
        gameSession: Object.freeze(gameSession),
        status: "",
        analysis: observeAnalysisPhase(model.analysis, nextPhase, analysisRequestId, refreshingNamedAnalysis),
        analysisCursor: ["result", "replay", "attract"].includes(nextPhase) && !enteringResult && !refreshingNamedAnalysis
          ? model.analysisCursor : Object.freeze({ kind: "idle", nextRequestId: model.analysisCursor.nextRequestId }),
        replayPose: enteringPlayback || !["replay", "attract"].includes(nextPhase) || message.analysisSourceInvalidated === true
          ? Object.freeze({ kind: "idle", nextRequestId: model.replayPose.nextRequestId }) : model.replayPose,
        replayClock
      }), effects);
    }
    case "flight-analysis-loaded":
      if (model.analysis.kind !== "loading" || model.analysis.requestId !== message.requestId
          || !["result", "replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      {
        try {
          if (analysisPhaseMismatch(message.data, gameSessionPhase(model.gameSession))) {
            throw new RangeError("Analysis context and Rust session phase disagree");
          }
        } catch (error: unknown) {
          if (!(error instanceof RangeError)) throw error;
          return transition(withModel(model, {
            analysis: Object.freeze({ kind: "failed", nextRequestId: model.analysis.nextRequestId,
              message: error.message, retained: model.analysis.retained }),
            status: `Analysisデータが一致しない: ${error.message}`
          }));
        }
        const requestId = model.analysisCursor.nextRequestId;
        const playbackPhase = gameSessionPhase(model.gameSession);
        const replayPoseRequestId = ["replay", "attract"].includes(playbackPhase) ? model.replayPose.nextRequestId : null;
        const timeSeconds = model.analysisCursorTimeSeconds;
        const effects: AppEffect[] = [{ type: "load-flight-analysis-cursor", requestId, timeSeconds }];
        if (replayPoseRequestId !== null) {
          effects.push({ type: "load-flight-replay-pose", requestId: replayPoseRequestId, timeSeconds });
        }
        if (["replay", "attract"].includes(playbackPhase) && model.replayClock.observation.playing) {
          effects.push({ type: "schedule-replay-clock-tick", generation: model.replayClock.generation, delayMilliseconds: 50 });
        }
        return transition(withModel(model, {
          analysis: Object.freeze({ kind: "ready", nextRequestId: model.analysis.nextRequestId, data: message.data }),
          analysisCursor: Object.freeze({ kind: "loading", requestId, nextRequestId: requestId + 1, retained: null }),
          replayPose: replayPoseRequestId === null
            ? Object.freeze({ kind: "idle", nextRequestId: model.replayPose.nextRequestId })
            : Object.freeze({ kind: "loading", requestId: replayPoseRequestId, nextRequestId: replayPoseRequestId + 1,
              retained: replayPoseData(model.replayPose) })
        }), effects);
      }
    case "flight-analysis-failed":
      if (model.analysis.kind !== "loading" || model.analysis.requestId !== message.requestId
          || !["result", "replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        analysis: Object.freeze({ kind: "failed", nextRequestId: model.analysis.nextRequestId,
          message: message.message, retained: null }),
        analysisCursor: settleAnalysisCursor(model.analysisCursor),
        replayClock: gameSessionPhase(model.gameSession) === "attract"
          ? Object.freeze({ ...model.replayClock, generation: model.replayClock.generation + 1,
            observation: Object.freeze({ ...model.replayClock.observation, playing: false }) }) : model.replayClock,
        status: `Analysisデータを取得できない: ${message.message}`
      }));
    case "flight-analysis-cursor-loaded":
      if (model.analysisCursor.kind !== "loading" || model.analysisCursor.requestId !== message.requestId
          || !["result", "replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      if (!analysisCursorMatches(flightAnalysisData(model.analysis), message.sample) || ("kind" in message.sample
          && message.sample.context.phase !== analysisPhase(gameSessionPhase(model.gameSession)))) return transition(withModel(model, {
        analysisCursor: Object.freeze({ kind: "failed", nextRequestId: model.analysisCursor.nextRequestId,
          message: "Analysis cursorの記録または操縦layoutが一致しない", retained: model.analysisCursor.retained }),
        status: "Analysis cursorの記録または操縦layoutが一致しない"
      }));
      return transition(withModel(model, { analysisCursor: Object.freeze({ kind: "ready",
        nextRequestId: model.analysisCursor.nextRequestId, sample: message.sample }) }));
    case "flight-analysis-cursor-failed":
      if (model.analysisCursor.kind !== "loading" || model.analysisCursor.requestId !== message.requestId
          || !["result", "replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        analysisCursor: Object.freeze({ kind: "failed", nextRequestId: model.analysisCursor.nextRequestId,
          message: message.message, retained: null }),
        status: `Analysis cursorを取得できない: ${message.message}`
      }));
    case "flight-replay-pose-loaded":
      if (model.replayPose.kind !== "loading" || model.replayPose.requestId !== message.requestId
          || !["replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      return transition(withModel(model, { replayPose: Object.freeze({ kind: "ready",
        nextRequestId: model.replayPose.nextRequestId, pose: message.pose }) }));
    case "flight-replay-pose-failed":
      if (model.replayPose.kind !== "loading" || model.replayPose.requestId !== message.requestId
          || !["replay", "attract"].includes(gameSessionPhase(model.gameSession))) return transition(model);
      return transition(withModel(model, {
        replayPose: Object.freeze({ kind: "failed", nextRequestId: model.replayPose.nextRequestId, message: message.message }),
        replayClock: gameSessionPhase(model.gameSession) === "attract"
          ? Object.freeze({ ...model.replayClock, generation: model.replayClock.generation + 1,
            observation: Object.freeze({ ...model.replayClock.observation, playing: false }) }) : model.replayClock,
        status: `Replay poseを取得できない: ${message.message}`
      }));
    case "replay-clock-tick":
      return updateReplayClock(model, message);
    case "replay-clock-command-completed": {
      const phase = gameSessionPhase(model.gameSession);
      if (model.replayClock.kind !== "requesting" || model.replayClock.requestId !== message.requestId
          || model.replayClock.generation !== message.generation
          || !["replay", "attract"].includes(phase)) return transition(model);
      const speed = playbackRateForCode(message.state.rateCode);
      if (speed === null || !Number.isFinite(message.state.timeSeconds) || message.state.timeSeconds < 0) {
        return transition(withModel(model, {
          replayClock: Object.freeze({ kind: "failed", message: "Rust Replay clock returned an invalid state",
            nextRequestId: model.replayClock.nextRequestId, generation: model.replayClock.generation + 1,
            observation: Object.freeze({ ...model.replayClock.observation, playing: false }) }),
          status: "Rust Replay clock returned an invalid state"
        }));
      }
      const next = withModel(model, {
        replayClock: Object.freeze({ kind: "settled", nextRequestId: model.replayClock.nextRequestId,
          generation: model.replayClock.generation,
          observation: Object.freeze({ timeSeconds: message.state.timeSeconds, speed, playing: message.state.playing }) })
      });
      const queried = flightAnalysisData(model.analysis) === null
        ? { model: next, effects: [] as AppEffect[] }
        : withReplayQuery(next, message.state.timeSeconds);
      if (message.state.playing && flightAnalysisData(model.analysis) !== null) {
        queried.effects.push({
          type: "schedule-replay-clock-tick",
          generation: message.generation,
          delayMilliseconds: 50
        });
      }
      return transition(queried.model, queried.effects);
    }
    case "replay-clock-command-failed":
      if (model.replayClock.kind !== "requesting" || model.replayClock.requestId !== message.requestId
          || model.replayClock.generation !== message.generation) return transition(model);
      return transition(withModel(model, {
        replayClock: Object.freeze({ kind: "failed", message: message.message, nextRequestId: model.replayClock.nextRequestId,
          generation: model.replayClock.generation + 1, observation: Object.freeze({
            playing: message.state?.playing ?? false,
            speed: message.state === undefined ? model.replayClock.observation.speed
              : playbackRateForCode(message.state.rateCode) ?? model.replayClock.observation.speed,
            timeSeconds: message.state?.timeSeconds ?? model.replayClock.observation.timeSeconds
          }) }),
        status: `Replay clock operation failed: ${message.message}`
      }));
    case "game-session-status":
      return transition(withModel(model, { status: message.message }));
    case "game-operation-failed": {
      if (model.pendingGameRequestId !== message.requestId) return transition(model);
      if (message.currentSession !== undefined) {
        const completed = updateApp(model, {
          type: "game-operation-completed",
          requestId: message.requestId,
          analysisSourceInvalidated: message.analysisSourceInvalidated === true,
          ...message.currentSession
        });
        if (completed.model.status === "GameSessionから不正な状態snapshotを受信した") return completed;
        return transition(withModel(completed.model, { status: message.message }), completed.effects);
      }
      return transition(withModel(model, {
        pendingGameRequestId: null,
        status: message.message,
        analysis: message.analysisSourceInvalidated === true
          ? observeAnalysisPhase(model.analysis, "title", null, true) : model.analysis,
        analysisCursor: message.analysisSourceInvalidated === true
          ? Object.freeze({ kind: "idle", nextRequestId: model.analysisCursor.nextRequestId }) : model.analysisCursor,
        replayPose: message.analysisSourceInvalidated === true
          ? Object.freeze({ kind: "idle", nextRequestId: model.replayPose.nextRequestId }) : model.replayPose
      }));
    }
    case "page-hidden": {
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "hidden" }),
        replayClock: stopReplayClock(model.replayClock),
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

function sameMenuScope(left: MenuScrollScope, right: MenuScrollScope): boolean {
  return left.scene === right.scene && left.panelId === right.panelId && left.viewKey === right.viewKey
    && (left.kind === "scene" ? right.kind === "scene" : right.kind === "overlay" && left.overlay === right.overlay);
}

function menuContextMatches(state: AppMenuScrollState, context: MenuScrollContext): boolean {
  return state.kind === "active" && state.generation === context.generation
    && sameMenuScope(state.scope, context.scope);
}

function nextMenuGeneration(generation: number): number | null {
  return Number.isSafeInteger(generation) && generation >= 0 && generation < Number.MAX_SAFE_INTEGER
    ? generation + 1 : null;
}

function synchronizeMenuScroll(
  model: AppModel,
  observedGeneration: number,
  target: { readonly kind: "closed" } | { readonly kind: "active"; readonly scope: MenuScrollScope }
): AppTransition {
  const current = model.menuScroll;
  if (observedGeneration !== current.generation) return transition(model);
  if (target.kind === "closed" && current.kind === "closed") return transition(model);
  if (target.kind === "active") {
    if (target.scope.panelId.trim().length === 0 || target.scope.viewKey.trim().length === 0
        || target.scope.kind === "overlay" && target.scope.overlay.trim().length === 0) return transition(model);
    if (current.kind === "active" && sameMenuScope(current.scope, target.scope)) return transition(model);
  }
  const generation = nextMenuGeneration(current.generation);
  if (generation === null) return transition(model);
  if (target.kind === "closed") {
    return transition(withModel(model, { menuScroll: Object.freeze({ kind: "closed", generation }) }));
  }
  const scope = target.scope.kind === "scene"
    ? Object.freeze({ kind: target.scope.kind, scene: target.scope.scene, panelId: target.scope.panelId, viewKey: target.scope.viewKey })
    : Object.freeze({ kind: target.scope.kind, scene: target.scope.scene, panelId: target.scope.panelId, viewKey: target.scope.viewKey, overlay: target.scope.overlay });
  return transition(withModel(model, {
    menuScroll: Object.freeze({ kind: "active", scope, generation, progress: 0, focus: Object.freeze({ kind: "none" }) })
  }));
}

function updateMenuScroll(model: AppModel, context: MenuScrollContext, intent: MenuScrollIntent): AppTransition {
  const current = model.menuScroll;
  if (current.kind !== "active" || !menuContextMatches(current, context)) return transition(model);
  let progress: number;
  if (intent.kind === "set-progress") {
    if (!Number.isFinite(intent.progress) || intent.progress < 0 || intent.progress > 1) return transition(model);
    progress = intent.progress;
  } else {
    if (!Number.isFinite(intent.pageProgress) || intent.pageProgress < 0 || intent.pageProgress > 1) return transition(model);
    const pages = intent.kind === "page" ? (intent.direction === "previous" ? -1 : 1) : intent.viewportPages;
    if (!Number.isFinite(pages)) return transition(model);
    const change = pages * intent.pageProgress;
    if (!Number.isFinite(change)) return transition(model);
    progress = Math.max(0, Math.min(1, current.progress + change));
  }
  if (progress === current.progress) return transition(model);
  const generation = nextMenuGeneration(current.generation);
  if (generation === null) return transition(model);
  return transition(withModel(model, {
    menuScroll: Object.freeze({ ...current, generation, progress, focus: Object.freeze({ kind: "none" }) })
  }));
}

function updateMenuFocus(model: AppModel, context: MenuScrollContext, focus: MenuFocusState): AppTransition {
  const current = model.menuScroll;
  if (current.kind !== "active" || !menuContextMatches(current, context)) return transition(model);
  if (focus.kind === "control" && focus.controlId.trim().length === 0) return transition(model);
  if (focus.kind === "none" ? current.focus.kind === "none"
      : current.focus.kind === "control" && current.focus.controlId === focus.controlId) return transition(model);
  const nextFocus = focus.kind === "none" ? Object.freeze({ kind: focus.kind })
    : Object.freeze({ kind: focus.kind, controlId: focus.controlId });
  return transition(withModel(model, { menuScroll: Object.freeze({ ...current, focus: nextFocus }) }));
}

function invalidateMenuScroll(model: AppModel, context: MenuScrollContext): AppTransition {
  const current = model.menuScroll;
  if (current.kind !== "active" || !menuContextMatches(current, context)) return transition(model);
  const generation = nextMenuGeneration(current.generation);
  if (generation === null) return transition(model);
  return transition(withModel(model, {
    menuScroll: Object.freeze({ ...current, generation, focus: Object.freeze({ kind: "none" }) })
  }));
}

function updateUiAction(model: AppModel, action: UiAction): AppTransition {
  if (action.type === "focus" || action.type === "back" || action.type === "scroll") return transition(model);
  if (action.type === "recenter-menu") {
    return isInVr(model.presentation) ? transition(model, [{ type: "recenter-menu" }]) : transition(model);
  }
  if (model.sharedGameUi !== null) {
    const semantic = sharedGameUiAction(model.sharedGameUi, action);
    return semantic === null ? transition(model) : transition(model, [{ type: "update-shared-game-ui", action: semantic }]);
  }
  if (action.type !== "activate" || model.gameSession.phase !== "boot") return transition(model);
  switch (action.controlId) {
    case "boot-enter-webxr": return beginPermissionRequest(model, "webxr");
    case "boot-enter-phone-vr": return beginPermissionRequest(model, "phone-vr");
    case "boot-exit-vr": return beginBackendSwitch(model, "screen", "Ending presentation session");
    case "boot-recenter-phone-tracking":
      return model.presentation.type === "ready" && model.presentation.mode === "phone-vr"
        ? transition(model, [{ type: "recenter-tracking" }]) : transition(model);
    case "boot-recenter-menu": return isInVr(model.presentation) ? transition(model, [{ type: "recenter-menu" }]) : transition(model);
    default: return transition(model);
  }
}

function updateSharedUiEffect(model: AppModel, effect: GameUiEffect): AppTransition {
  if (effect.kind === "session") {
    if (model.pendingGameRequestId !== null || model.presentation.type !== "ready") return transition(model);
    if (effect.command.kind === "resume" && model.flightExecution.kind === "stopped") return transition(model);
    const requestId = model.nextRequestId;
    return transition(withModel(model, { pendingGameRequestId: requestId, nextRequestId: requestId + 1, status: "ゲーム状態を更新している" }),
      [{ type: "shared-game-session-command", command: effect.command, requestId }]);
  }
  if (effect.kind === "record") {
    const action = effect.action;
    if (action.kind === "seek_analysis") {
      const requestId = model.analysisCursor.nextRequestId;
      return transition(withModel(model, { analysisCursor: Object.freeze({ kind: "loading", requestId,
        nextRequestId: requestId + 1, retained: null }) }), [
        { type: "shared-record-action", action },
        { type: "load-flight-analysis-cursor", requestId, timeSeconds: action.time_seconds }
      ]);
    }
    if (model.replayClock.kind === "requesting") return transition(model);
    const replayClock = requestReplayClock(model.replayClock, true);
    return transition(withModel(model, { replayClock }),
      [{ type: "shared-replay-action", action, requestId: replayClock.requestId, generation: replayClock.generation }]);
  }
  const command = effect.command;
  switch (command.kind) {
    case "set_screen_render_scale":
      return model.presentation.type === "ready" && model.presentation.mode === "screen"
          && canSelectRenderingSettings(model)
        ? transition(model, [{ type: "select-screen-render-scale", scale: command.scale }]) : transition(model);
    case "set_screen_render_auto":
      return model.presentation.type === "ready" && model.presentation.mode === "screen"
          && canSelectRenderingSettings(model)
        ? transition(model, [{ type: "select-screen-render-auto", enabled: command.enabled }]) : transition(model);
    case "set_water_quality": {
      const selection = model.screenRenderScale?.selection;
      const selected = selection?.kind === "auto" ? selection.manual_water_quality : selection?.water_quality ?? model.lakeWaterQuality.applied;
      if (!canSelectLakeWaterQuality(model)
          || command.quality === selected && model.lakeWaterQuality.kind === "ready"
          || !Number.isSafeInteger(model.nextRequestId) || model.nextRequestId >= Number.MAX_SAFE_INTEGER) return transition(model);
      const requestId = model.nextRequestId;
      return transition(withModel(model, { lakeWaterQuality: Object.freeze({ kind: "applying", applied: model.lakeWaterQuality.applied,
        requested: command.quality, requestId }), nextRequestId: requestId + 1 }),
        [{ type: "apply-lake-water-quality", requestId, quality: command.quality }]);
    }
    case "set_presentation": {
      const mode = command.mode === "phone_vr" ? "phone-vr" : command.mode;
      return mode === "screen" ? beginBackendSwitch(model, mode, "Ending presentation session") : beginPermissionRequest(model, mode);
    }
    case "recenter_phone_tracking": return transition(model, [{ type: "recenter-tracking" }]);
    case "recenter_menu": return transition(model, [{ type: "recenter-menu" }]);
    case "download_flight_csv":
    case "download_record_json": {
      const phase = gameSessionPhase(model.gameSession);
      if ((phase !== "result" && phase !== "replay") || model.flightLogDownload.kind === "pending" || model.presentation.type !== "ready") return transition(model);
      const requestId = model.nextFlightLogDownloadRequestId;
      const source: FlightLogSource = Object.freeze({ phase, revision: model.flightRecordSourceRevision });
      const format = command.kind === "download_flight_csv" ? "csv" : "json";
      return transition(withModel(model, { flightLogDownload: Object.freeze({ kind: "pending", requestId, source, format }),
        nextFlightLogDownloadRequestId: requestId + 1 }), [{ type: "download-flight-log", requestId, source, format }]);
    }
    case "open_stored_record": {
      if (model.pendingGameRequestId !== null) return transition(model);
      const requestId = model.nextRequestId;
      return transition(withModel(model, { pendingGameRequestId: requestId, nextRequestId: requestId + 1,
        status: `FlightRecord ${String(command.id)}を開いている` }), [{ type: "open-stored-flight-record", id: command.id, requestId }]);
    }
    case "set_replay_camera": return transition(model);
    default: return assertNever(command);
  }
}

function updateReplayClock(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "replay-clock-tick" }>
): AppTransition {
  const phase = gameSessionPhase(model.gameSession);
  if (!model.replayClock.observation.playing || !["replay", "attract"].includes(phase) || message.generation !== model.replayClock.generation
      || flightAnalysisData(model.analysis) === null || !Number.isFinite(message.elapsedSeconds) || message.elapsedSeconds < 0) {
    return transition(model);
  }
  return beginReplayClockCommand(model, { kind: "advance", elapsedSeconds: message.elapsedSeconds }, false);
}

function beginReplayClockCommand(
  model: AppModel,
  command: ReplayClockCommand,
  restartSchedule: boolean
): AppTransition {
  if (model.replayClock.kind === "requesting") return transition(model);
  const replayClock = requestReplayClock(model.replayClock, restartSchedule);
  return transition(withModel(model, { replayClock }),
    [{ type: "control-replay-clock", requestId: replayClock.requestId, generation: replayClock.generation, command }]);
}

function requestReplayClock(state: ReplayClockRequestState, restartSchedule: boolean): Extract<ReplayClockRequestState, { kind: "requesting" }> {
  return Object.freeze({ kind: "requesting", requestId: state.nextRequestId, nextRequestId: state.nextRequestId + 1,
    generation: restartSchedule ? state.generation + 1 : state.generation, observation: state.observation });
}

function stopReplayClock(state: ReplayClockRequestState): ReplayClockRequestState {
  return Object.freeze({ kind: "settled", nextRequestId: state.nextRequestId, generation: state.generation + 1,
    observation: Object.freeze({ ...state.observation, playing: false }) });
}

function playbackRateForCode(code: number): ReplayClockRequestState["observation"]["speed"] | null {
  switch (code) {
    case 0: return 0.5;
    case 1: return 1;
    case 2: return 2;
    default: return null;
  }
}

function withReplayQuery(model: AppModel, timeSeconds: number): { readonly model: AppModel; readonly effects: AppEffect[] } {
  const replay = withReplayPoseQuery(model, timeSeconds);
  model = replay.model;
  const cursorRequestId = model.analysisCursor.nextRequestId;
  model = withModel(model, {
    analysisCursor: Object.freeze({ kind: "loading", requestId: cursorRequestId, nextRequestId: cursorRequestId + 1,
      retained: analysisCursorData(model.analysisCursor) })
  });
  return {
    model,
    effects: [
      ...replay.effects,
      { type: "load-flight-analysis-cursor", requestId: cursorRequestId, timeSeconds }
    ]
  };
}

function withReplayPoseQuery(model: AppModel, timeSeconds: number, retain = true): AppTransition {
  const requestId = model.replayPose.nextRequestId;
  return transition(withModel(model, { replayPose: Object.freeze({ kind: "loading", requestId, nextRequestId: requestId + 1,
    retained: retain ? replayPoseData(model.replayPose) : null }) }), [{ type: "load-flight-replay-pose", requestId, timeSeconds }]);
}


function validFlightControllerIdentity(identity: FlightControllerIdentity): boolean {
  return Number.isSafeInteger(identity.sessionId) && identity.sessionId > 0
    && Number.isSafeInteger(identity.controllerId) && identity.controllerId > 0;
}

function beginStoredFlightRecordLoad(model: AppModel): AppTransition {
  const requestId = model.recordList.nextRequestId;
  return transition(withModel(model, {
    recordList: Object.freeze({ kind: "loading", requestId, nextRequestId: requestId + 1,
      retained: recordListEntries(model.recordList) })
  }), [{ type: "load-stored-flight-records", requestId }]);
}

function entersResult(previousPhase: GamePhase, nextPhase: GamePhase): boolean {
  return previousPhase !== "result" && nextPhase === "result";
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
    status: message.mode === "webxr" ? "Starting WebXR presentation" : "Waiting for gravity-referenced orientation."
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
    const next = withModel(model, {
      presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
      status: `${causePrefix}${outcome}`
    });
    return ["replay", "attract"].includes(gameSessionPhase(next.gameSession))
      ? withReplayPoseQuery(next, next.analysisCursorTimeSeconds, false) : transition(next);
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
  const playback = origin.origin === "backend-fault" && (model.gameSession.phase === "replay" || model.gameSession.phase === "attract")
    ? beginReplayClockCommand(withModel(model, { replayClock: stopReplayClock(model.replayClock) }), { kind: "pause" }, false)
    : transition(model);
  model = playback.model;
  const requestId = model.nextRequestId;
  const message = origin.origin === "backend-fault" ? origin.cause : "Page restored";
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", ...origin, requestId, to: "screen", phase: "stopping" }),
    nextRequestId: requestId + 1,
    status: `${message}; restoring Screen`
  }), [...playback.effects, { type: "switch-backend", mode: "screen", requestId }]);
}

function canRequestPresentation(state: PresentationUiState): boolean {
  return state.type === "failed" || (state.type === "ready" && state.mode === "screen");
}

function isInVr(state: PresentationUiState): boolean {
  return state.type === "ready" && state.mode !== "screen";
}

function observeAnalysisPhase(state: FlightAnalysisState, phase: GamePhase, requestId: number | null,
  clearData: boolean): FlightAnalysisState {
  const keepData = ["result", "replay", "attract"].includes(phase) && !clearData;
  const data = keepData ? flightAnalysisData(state) : null;
  if (requestId !== null) return Object.freeze({ kind: "loading", requestId, nextRequestId: requestId + 1, retained: data });
  if (!keepData) return Object.freeze({ kind: "idle", nextRequestId: state.nextRequestId });
  return state;
}

function settleAnalysisCursor(state: AnalysisCursorState): AnalysisCursorState {
  if (state.kind !== "loading") return state;
  return state.retained === null ? Object.freeze({ kind: "idle", nextRequestId: state.nextRequestId })
    : Object.freeze({ kind: "ready", nextRequestId: state.nextRequestId, sample: state.retained });
}

function analysisPhase(phase: GamePhase): "result" | "replay" | "attract" | "unavailable" {
  if (phase === "result") return "result";
  if (phase === "replay") return "replay";
  if (phase === "attract") return "attract";
  return "unavailable";
}

function analysisPhaseMismatch(data: FlightAnalysisInput | null, phase: GamePhase): boolean {
  return data !== null && data.context.phase !== analysisPhase(phase);
}

function labelForMode(mode: PresentationMode): string {
  switch (mode) {
    case "screen": return "Screen";
    case "webxr": return "WebXR";
    case "phone-vr": return "Phone VR";
  }
}

function withModel(model: AppModel, changes: Partial<AppModel>): AppModel {
  const sourceChanged = changes.gameSession !== undefined && changes.gameSession.phase !== model.gameSession.phase
    || changes.pendingGameRequestId !== undefined && changes.pendingGameRequestId !== null && changes.pendingGameRequestId !== model.pendingGameRequestId;
  const unavailable = changes.presentation?.type === "hidden" || changes.presentation?.type === "cached";
  return freezeAppModel({
    ...model,
    ...changes,
    ...(sourceChanged ? { flightRecordSourceRevision: model.flightRecordSourceRevision + 1 } : {}),
    ...(sourceChanged || unavailable ? { flightLogDownload: Object.freeze({ kind: "idle" as const }) } : {})
  });
}

type AppModelSource = Omit<AppModel, "briefingDetailsOpen" | "resultTab" | "analysisChart" | "analysisCursorTimeSeconds" | "replayViewMode" | "replayCameraMode">;

function freezeAppModel(source: AppModelSource): AppModel {
  return Object.freeze({ ...source,
    get briefingDetailsOpen() { return source.sharedGameUi?.selections.technical_open ?? false; },
    get resultTab() { return source.sharedGameUi?.selections.result_tab ?? "summary"; },
    get analysisChart() { return source.sharedGameUi?.selections.analysis_chart ?? "map"; },
    get analysisCursorTimeSeconds() { return ["replay", "attract"].includes(source.gameSession.phase)
      ? replayClockObservation(source.replayClock).timeSeconds : source.sharedGameUi?.selections.analysis_time_seconds ?? 0; },
    get replayViewMode() { return source.sharedGameUi?.selections.replay_view ?? "cinematic"; },
    get replayCameraMode() { return source.sharedGameUi?.selections.replay_camera ?? "auto"; }
  });
}

export function isCurrentFlightLogDownload(model: AppModel, requestId: number, source: FlightLogSource): boolean {
  return model.flightLogDownload.kind === "pending" && model.flightLogDownload.requestId === requestId
    && model.flightLogDownload.source.phase === source.phase
    && model.flightLogDownload.source.revision === source.revision
    && model.flightRecordSourceRevision === source.revision && model.gameSession.phase === source.phase
    && model.pendingGameRequestId === null && model.presentation.type === "ready";
}


function transition(model: AppModel, effects: readonly AppEffect[] = []): AppTransition {
  return Object.freeze({ model, effects: Object.freeze([...effects]) });
}

function assertNever(value: never): never {
  throw new Error(`Unhandled application message: ${JSON.stringify(value)}`);
}
