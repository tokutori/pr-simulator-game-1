import { createBootViewModel } from "./boot-view.js";
import { createGameViewModel } from "./game-view.js";
import { gameSessionPhaseCode } from "./app-state.js";
import type { AppMessage, AppModel } from "./app-state.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import type { FlightSnapshotInput } from "./session-snapshot.js";
import { NO_ENVIRONMENT_BRIEFING } from "../game/environment-briefing.js";
import type { EnvironmentBriefingProjection } from "../game/environment-briefing.js";
import type { FlightHudModel } from "../presentation/flight-hud-model.js";
import { projectSharedFlightHudModel } from "../presentation/flight-hud-model.js";
import { createHeadHudView, DEFAULT_HEAD_HUD_PROFILE, NO_HEAD_HUD_VIEW } from "../presentation/head-hud-view.js";
import type { HeadHudView } from "../presentation/head-hud-view.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import type { PanelUnavailableReason } from "../render/contracts/runtime.js";
import { NO_VENUE_MAP } from "../game/biwa-venue-map.js";
import type { VenueMapProjection } from "../game/biwa-venue-map.js";

export interface FlightFrameViewDraft {
  readonly model: AppModel;
  readonly snapshot: FlightDisplaySnapshot | null;
  readonly analysis: AppModel["flightAnalysis"];
  readonly hud: FlightHudModel | null;
  readonly headHud: HeadHudView;
  readonly environment: EnvironmentBriefingProjection;
  readonly venue: VenueMapProjection;
  readonly frameRateEnabled: boolean;
}

export function createFlightFrameViewDraft(model: AppModel, snapshotInput: FlightSnapshotInput | null, viewer: ViewerFrame, locale: string,
  environment: EnvironmentBriefingProjection = NO_ENVIRONMENT_BRIEFING, venue: VenueMapProjection = NO_VENUE_MAP,
  framesPerSecond?: number | null): FlightFrameViewDraft {
  const phase = gameSessionPhaseCode(model.gameSession);
  const code = model.difficulty.informationCode;
  if (!Number.isInteger(code) || code < 0 || code > 4) throw new RangeError("Information code must lie in [0, 4]");
  let snapshot = snapshotInput;
  if (model.flightExecution.kind === "stopped" && (phase === 5 || phase === 6)) snapshot = model.flightExecution.snapshot;
  const hud = snapshot !== null && (phase === 5 || phase === 6)
    ? createFlightUiHudModel(model)
    : null;
  const headHud = model.presentation.type === "ready" && model.presentation.mode !== "screen" && ((hud !== null && phase === 5) || framesPerSecond !== undefined)
    ? createHeadHudView(phase === 5 ? hud : null, viewer, locale, DEFAULT_HEAD_HUD_PROFILE, framesPerSecond)
    : NO_HEAD_HUD_VIEW;
  return Object.freeze({ model, snapshot, analysis: model.flightAnalysis, hud, headHud, environment, venue, frameRateEnabled: framesPerSecond !== undefined });
}

export function createFlightUiHudModel(model: AppModel): FlightHudModel {
  const view = model.sharedGameUi;
  const shared = view?.hud;
  if (shared === undefined || shared === null) throw new RangeError("Shared Rust HUD observation is unavailable");
  if (view === null) throw new RangeError("Shared Rust UI observation is unavailable");
  return projectSharedFlightHudModel(shared, view.theme);
}

export function finalizeFlightFrameView(draft: FlightFrameViewDraft, headHud: HeadHudView): UiViewModel {
  const view = gameSessionPhaseCode(draft.model.gameSession) < 0
    ? createBootViewModel(draft.model)
    : draft.model.sharedGameUi === null ? createBootViewModel(draft.model)
    : createGameViewModel(draft.model.sharedGameUi, headHud.kind === "visible" ? headHud.layer : NO_HEAD_HUD);
  return draft.frameRateEnabled && headHud.kind === "visible" ? Object.freeze({ ...view, headHud: headHud.layer }) : view;
}

export interface FlightMenuFrameFailure {
  readonly kind: "flight-menu-unavailable";
  readonly reason: "context-unavailable" | "text-overflow";
  readonly mode: "phone-vr" | "webxr";
  readonly presentation: AppModel["presentation"];
  readonly requestGeneration: number;
  readonly viewModel: UiViewModel;
}

export function failFlightMenuFrame(draft: FlightFrameViewDraft, viewModel: UiViewModel, reason: FlightMenuFrameFailure["reason"]): FlightMenuFrameFailure {
  const presentation = draft.model.presentation;
  if (presentation.type !== "ready" || presentation.mode === "screen") throw new Error("Flight Menu failure requires an active VR presentation");
  return Object.freeze({ kind: "flight-menu-unavailable", reason, mode: presentation.mode, presentation,
    requestGeneration: draft.model.nextRequestId,
    viewModel: Object.freeze({ ...viewModel, panels: Object.freeze([]), headHud: NO_HEAD_HUD }) });
}

export function flightMenuFailureRecovery(model: AppModel, failure: FlightMenuFrameFailure): AppMessage | null {
  if (model.presentation !== failure.presentation || model.nextRequestId !== failure.requestGeneration) return null;
  return Object.freeze({ type: "backend-ended", mode: failure.mode, message: `VR Flight Menu rendering failed: ${failure.reason}` });
}

export function menuFrameFailureRecovery(model: AppModel, frameModel: AppModel, mode: "phone-vr" | "webxr", reason: PanelUnavailableReason): AppMessage | null {
  if (reason === "viewer-unavailable" || model !== frameModel || frameModel.presentation.type !== "ready" || frameModel.presentation.mode !== mode) return null;
  return Object.freeze({ type: "backend-ended", mode, message: `VR Menu rendering failed: ${reason}` });
}
